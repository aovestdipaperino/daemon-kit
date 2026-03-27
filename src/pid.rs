//! PID file management.

use std::path::{Path, PathBuf};

use crate::error::{DaemonError, Result};

/// Manages a PID file for tracking the daemon process.
pub struct PidFile {
    path: PathBuf,
}

impl PidFile {
    /// Create a new PID file handle at the given path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Path to the PID file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write the current process PID to the file.
    pub fn write(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, std::process::id().to_string()).map_err(|e| {
            DaemonError::PidFile(format!("failed to write {}: {e}", self.path.display()))
        })
    }

    /// Remove the PID file.
    pub fn remove(&self) {
        std::fs::remove_file(&self.path).ok();
    }

    /// Read the PID from the file. Returns `None` if missing or unreadable.
    pub fn read(&self) -> Option<u32> {
        let contents = std::fs::read_to_string(&self.path).ok()?;
        contents.trim().parse().ok()
    }

    /// Returns the PID if the file exists AND the process is alive.
    /// Removes stale PID files automatically.
    pub fn alive_pid(&self) -> Option<u32> {
        let pid = self.read()?;
        if is_process_alive(pid) {
            Some(pid)
        } else {
            self.remove();
            None
        }
    }
}

/// Check if a process with the given PID is alive.
#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None).is_ok()
}

#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    use std::os::windows::io::FromRawHandle;
    unsafe {
        let handle = winapi_process_handle(pid);
        if handle.is_null() {
            return false;
        }
        let mut exit_code: u32 = 0;
        // STILL_ACTIVE = 259
        let result = windows_sys::Win32::System::Threading::GetExitCodeProcess(
            handle as _,
            &mut exit_code as *mut u32,
        );
        windows_sys::Win32::Foundation::CloseHandle(handle as _);
        result != 0 && exit_code == 259
    }
}

// Fallback for other platforms
#[cfg(not(any(unix, windows)))]
fn is_process_alive(_pid: u32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_file_write_read_remove() {
        let dir = std::env::temp_dir().join("daemon-kit-test");
        std::fs::create_dir_all(&dir).unwrap();
        let pf = PidFile::new(dir.join("test.pid"));

        pf.write().unwrap();
        let pid = pf.read().unwrap();
        assert_eq!(pid, std::process::id());

        pf.remove();
        assert!(pf.read().is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stale_pid_file_cleaned_up() {
        let dir = std::env::temp_dir().join("daemon-kit-stale-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stale.pid");

        // Write a PID that definitely doesn't exist
        std::fs::write(&path, "99999999").unwrap();
        let pf = PidFile::new(&path);
        assert!(pf.alive_pid().is_none());
        // File should be cleaned up
        assert!(!path.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn alive_pid_returns_current_process() {
        let dir = std::env::temp_dir().join("daemon-kit-alive-test");
        std::fs::create_dir_all(&dir).unwrap();
        let pf = PidFile::new(dir.join("alive.pid"));

        pf.write().unwrap();
        // Current process is alive
        assert_eq!(pf.alive_pid(), Some(std::process::id()));

        pf.remove();
        std::fs::remove_dir_all(&dir).ok();
    }
}
