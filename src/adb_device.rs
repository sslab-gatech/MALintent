//! ADB device representing an Android device or emulator.
//!  
//! This contains utility functions to interact with the device through adb.

use core::panic;
use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, Command},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::util::encode_hex;

use tempfile::tempdir;

use subprocess::ExitStatus;
use subprocess::Popen;
use subprocess::PopenConfig;
use subprocess::Redirection;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdbDevice {
    adb_command: String,
}

impl AdbDevice {
    pub fn new(adb_command: &str) -> Self {
        Self {
            adb_command: adb_command.to_owned(),
        }
    }

    /// Runs a command on the device and returns the stdout.
    fn run_command(&self, command: &str) -> Result<String, libafl::Error> {
        let mut adb_command = Command::new(&self.adb_command);
        adb_command.arg("shell").arg(command);
        println!("Running command: {:?}", adb_command);
        let output = adb_command
            .output()
            .expect(&format!("Failed to execute command: {}", command));

        let stdout = String::from_utf8(output.stdout).expect("Failed to parse command stdout");

        // Check the exit code
        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr).expect("Failed to parse command stderr");

            return Err(libafl::Error::unknown(&format!(
                "Command failed: {}\nStdout: {}\nStderr: {}",
                command, stdout, stderr
            )));
        }

        Ok(stdout)
    }

    /// Runs a command on the device and returns the stdout as a reader.
    fn run_command_io(&self, command: &str) -> Result<Child, libafl::Error> {
        let mut adb_command = Command::new(&self.adb_command);
        adb_command.arg("shell").arg(command);

        let child = adb_command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect(&format!("Failed to spawn command: {}", command));

        Ok(child)
    }

    /// Runs an "am start" command on the device
    pub fn run_am_start(&self, command: &str, app_name: &str, timeout: Duration) -> Result<(), io::Error> {
        let mut adb_command = Command::new(&self.adb_command);
        adb_command.arg("shell").arg(command);

        for i in 0..5 {
            let mut restart = false;

            println!("Running command: {:?}", adb_command);

            let mut p = Popen::create(
                &[
                    &self.adb_command.to_owned(),
                    &"shell".to_owned(),
                    &command.to_owned(),
                ],
                PopenConfig {
                    stdout: Redirection::Pipe,
                    stderr: Redirection::Pipe,
                    ..Default::default()
                },
            )
            .unwrap();

            // Wait for the command to finish
            let result = p.wait_timeout(timeout);
            if let Ok(None) = result {
                println!("Command timed out");

                // A timeout indicates a lack of resources
                restart = true;

                if p.kill().is_err() {
                    println!("Failed to kill");
                }

                // Wait for the command to finish
                println!("Waiting for command to finish");
                p.wait().unwrap();
            }

            // Capture stdout
            let mut stdout = String::new();
            BufReader::new(p.stdout.take().unwrap())
                .read_to_string(&mut stdout)
                .unwrap();

            // Capture stderr
            let mut stderr = String::new();
            BufReader::new(p.stderr.take().unwrap())
                .read_to_string(&mut stderr)
                .unwrap();

            // Capture the exit code
            let exit_code = p.poll().unwrap();

            // The command failed when there is either a non-zero exit code or
            // output on stderr.
            // Thus, we return Ok only if the command succeeded.
            if stderr.contains("intent has been delivered to currently running top-most instance.")
            {
                return Ok(());
            }

            if let ExitStatus::Exited(0) = exit_code {
                // Now, we need to check the output on stderr.
                // Successfull, if stderr is empty or contains "has been delivered"
                if stderr.is_empty() {
                    return Ok(());
                }

                if stderr.contains("Activity class") && stderr.contains("does not exist") {
                    // This should be handled like a timeout
                    println!("Activity does not exist");
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "Activity does not exist"));
                }

                println!("Command failed (stderr)");
            } else {
                println!("Command failed (exit code): {:?}", exit_code);
            }

            // If the device is low on resources, we restart it
            if stderr.contains("OutOfResourcesException")
                || stderr.contains(
                    "Activity not started, its current task has been brought to the front",
                )
            {
                restart = true;
            }

            println!("Stdout: {}", stdout);
            println!("Stderr: {}", stderr);

            if restart {
                if i > 1 {
                    self.restart_device();
                }

                self.restart_app(app_name);
            }

            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        Err(io::Error::new(
            io::ErrorKind::Other,
            "Maximum retries reached",
        ))
    }

    #[allow(dead_code)]
    fn start_app_monkey(&self, app_name: &str) {
        // Calling the monkey multiple times will not create additional idle events
        self.run_command(&format!("monkey --pct-syskeys 0 -p {} 1", app_name))
            .expect("Failed to start app");
    }

    fn start_app_explicit(&self, app_name: &str) -> Result<(), libafl::Error> {
        // Get the main activity of the app
        let output = self
            .run_command(&format!(
                "cmd package resolve-activity --brief {} | tail -n 1",
                app_name
            ))
            .expect("Failed to get main activity");

        let main_activity = output.trim();

        // Bail out if there is any space character in the main activity
        if main_activity.contains(" ") {
            return Err(libafl::Error::unknown(&format!(
                "Invalid main activity: {}",
                main_activity
            )));
        }

        // Start the app
        let command = format!(
            "am start-activity --attach-agent /data/user/0/{}/code_cache/startup_agents/libcoverage_instrumenting_agent.so {}",
            app_name, main_activity
        );

        match self.run_command(&command) {
            Ok(_) => Ok(()),
            Err(err) => {
                Err(libafl::Error::unknown(&format!(
                    "Failed to start app: {}",
                    err
                )))
            },
        }
    }

    /// Tries to start the app with the given name.
    pub fn start_app(&self, app_name: &str) -> Result<(), libafl::Error> {
        println!("Starting app: {}", app_name);
        //self.start_app_monkey(app_name);
        self.start_app_explicit(app_name)?;

        // Get the pid of the app
        std::thread::sleep(std::time::Duration::from_secs(2));
        let pid = self.pid_of(app_name)?;

        println!("App started (pid {}), waiting for idle", pid);

        std::thread::sleep(std::time::Duration::from_secs(5));

        // Wait for the app to start
        let shell_command = format!("logcat --pid={}", pid,);

        let mut logcat_child = self
            .run_command_io(&shell_command)
            .expect("Failed to start logcat command");

        let stdout = logcat_child.stdout.take().expect("Failed to get stdout");
        let reader = &mut BufReader::new(stdout).lines();

        // Shared object holding the time of the last update
        let last_update = Arc::new(Mutex::new(Some(Instant::now())));
        let last_update_clone = Arc::clone(&last_update);

        // Start timeout thread
        let handle = thread::spawn(move || {
            loop {
                match *last_update_clone.lock().unwrap() {
                    Some(my_time) => {
                        if my_time.elapsed() > Duration::from_secs(20) {
                            break;
                        }
                    }
                    None => {
                        break;
                    }
                }

                thread::sleep(Duration::from_secs(1));
            }

            logcat_child.kill().expect("Failed to kill child");
        });

        let wait_for_timeout_thread = || {
            *last_update.lock().unwrap() = None;
            handle.join().unwrap();
        };

        while let Some(line) = reader.next() {
            // Update the last_update
            *last_update.lock().unwrap() = Some(Instant::now());

            match line {
                Ok(line) => {
                    if line.contains("ActivityThread: Reporting idle of ActivityRecord") {
                        println!("Found idle message: {:?}", line);

                        // Signal thread to stop
                        wait_for_timeout_thread();

                        return Ok(());
                    }
                }
                Err(_) => {
                    continue;
                }
            }

        }

        println!("Failed to find idle message");

        // Signal thread to stop
        wait_for_timeout_thread();

        return Err(libafl::Error::unknown(
            "Could not find idle message in logcat",
        ));
    }

    /// Stops the app with the given name.
    pub fn stop_app(&self, app_name: &str) -> Result<(), libafl::Error> {
        println!("Stopping app: {}", app_name);
        for _ in 0..5 {
            if self
                .run_command(&format!("pm disable {}", app_name))
                .is_ok()
            {
                if self.run_command(&format!("pm enable {}", app_name)).is_ok() {
                    return Ok(());
                }
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        return Err(libafl::Error::unknown(&format!(
            "Failed to stop app {}",
            app_name
        )));
    }

    /// Restarts the app with the given name.
    pub fn restart_app(&self, app_name: &str) {
        println!("Restarting app: {}", app_name);

        for i in 0..5 {
            if i > 1 {
                self.restart_device();
            }

            self.stop_app(app_name).expect("Failed to stop app");

            // Some apps need to be started immediately, others need some time
            std::thread::sleep(std::time::Duration::from_secs(i % 2));

            match self.start_app(app_name) {
                Ok(_) => return,
                Err(err) => {
                    println!("Failed to start app: {}", err);
                }
            }
        }

        // Failed to start the app
        panic!("Failed to re-start app");
    }

    /// Returns the pid of the app with the given name.
    pub fn pid_of(&self, app_name: &str) -> Result<String, libafl::Error> {
        let shell_command = format!("pidof -s {}", app_name,);

        let output = self.run_command(&shell_command)?;

        let pid = output.trim().to_owned();

        // Return Err if pid is empty
        if pid.is_empty() {
            return Err(libafl::Error::unknown(&format!(
                "Failed to get pid of app {}",
                app_name
            )));
        }

        Ok(pid)
    }

    /// Restart the entire device via adb.
    pub fn restart_device(&self) {
        println!("Restarting device");
        self.run_command("stop").expect("Failed to stop device");
        std::thread::sleep(std::time::Duration::from_secs(1));
        self.run_command("start").expect("Failed to start device");
        std::thread::sleep(std::time::Duration::from_secs(3));
    }

    /// Enables native hooking for an application and restarts it, if it was not already enabled.
    pub fn enable_native_hooking(&self, app_name: &str) {
        println!("Enabling native hooking for app: {}", app_name);
        let was_enabled = self.is_native_hooking_enabled(app_name);

        // The filename is the file ".hook_native" in the app's data directory
        let filename = format!("/data/user/0/{}/.hook_native", app_name);

        // Create the file
        self.run_command(&format!("touch {}", filename))
            .expect("Failed to touch file");

        if !was_enabled {
            // Restart the app if it was not already enabled
            self.restart_app(app_name);
        }
    }

    /// Disables native hooking for an application and restarts it, if it was enabled.
    pub fn disable_native_hooking(&self, app_name: &str) {
        println!("Disabling native hooking for app: {}", app_name);
        let was_enabled = self.is_native_hooking_enabled(app_name.clone());

        // The filename is the file ".hook_native" in the app's data directory
        let filename = format!("/data/user/0/{}/.hook_native", app_name);

        // Delete the file
        self.run_command(&format!("rm -f {}", filename))
            .expect("Failed to delete file");

        if was_enabled {
            // Restart the app if it was enabled
            self.restart_app(app_name);
        }
    }

    /// Check if native hooking is enabled for the given app.
    pub fn is_native_hooking_enabled(&self, app_name: &str) -> bool {
        // The filename is the file ".hook_native" in the app's data directory
        let filename = format!("/data/user/0/{}/.hook_native", app_name);

        // Check if the file exists
        let output = self.run_command(&format!("ls {}", filename));

        return match output {
            Ok(output) => output.trim() == filename,
            Err(_) => false,
        };
    }

    /// Pulls trace files from the device.
    pub fn pull_native_trace_files(
        &self,
        app_name: &str,
        trace_dir_host: &PathBuf,
    ) -> Result<(), io::Error> {
        println!("Pulling trace files for app: {}", app_name);

        // The trace files are located in the app's data directory
        let trace_dir = format!("/data/user/0/{}/native_traces", app_name);

        // Pull the files to a temporary directory
        let temp_dir = tempdir()?;
        let temp_dir_path = temp_dir.path().to_owned();

        // Pull the files
        let output = Command::new(&self.adb_command)
            .arg("pull")
            .arg(&trace_dir)
            .arg(&temp_dir_path)
            .output()?;

        // Print the output
        println!("Output: {}", String::from_utf8_lossy(&output.stdout));

        // Move the files to the destination directory
        let native_traces_dir = temp_dir_path.join("native_traces");

        if !native_traces_dir.exists() {
            println!("No native traces found");
            return Ok(());
        }

        fs::create_dir_all(trace_dir_host)?;
        for entry in fs::read_dir(native_traces_dir)? {
            let entry = entry?;
            let dest_path = trace_dir_host.join(entry.file_name());
            fs::copy(entry.path(), dest_path)?;
        }

        // Delete the files on the device
        self.run_command(&format!("rm -rf {}", trace_dir))
            .expect("Failed to delete files on the device");

        Ok(())
    }

    // Creates a file on the device with the given bytes
    pub fn create_file(&self, filename: &str, content: Vec<u8>) {
        //println!("Creating file: {} (length: {})", filename, content.len());

        // Create the file
        self.run_command(&format!("touch {}", filename))
            .expect("Failed to touch file");

        // Write the byte content to the file
        self.run_command(&format!(
            "echo -n -e \"{}\" > {}",
            encode_hex(&content),
            filename
        ))
        .expect("Failed to write to file");
    }

    // Register a content on the device with the given bytes
    pub fn register_content(&self, uri: &str, content: Vec<u8>) {
        //println!("Registering content: {} (length: {})", uri, content.len());

        let mut child = self
            .run_command_io(&format!("content write --uri {}", uri))
            .expect("Failed to register content");

        // Write the byte content to the file
        let mut stdin = child.stdin.take().unwrap();

        stdin.write_all(&content).expect("Failed to write to file");

        // Close stdin
        drop(stdin);

        // Wait for the command to finish
        child
            .wait()
            .expect("Failed to wait for content command to finish");
    }

    // Grant content provider uri permissions to the given package
    pub fn grant_uri_permissions(&self, package: &str) {
        // The command is something like: am broadcast -n 'org.gts3.jnifuzz.contentprovider/org.gts3.jnifuzz.contentprovider.UriPermissionManager' -a org.gts3.jnifuzz.sampleintent.UriPermissionManager --es android.intent.extra.PACKAGE_NAME 'com.instagram.android'
        // This function uses run_command
        //println!("Granting uri permissions: {} to {}", uri, package);

        self.run_command(&format!(
            "am broadcast -n 'org.gts3.jnifuzz.contentprovider/org.gts3.jnifuzz.contentprovider.UriPermissionManager' \
            -a org.gts3.jnifuzz.sampleintent.GRANT_PERMISSION \
            --es android.intent.extra.PACKAGE_NAME '{}'",
            package,
        )).expect("Failed to grant uri permissions");
    }

    // Set the given app as debug app
    pub fn set_debug_app(&self, package: &str) {
        self.run_command(&format!(
            "am set-debug-app --persistent {}",
            package,
        )).expect("Failed to set debug app");
    }

    // Reports if a native crash happened in the app, and whether it's caused by
    // the coverage agent (i.e., libcoverage_agent found in the stack trace)
    pub fn report_native_crash(&self, app_name: &str) {
        // Check the logcat 'crash' buffer of the past 3 seconds for native crashes
        let start_time = (SystemTime::now() - Duration::from_secs(3))
            .duration_since(UNIX_EPOCH)
            .unwrap();

        let shell_command = &format!(
            "logcat -b crash -t {}.{:03}",
            start_time.as_secs(),
            start_time.subsec_millis()
        );

        let output = self
            .run_command(&shell_command)
            .expect("Failed to start logcat command");

        let mut found_crash = false;
        let mut caused_by_coverage = false;

        for line in output.lines() {
            // Check if the line contains "pid: <pid>"
            if line.contains("Fatal signal") {
                found_crash = line.contains(&format!("({})", app_name));
            }

            if found_crash && line.contains("libcoverage_instrumenting_agent.so") {
                caused_by_coverage = true;
                break;
            }
        }

        if found_crash {
            println!("Found native crash (caused by coverage: {})", caused_by_coverage);
        }
    }
}
