//! Per-user ownership and activation, independent of executable/AppImage paths.
use crate::tray::Actions;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const RESTART_ARGUMENT: &str = "--wait-for-instance-exit";

#[derive(Serialize, Deserialize)]
struct Endpoint {
    port: u16,
    token: String,
    pid: u32,
}

#[derive(Default)]
struct Activation {
    // Retain requests received before the first graphical context exists.
    state: Mutex<(Option<Actions>, bool)>,
}

impl Activation {
    fn show(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(actions) = &state.0 {
            if actions.is_quitting() {
                return false;
            }
            actions.show();
        } else {
            state.1 = true;
        }
        true
    }
}

pub struct Instance {
    _lock: File,
    endpoint_path: PathBuf,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    activation: Arc<Activation>,
}

impl Instance {
    /// A duplicate activates the owner and returns None. Update replacements
    /// wait for the old owner to exit instead of activating an obsolete binary.
    pub fn acquire() -> io::Result<Option<Self>> {
        let directory = crate::settings::config_dir()?.join("runtime");
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(directory.join("instance.lock"))?;
        let endpoint_path = directory.join("activation.json");
        let restarting = std::env::args_os().any(|arg| arg == RESTART_ARGUMENT);
        let deadline = Instant::now() + Duration::from_secs(if restarting { 15 } else { 4 });
        loop {
            match lock.try_lock() {
                Ok(()) => break,
                Err(TryLockError::Error(err)) => return Err(err),
                Err(TryLockError::WouldBlock) => {
                    if !restarting && Self::activate(&endpoint_path).is_ok() {
                        return Ok(None);
                    }
                    if Instant::now() >= deadline {
                        return Err(io::Error::new(io::ErrorKind::TimedOut,
                            "Asset Extractor is already running but did not respond. Use its tray icon or wait for it to finish exiting."));
                    }
                    thread::sleep(Duration::from_millis(25));
                }
            }
        }

        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let mut nonce = [0u8; 32];
        getrandom::fill(&mut nonce).map_err(|err| io::Error::other(err.to_string()))?;
        let endpoint = Endpoint {
            port: listener.local_addr()?.port(),
            token: nonce.iter().map(|byte| format!("{byte:02x}")).collect(),
            pid: std::process::id(),
        };
        let expected = format!("asset-extractor-show:{}\n", endpoint.token).into_bytes();
        let activation = Arc::new(Activation::default());
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_activation = Arc::clone(&activation);
        let worker_shutdown = Arc::clone(&shutdown);

        // Publish atomically while holding the OS lock. A new lock owner can
        // replace stale endpoint information left behind by a crashed process.
        let mut file = tempfile::NamedTempFile::new_in(&directory)?;
        serde_json::to_writer(&mut file, &endpoint).map_err(io::Error::other)?;
        file.flush()?;
        file.persist(&endpoint_path).map_err(|err| err.error)?;
        let worker = thread::Builder::new()
            .name("instance-activation".into())
            .spawn(move || {
                while !worker_shutdown.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let timeout = Some(Duration::from_millis(300));
                            if stream.set_read_timeout(timeout).is_err()
                                || stream.set_write_timeout(timeout).is_err()
                            {
                                continue;
                            }
                            let mut request = vec![0; expected.len()];
                            if stream.read_exact(&mut request).is_ok()
                                && request == expected
                                && worker_activation.show()
                            {
                                let _ = stream.write_all(b"ok");
                            }
                        }
                        Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(20));
                        }
                        Err(_) => break,
                    }
                }
            })?;
        Ok(Some(Self {
            _lock: lock,
            endpoint_path,
            shutdown,
            worker: Some(worker),
            activation,
        }))
    }

    fn activate(path: &std::path::Path) -> io::Result<()> {
        let endpoint: Endpoint =
            serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)?;
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, endpoint.port));
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(300))?;
        stream.set_read_timeout(Some(Duration::from_millis(300)))?;
        stream.set_write_timeout(Some(Duration::from_millis(300)))?;
        #[cfg(target_os = "windows")]
        {
            // Transfer this user-initiated launch's foreground permission to
            // the existing GUI before it handles the activation request.
            // SAFETY: This Win32 API takes only the published process ID.
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(endpoint.pid);
            }
        }
        stream.write_all(format!("asset-extractor-show:{}\n", endpoint.token).as_bytes())?;
        let mut ack = [0; 2];
        stream.read_exact(&mut ack)?;
        if &ack != b"ok" {
            return Err(io::Error::other("Invalid activation response"));
        }
        Ok(())
    }

    pub fn attach(&self, actions: Actions) {
        let mut state = self
            .activation
            .state
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        if state.1 {
            actions.show();
            state.1 = false;
        }
        state.0 = Some(actions);
    }

    pub fn configure_restart(command: &mut std::process::Command) {
        command.arg(RESTART_ARGUMENT);
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        // Never unlink the lock file: a racing launch must lock the same inode.
        let _ = fs::remove_file(&self.endpoint_path);
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
