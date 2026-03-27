//! # daemon-kit
//!
//! Cross-platform daemon/service toolkit. Provides a unified API for:
//!
//! - **Daemonizing** a process (fork on Unix, Windows Service on Windows)
//! - **PID file** management (write, read, stale detection)
//! - **Service installation** (launchd on macOS, systemd on Linux, Windows Service on Windows)
//! - **Lifecycle** (start, stop, status)
//!
//! # Example
//!
//! ```no_run
//! use daemon_kit::{Daemon, DaemonConfig};
//!
//! let config = DaemonConfig::new("my-daemon")
//!     .pid_dir("~/.my-app")
//!     .log_file("~/.my-app/daemon.log");
//!
//! let daemon = Daemon::new(config);
//!
//! // Check status
//! if let Some(pid) = daemon.running_pid() {
//!     println!("running as PID {pid}");
//! }
//!
//! // Start (forks on Unix, foreground = false)
//! daemon.start(false, || {
//!     Ok(())
//! }).unwrap();
//! ```

mod config;
mod error;
mod pid;
mod service;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub use config::DaemonConfig;
pub use error::{DaemonError, Result};
pub use pid::PidFile;
pub use service::ServiceInstaller;

/// Main daemon handle. Provides start/stop/status and service installation.
pub struct Daemon {
    config: DaemonConfig,
    pid_file: PidFile,
}

impl Daemon {
    /// Create a new daemon handle from the given configuration.
    pub fn new(config: DaemonConfig) -> Self {
        let pid_path = config.pid_dir.join(format!("{}.pid", config.name));
        Self {
            pid_file: PidFile::new(pid_path),
            config,
        }
    }

    /// Returns the PID of the running daemon, or `None` if not running.
    /// Cleans up stale PID files automatically.
    pub fn running_pid(&self) -> Option<u32> {
        self.pid_file.alive_pid()
    }

    /// Returns `true` if the daemon is currently running.
    pub fn is_running(&self) -> bool {
        self.running_pid().is_some()
    }

    /// Start the daemon. On Unix, forks to background (unless `foreground` is
    /// true). On Windows, registers as a Windows Service.
    ///
    /// The `run` closure contains your long-running daemon logic. It will be
    /// called after daemonization is complete.
    ///
    /// Returns `Err` if a daemon is already running.
    pub fn start<F>(&self, foreground: bool, run: F) -> Result<()>
    where
        F: FnOnce() -> Result<()> + Send + 'static,
    {
        if self.is_running() {
            return Err(DaemonError::AlreadyRunning(
                self.running_pid().unwrap_or(0),
            ));
        }

        if foreground {
            self.pid_file.write()?;
            let result = run();
            self.pid_file.remove();
            return result;
        }

        self.start_platform(run)
    }

    /// Stop the running daemon.
    pub fn stop(&self) -> Result<()> {
        let Some(pid) = self.running_pid() else {
            return Err(DaemonError::NotRunning);
        };
        self.stop_platform(pid)?;
        self.pid_file.remove();
        Ok(())
    }

    /// Install an autostart service (launchd/systemd/Windows Service).
    pub fn install_service(&self) -> Result<()> {
        let installer = ServiceInstaller::new(&self.config);
        installer.install()
    }

    /// Remove the autostart service.
    pub fn uninstall_service(&self) -> Result<()> {
        let installer = ServiceInstaller::new(&self.config);
        installer.uninstall()
    }

    /// Returns true if an autostart service is installed.
    pub fn is_service_installed(&self) -> bool {
        let installer = ServiceInstaller::new(&self.config);
        installer.is_installed()
    }

    // Platform-specific implementations

    #[cfg(unix)]
    fn start_platform<F>(&self, run: F) -> Result<()>
    where
        F: FnOnce() -> Result<()> + Send + 'static,
    {
        unix::daemonize_and_run(&self.config, &self.pid_file, run)
    }

    #[cfg(windows)]
    fn start_platform<F>(&self, run: F) -> Result<()>
    where
        F: FnOnce() -> Result<()> + Send + 'static,
    {
        windows::start_service(&self.config, &self.pid_file, run)
    }

    #[cfg(unix)]
    fn stop_platform(&self, pid: u32) -> Result<()> {
        unix::stop_process(pid)
    }

    #[cfg(windows)]
    fn stop_platform(&self, _pid: u32) -> Result<()> {
        windows::stop_service(&self.config)
    }
}
