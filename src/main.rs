mod adb_device;
mod adb_executor;
mod intent_generator;
mod intent_input;
mod intent_mutator;
mod socket_coverage_observer;
mod util;

use adb_device::AdbDevice;
use clap::Parser;
use intent_generator::IntentGenerator;
use intent_input::IntentInput;
use intent_mutator::{
    IntentRandomAddExtraMutator, IntentRandomDataMutator, IntentRandomExtraContentMutator,
    IntentRandomExtraKeyMutator, IntentRandomExtraSchemeMutator, IntentRandomExtraSuffixMutator,
    IntentRandomFlagMutator, IntentRandomMimeTypeMutator,
};
use socket_coverage_observer::SocketCoverageObserver;

use std::{env, path::PathBuf};

use libafl::{
    prelude::{
        tuple_list, AflMapFeedback, CachedOnDiskCorpus, ConstFeedback, CrashFeedback,
        InMemoryCorpus, OnDiskCorpus, SimpleEventManager, SimpleMonitor, StdRand,
        StdScheduledMutator, OnDiskTOMLMonitor,
    },
    schedulers::QueueScheduler,
    stages::StdMutationalStage,
    state::StdState,
    Fuzzer, StdFuzzer,
};

/// Executes through adb on a device or emulator receiving coverage feedback
/// through a socket.
#[derive(Parser, Debug)]
#[command(version, about)]
struct CommandLineArgs {
    /// The address of the coverage agent socket
    #[arg(short, long, default_value = "localhost:6249")]
    coverage_socket_address: String,

    /// The adb command used to send intents and control the device, can also
    /// be set with the `ADB_COMMAND` environment variable
    #[arg(short, long, default_value = "adb")]
    adb_command: String,

    /// The config file or directory from where to read the intent information
    #[arg(short, long, default_value = "intent_template.json")]
    intent_config: String,

    /// Re-run corpus instead of fuzzing
    #[arg(short, long, default_value = "false")]
    run_corpus: bool,

    /// Trace JNI calls instead of Java coverage
    #[arg(short, long, default_value = "false")]
    trace_native: bool,

    /// Switch to disable usage of coverage feedback
    #[arg(long, default_value = "false")]
    no_coverage: bool,

    /// The directory to store the corpus in
    #[arg(long, default_value = "corpus")]
    corpus_dir: PathBuf,

    /// The directory to store the crashes in
    #[arg(long, default_value = "crashes")]
    crashes_dir: PathBuf,

    /// The directory to store the traces in
    #[arg(long, default_value = "traces")]
    traces_dir: PathBuf,

    /// The file to store the fuzzer stats in
    #[arg(long, default_value = "fuzzer_stats.toml")]
    stats_file: PathBuf,

    /// The file to store the overall edge count in
    #[arg(long, default_value = "overall_coverage.txt")]
    overall_coverage_file: PathBuf,
}

fn main() {
    let mut args = CommandLineArgs::parse();

    // Set ADB_COMMAND from environment if present.
    if let Ok(command) = env::var("ADB_COMMAND") {
        args.adb_command = command;
    }

    // Generator of initial intents.
    let generator = IntentGenerator::new(&args.intent_config);
    let app_name = generator.package_name();

    // Check if the receiver type is supported
    if !generator.is_supported() {
        println!("Receiver type not supported");
        return;
    }

    // Adb device to send intents to.
    let adb_device = AdbDevice::new(&args.adb_command);

    adb_device.grant_uri_permissions(&app_name);
    adb_device.set_debug_app(&app_name);

    let enable_synchronization = generator.enable_synchronization();

    if args.run_corpus {
        // Create the ".hook_native" file to enable JNI tracing.
        if args.trace_native {
            adb_device.enable_native_hooking(&app_name);
        } else {
            adb_device.disable_native_hooking(&app_name);
        }
        adb_device.restart_app(&app_name);

        // Observer to get coverage feedback from the device.
        let observer = socket_coverage_observer::create_coverage_map_observer(
            adb_device.clone(),
            app_name.clone(),
            &args.coverage_socket_address,
            true,
            enable_synchronization,
            !args.no_coverage,
            &args.overall_coverage_file,
        );

        re_run(observer, adb_device.clone(), args.corpus_dir);

        // Stop app to disable JNI tracing.
        adb_device.stop_app(&app_name).expect("Failed to stop app");

        if args.trace_native {
            // Pull the trace files from the device.
            adb_device
                .pull_native_trace_files(&app_name, &args.traces_dir)
                .expect("Failed to pull trace files");
        }
    } else {
        // Fuzzing with native hooking is not supported.
        if args.trace_native {
            println!("Native hooking is not supported for fuzzing. Please use the --run-corpus option.");
            return;
        }

        // Start the app.
        adb_device.disable_native_hooking(&app_name);
        adb_device.restart_app(&app_name);

        // Observer to get coverage feedback from the device.
        let observer = socket_coverage_observer::create_coverage_map_observer(
            adb_device.clone(),
            app_name.clone(),
            &args.coverage_socket_address,
            false,
            enable_synchronization,
            !args.no_coverage,
            &args.overall_coverage_file,
        );

        fuzz(observer, adb_device, args, generator);
    }
}

fn re_run(observer: SocketCoverageObserver, adb_device: AdbDevice, corpus_dir: PathBuf) {
    let mut feedback = ConstFeedback::new(true);
    let mut objective = ConstFeedback::new(false);
    // The Monitor trait defines how the fuzzer stats are displayed to the user
    let mon = SimpleMonitor::new(|s| println!("{s}"));
    // The event manager handles the various events generated during the fuzzing loop
    // such as the notification of the addition of a new item to the corpus
    let mut mgr = SimpleEventManager::new(mon);

    let mut state = StdState::new(
        // RNG
        StdRand::with_seed(0),
        // The corpus is kept in memory for performance
        InMemoryCorpus::<IntentInput>::new(),
        // Do not store solutions
        InMemoryCorpus::<IntentInput>::new(),
        // Constant feedbacks
        &mut feedback,
        // Same for objective feedbacks
        &mut objective,
    )
    .unwrap();

    let scheduler = QueueScheduler::new();

    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let mut executor = adb_executor::AdbExecutor::new(adb_device, tuple_list!(observer));

    state
        .load_initial_inputs_forced(
            &mut fuzzer,
            &mut executor,
            &mut mgr,
            &[PathBuf::from(corpus_dir)],
        )
        .expect("Failed to load the corpus");
}

fn fuzz(
    observer: SocketCoverageObserver,
    adb_device: AdbDevice,
    args: CommandLineArgs,
    mut generator: IntentGenerator,
) {
    let mut feedback = AflMapFeedback::new(&observer);
    // The Monitor trait defines how the fuzzer stats are displayed to the user
    let simple_mon = SimpleMonitor::new(|s| println!("{s}"));

    let mon = OnDiskTOMLMonitor::new(
        args.stats_file,
        simple_mon,
    );

    // The event manager handles the various events generated during the fuzzing loop
    // such as the notification of the addition of a new item to the corpus
    let mut mgr = SimpleEventManager::new(mon);

    // A feedback to choose if an input is a solution or not
    let mut objective = CrashFeedback::new();

    // create a State from scratch
    let mut state = StdState::new(
        // RNG
        StdRand::with_seed(0),
        // Corpus that will be evolved.
        CachedOnDiskCorpus::<IntentInput>::new(PathBuf::from(args.corpus_dir), 128).unwrap(),
        // Corpus in which we store solutions (crashes in this example),
        // on disk so the user can get them after stopping the fuzzer
        OnDiskCorpus::<IntentInput>::new(PathBuf::from(args.crashes_dir)).unwrap(),
        // States of the feedbacks.
        // The feedbacks can report the data that should persist in the State.
        &mut feedback,
        // Same for objective feedbacks
        &mut objective,
    )
    .unwrap();

    // A queue policy to get testcases from the corpus
    let scheduler = QueueScheduler::new();

    // A fuzzer with feedbacks and a corpus scheduler
    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let mut executor = adb_executor::AdbExecutor::new(adb_device, tuple_list!(observer));

    let number_of_intents = generator.number_of_intents();

    // Generate initial inputs
    state
        .generate_initial_inputs_forced(
            &mut fuzzer,
            &mut executor,
            &mut generator,
            &mut mgr,
            number_of_intents,
        )
        .expect("Failed to generate the initial corpus");

    let mutator = StdScheduledMutator::new(tuple_list!(
        IntentRandomDataMutator::new(),
        IntentRandomFlagMutator::new(),
        IntentRandomMimeTypeMutator::new(),
        IntentRandomAddExtraMutator::new(),
        IntentRandomExtraKeyMutator::new(),
        IntentRandomExtraContentMutator::new(),
        IntentRandomExtraSchemeMutator::new(),
        IntentRandomExtraSuffixMutator::new()
    ));
    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    fuzzer
        .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
        .expect("Error in the fuzzing loop");
}
