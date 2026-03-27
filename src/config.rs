//! Daemon configuration.

use std::path::PathBuf;

/// Configuration for a daemon instance.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Service/daemon name (used for PID file, service name, etc.)
    pub name: String,

    /// Directory for PID file (default: `~/.config/<name>/`)
    pub pid_dir: PathBuf,

    /// Path to the log file for daemon stdout/stderr.
    pub log_file: Option<PathBuf>,

    /// Absolute path to the executable binary.
    /// Used when generating service files. Defaults to `std::env::current_exe()`.
    pub executable: PathBuf,

    /// Arguments to pass when the service manager starts the daemon.
    /// Typically includes a "foreground" flag since the service manager
    /// handles backgrounding.
    pub service_args: Vec<String>,

    /// Human-readable description for the service.
    pub description: String,
}

impl DaemonConfig {
    /// Create a new config with the given daemon name.
    pub fn new(name: &str) -> Self {
        let pid_dir = dirs::home_dir()
            .map(|h| h.join(format!(".config/{name}")))
            .unwrap_or_else(|| PathBuf::from("/tmp"));

        let executable = std::env::current_exe().unwrap_or_else(|_| PathBuf::from(name));

        Self {
            name: name.to_string(),
            pid_dir,
            log_file: None,
            executable,
            service_args: vec!["daemon".to_string(), "--foreground".to_string()],
            description: format!("{name} daemon"),
        }
    }

    /// Set the PID file directory.
    pub fn pid_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.pid_dir = dir.into();
        self
    }

    /// Set the log file path.
    pub fn log_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.log_file = Some(path.into());
        self
    }

    /// Set the executable path (for service file generation).
    pub fn executable(mut self, path: impl Into<PathBuf>) -> Self {
        self.executable = path.into();
        self
    }

    /// Set the arguments the service manager passes when starting the daemon.
    pub fn service_args(mut self, args: Vec<String>) -> Self {
        self.service_args = args;
        self
    }

    /// Set the human-readable service description.
    pub fn description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }
}
