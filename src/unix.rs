//! Unix-specific daemon implementation using `daemonize2`.

use std::time::Duration;

use crate::config::DaemonConfig;
use crate::error::{DaemonError, Result};
use crate::pid::PidFile;

/// Daemonize the process and run the closure.
pub fn daemonize_and_run<F>(config: &DaemonConfig, pid_file: &PidFile, run: F) -> Result<()>
where
    F: FnOnce() -> Result<()> + Send + 'static,
{
    let mut daemon = daemonize2::Daemonize::new()
        .pid_file(pid_file.path())
        .working_directory(".");

    if let Some(ref log_path) = config.log_file {
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let stdout = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|e| DaemonError::Daemonize(format!("cannot open log file: {e}")))?;
        let stderr = stdout
            .try_clone()
            .map_err(|e| DaemonError::Daemonize(format!("cannot clone log fd: {e}")))?;
        daemon = daemon.stdout(stdout).stderr(stderr);
    }

    // SAFETY: fork() is inherently unsafe. We ensure no other threads are
    // running at this point (called before tokio runtime starts).
    unsafe {
        daemon
            .start()
            .map_err(|e| DaemonError::Daemonize(e.to_string()))?;
    }

    // We are now the forked child. PID file was written by daemonize2.
    run()
}

/// Stop a process by PID using SIGTERM, then SIGKILL after 5 seconds.
pub fn stop_process(pid: u32) -> Result<()> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    let nix_pid = Pid::from_raw(pid as i32);

    signal::kill(nix_pid, Signal::SIGTERM).map_err(|e| {
        DaemonError::Service(format!("failed to send SIGTERM to PID {pid}: {e}"))
    })?;

    // Wait up to 5 seconds for graceful shutdown
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if signal::kill(nix_pid, None).is_err() {
            // Process is gone
            log::info!("daemon stopped (PID: {pid})");
            return Ok(());
        }
    }

    // Force kill
    signal::kill(nix_pid, Signal::SIGKILL).ok();
    log::warn!("daemon force-killed (PID: {pid})");
    Ok(())
}
