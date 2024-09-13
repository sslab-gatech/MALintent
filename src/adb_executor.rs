//! [Executor] for running a single intent through adb on device/emulator.
//!  
//! This contains the main [AdbExecutor] struct which implements the libAFL
//! [Executor] trait. This struct contains the logic to actually invoke and
//! monitor the execution of the intent on the device.

use std::fmt::Debug;
use std::time::Duration;
use std::{fmt::Formatter, marker::PhantomData};

use libafl::prelude::{
    ExitKind, HasBytesVec, HasObservers, MatchName, ObserversTuple, UsesObservers,
};
use libafl::{executors::Executor, prelude::UsesInput, state::UsesState};

use crate::adb_device::AdbDevice;
use crate::intent_input::{ExtraType, IntentInput, ReceiverType, URIScheme};

// Lots of single letter generic types get confusing. A best-effort explanation
// from my understanding:
//
// EM: Execution Manager?
// OT: ObserversType, usually a subtype of ObserversTuple.
// Z: Fuzzer
// S: State, the input state for the program?
pub struct AdbExecutor<EM, OT, Z, S> {
    adb_device: AdbDevice,

    observers: OT,
    phantom: PhantomData<(EM, S, Z)>,
}

impl<EM, OT, Z, S> AdbExecutor<EM, OT, Z, S> {
    pub fn new(adb_device: AdbDevice, observers: OT) -> Self {
        Self {
            adb_device,
            observers,
            phantom: PhantomData,
        }
    }
}

impl<EM, OT, Z, S> Executor<EM, Z> for AdbExecutor<EM, OT, Z, S>
where
    EM: UsesState<State = S>,
    OT: Debug + MatchName + ObserversTuple<S>,
    S: UsesInput<Input = IntentInput>,
    Z: UsesState<State = S>,
{
    fn run_target(
        &mut self,
        _fuzzer: &mut Z,
        _state: &mut Self::State,
        _mgr: &mut EM,
        input: &Self::Input,
    ) -> Result<libafl::prelude::ExitKind, libafl::Error> {
        //println!("Asked to run with input: {:?}", input);

        // Only 'activity' and 'broadcastReceiver' as receiver types are implemented as of now
        let timeout = match input.receiver_type {
            ReceiverType::Activity => Duration::from_secs(5),
            _ => Duration::from_secs(20),
        };

        // Get the command to run on the device
        let shell_command = input.shell_command();

        // Create required files and content on the device for all URI extras
        input
            .extras
            .iter()
            .enumerate()
            .filter_map(|(index, extra)| match &extra.value {
                ExtraType::URI(uri) => Some((index + 1, uri)),
                _ => None,
            })
            .chain(input.data.iter().map(|uri| (0, uri)))
            .for_each(|(id, uri)| {
                let identifier = uri.identifier(id);
                let content_bytes = uri.content.bytes().to_vec();

                // Depending on the scheme, create the file or register the content on the adb device
                // Note that we need to skip the "file://" prefix for the identifier if it is a file
                match uri.scheme {
                    URIScheme::Content => {
                        self.adb_device.register_content(&identifier, content_bytes)
                    }
                    URIScheme::File => self.adb_device.create_file(&identifier[7..], content_bytes),
                    URIScheme::Other => {}
                }
            });

        // Run the command
        println!("Running command: {:?}", shell_command);
        let result = self
            .adb_device
            .run_am_start(&shell_command, &input.component_package, timeout);

        // The command failed when there is either a non-zero exit code or
        // output on stderr.
        // Thus, we return Ok only if the command succeeded.
        match result {
            Ok(_) => Ok(ExitKind::Ok),
            Err(_) => Ok(ExitKind::Timeout)
        }
    }
}

// Need to implement HasObservers so we can use observers with this executor.
impl<EM, OT, Z, S> HasObservers for AdbExecutor<EM, OT, Z, S>
where
    S: UsesInput,
    OT: ObserversTuple<S>,
{
    fn observers(&self) -> &Self::Observers {
        &self.observers
    }

    fn observers_mut(&mut self) -> &mut Self::Observers {
        &mut self.observers
    }
}

// Debug and UsesState are required traits for implementing Executor in libAFL.
impl<EM, OT, Z, S> Debug for AdbExecutor<EM, OT, Z, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdbExecutor").finish()
    }
}

impl<EM, OT, Z, S> UsesState for AdbExecutor<EM, OT, Z, S>
where
    S: UsesInput,
{
    type State = S;
}

impl<EM, OT, Z, S> UsesObservers for AdbExecutor<EM, OT, Z, S>
where
    OT: ObserversTuple<S>,
    S: UsesInput,
{
    type Observers = OT;
}
