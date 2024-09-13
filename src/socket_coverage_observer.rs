//! Handles getting the coverage map from CoverageAgent over a socket.

use std::{
    io::{BufReader, Read, Write},
    net::TcpStream, time::Duration, path::PathBuf,
};

use libafl::prelude::{
    AsIter, AsSlice, AsMutSlice, ConstMapObserver, HasLen, HitcountsMapObserver, MapObserver, Named,
    Observer, UsesInput,
};
use serde::{Deserialize, Serialize};

use crate::{adb_device::AdbDevice, intent_input::IntentInput};

const COVERAGE_MAP_SIZE: usize = 1024 * 1024;

pub fn create_coverage_map_observer<'a>(
    adb_device: AdbDevice,
    app_name: String,
    address: &str,
    trace_native: bool,
    enable_synchronization: bool,
    use_coverage: bool,
    overall_coverage_file: &PathBuf,
) -> SocketCoverageObserver<'a> {
    return SocketCoverageObserver::new(
        adb_device,
        app_name,
        address,
        trace_native,
        enable_synchronization,
        use_coverage,
        overall_coverage_file,
    );
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SocketCoverageObserver<'a> {
    adb_device: AdbDevice,
    app_name: String,
    address: String,
    trace_native: bool,
    enable_synchronization: bool,
    use_coverage: bool,

    #[serde(skip, default = "default_stream")]
    stream: TcpStream,
    #[serde(skip, default = "default_reader")]
    reader: BufReader<TcpStream>,

    base_observer: HitcountsMapObserver<ConstMapObserver<'a, u8, COVERAGE_MAP_SIZE>>,
    // array to keep track of which edges have been covered
    overall_coverage: ConstMapObserver<'a, u8, COVERAGE_MAP_SIZE>,

    overall_coverage_file: PathBuf,
    // Save the start time
    start_time: std::time::SystemTime,
    last_overall_coverage: u64,
}

impl<'a> SocketCoverageObserver<'a> {
    fn new(
        adb_device: AdbDevice,
        app_name: String,
        address: &str,
        trace_native: bool,
        enable_synchronization: bool,
        use_coverage: bool,
        overall_coverage_file: &PathBuf,
    ) -> Self {
        let mut stream = TcpStream::connect(address).expect("Failed to connect to socket");
        stream.set_read_timeout(Some(Duration::from_secs(10))).expect("Failed to set read timeout");
        let reader = BufReader::new(stream.try_clone().expect("Failed to clone tcp stream"));

        stream
            .set_nodelay(true)
            .expect("Failed to set nodelay on socket");

        // Set up the socket for synchronization if requested.
        stream
            .write(if enable_synchronization { b"ss" } else { b"se" })
            .expect("Failed to write to socket");

        // Delete coverage file if it exists
        if overall_coverage_file.exists() {
            std::fs::remove_file(overall_coverage_file).unwrap();
        }
        // Write first entry to coverage file
        let mut file = std::fs::File::create(overall_coverage_file).unwrap();
        file.write_all(b"0: 0\n").unwrap();

        Self {
            adb_device,
            app_name,
            address: address.to_owned(),
            trace_native,
            enable_synchronization,
            use_coverage,
            stream,
            reader,
            base_observer: HitcountsMapObserver::new(ConstMapObserver::owned(
                "edges_from_socket",
                vec![0; COVERAGE_MAP_SIZE],
            )),
            overall_coverage: ConstMapObserver::owned(
                "overall_edges",
                vec![0; COVERAGE_MAP_SIZE],
            ),
            overall_coverage_file: overall_coverage_file.to_owned(),
            start_time: std::time::SystemTime::now(),
            last_overall_coverage: 0,
        }
    }

    fn init(&mut self) {
        self.stream =
            TcpStream::connect(self.address.clone()).expect("Failed to connect to socket");
        self.stream.set_read_timeout(Some(Duration::from_secs(10))).expect("Failed to set read timeout");
        self.reader = BufReader::new(self.stream.try_clone().expect("Failed to clone tcp stream"));

        self.stream
            .set_nodelay(true)
            .expect("Failed to set nodelay on socket");

        // Set up the socket for synchronization if requested.
        self.stream
            .write(if self.enable_synchronization {
                b"ss"
            } else {
                b"se"
            })
            .expect("Failed to write to socket");
    }

    fn reset_coverage(&mut self, hash: String) -> Result<(), libafl::Error> {
        let mut buffer = [0; 1];

        if self.trace_native {
            // Write "ts", the filename, and a newline to the socket.
            // The filename is "id_<hash>.txt"
            self.stream.write(b"ts")?;
            self.stream
                .write(format!("trace_{}.txt", hash).as_bytes())?;
            self.stream.write(b"\n")?;
        }

        self.stream.write(b"r")?;
        self.reader.read(&mut buffer)?;
        // Check buffer contains b'd'
        if buffer[0] != b'd' {
            return Err(libafl::Error::unknown(format!(
                "Failed to reset coverage map (got {:?})",
                buffer
            )));
        }
        Ok(())
    }

    pub fn save_overall_edge_count(&self) {
        // Number of bytes not 0 in the overall coverage.
        let overall_coverage = self.overall_coverage.as_slice().iter().filter(|&b| *b != 0).count();

        // Do nothing if the overall coverage hasn't changed.
        if overall_coverage <= self.last_overall_coverage as usize {
            return;
        }

        // Create the directory if it doesn't exist
        let mut dir = self.overall_coverage_file.clone();
        dir.pop();
        std::fs::create_dir_all(&dir).unwrap();

        // Get the time since the start of the program
        let elapsed = self.start_time.elapsed().unwrap();

        // Append the overall coverage to the file
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.overall_coverage_file)
            .unwrap();
        file.write_all(format!("{}: {}\n", elapsed.as_secs(), overall_coverage).as_bytes())
            .unwrap();
    }
}

impl<S> Observer<S> for SocketCoverageObserver<'_>
where
    S: UsesInput<Input = IntentInput>,
{
    #[inline]
    fn pre_exec(
        &mut self,
        state: &mut S,
        input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        for i in 0..5 {
            if let Err(err) = self.reset_coverage(input.hash()) {
                println!(
                    "Failed to write reset message to socket. Restarting app. Error: {:?}",
                    err
                );

                if self.trace_native {
                    self.adb_device.report_native_crash(&self.app_name);
                }

                if i > 1 {
                    self.adb_device.restart_device();
                }

                self.adb_device.restart_app(&self.app_name);

                std::thread::sleep(std::time::Duration::from_secs(1 + i));

                self.init();

                std::thread::sleep(std::time::Duration::from_secs(1 + i));
            } else {
                // Reset the local coverage map.
                return self.base_observer.pre_exec(state, input);
            }
        }

        Err(libafl::Error::unknown(
            "Failed to reset coverage map (after restarting)",
        ))
    }

    #[inline]
    fn post_exec(
        &mut self,
        state: &mut S,
        input: &<S as UsesInput>::Input,
        exit_kind: &libafl::prelude::ExitKind,
    ) -> Result<(), libafl::Error> {
        // Retrieve the coverage from the socket into the observer.
        self.stream
            .write(b"d")
            .expect("Failed to write send-coverage message to socket");

        let mut buffer = vec![0; COVERAGE_MAP_SIZE];
        if let Err(_err) = self.reader.read_exact(&mut buffer) {
            println!("Failed to read entire coverage from socket.");
            return Ok(());
        }

        if self.use_coverage {
            let observer_buffer = self.base_observer.as_mut_slice();
            // Copy into the observer buffer
            observer_buffer.copy_from_slice(&buffer);
        }

        // Update the overall coverage.
        let overall_buffer = self.overall_coverage.as_mut_slice();
        for (i, &b) in buffer.iter().enumerate() {
            if b != 0 {
                overall_buffer[i] = b;
            }
        }

        // Save the overall edge count to a file
        self.save_overall_edge_count();

        self.base_observer.post_exec(state, input, exit_kind)
    }
}

impl Named for SocketCoverageObserver<'_> {
    fn name(&self) -> &str {
        "SocketCoverageObserver"
    }
}

impl HasLen for SocketCoverageObserver<'_> {
    fn len(&self) -> usize {
        self.base_observer.len()
    }
}

impl<'it> AsIter<'it> for SocketCoverageObserver<'_> {
    type Item = u8;
    type IntoIter = <ConstMapObserver<'it, u8, 1> as AsIter<'it>>::IntoIter;

    fn as_iter(&'it self) -> Self::IntoIter {
        self.base_observer.as_iter()
    }
}

impl MapObserver for SocketCoverageObserver<'_> {
    type Entry = u8;

    #[inline]
    fn get(&self, idx: usize) -> &Self::Entry {
        self.base_observer.get(idx)
    }

    #[inline]
    fn get_mut(&mut self, idx: usize) -> &mut Self::Entry {
        self.base_observer.get_mut(idx)
    }

    #[inline]
    fn usable_count(&self) -> usize {
        self.base_observer.usable_count()
    }

    fn count_bytes(&self) -> u64 {
        self.base_observer.count_bytes()
    }

    fn hash(&self) -> u64 {
        self.base_observer.hash()
    }

    #[inline]
    fn initial(&self) -> Self::Entry {
        self.base_observer.initial()
    }

    #[inline]
    fn reset_map(&mut self) -> Result<(), libafl::Error> {
        self.base_observer.reset_map()
    }

    fn to_vec(&self) -> Vec<Self::Entry> {
        self.base_observer.to_vec()
    }

    fn how_many_set(&self, indexes: &[usize]) -> usize {
        self.base_observer.how_many_set(indexes)
    }
}

// For some reason MapObserver requires the struct to implement Serialize/Deserialize.
//
// As far as I can tell it's not really used but since TcpStream and BufReader
// can't be serialized we need these two methods to make serde happy.
//
// Panic if they ever get called.
fn default_stream() -> TcpStream {
    panic!("Deserialize (default_stream) called on SocketCoverageObserver")
}
fn default_reader() -> BufReader<TcpStream> {
    panic!("Deserialize (default_reader) called on SocketCoverageObserver")
}
