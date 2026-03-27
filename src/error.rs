//! Error types for daemon-kit.

/// Result type alias for daemon-kit operations.
pub type Result<T> = std::result::Result<T, DaemonError>;

/// Errors that can occur during daemon operations.
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("daemon already running (PID: {0})")]
    AlreadyRunning(u32),

    #[error("daemon is not running")]
    NotRunning,

    #[error("failed to daemonize: {0}")]
    Daemonize(String),

    #[error("PID file error: {0}")]
    PidFile(String),

    #[error("service error: {0}")]
    Service(String),

    #[error("platform not supported: {0}")]
    Unsupported(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
