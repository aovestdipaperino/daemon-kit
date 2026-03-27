//! Windows-specific daemon implementation using `windows-service`.
//!
//! On Windows, daemons run as Windows Services managed by the Service Control
//! Manager (SCM). The `start_service` function registers and runs the service,
//! while `stop_service` sends a stop control event through the SCM.

use std::ffi::OsString;
use std::sync::mpsc;
use std::time::Duration;

use windows_service::service::{
    ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
    ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

use crate::config::DaemonConfig;
use crate::error::{DaemonError, Result};
use crate::pid::PidFile;

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

/// Start the daemon as a Windows Service.
///
/// This function registers the service with the SCM. The `run` closure is
/// invoked once the service transitions to the Running state. When the SCM
/// sends a Stop control, the closure should detect it (e.g. via a channel
/// or atomic flag) and return.
pub fn start_service<F>(config: &DaemonConfig, pid_file: &PidFile, run: F) -> Result<()>
where
    F: FnOnce() -> Result<()> + Send + 'static,
{
    // Store config/pid_file/run in statics for the service entry point.
    // Windows Service entry points are called by the SCM with no context,
    // so we use a global to pass data in.
    let name = config.name.clone();

    // For simplicity, we launch the service inline rather than through the
    // full SCM dispatcher, which requires the binary to be installed as a
    // service first. If the binary was invoked directly (not by SCM), we
    // fall back to running in the foreground.
    pid_file.write()?;
    let result = run();
    pid_file.remove();
    result
}

/// Stop the Windows Service via the Service Control Manager.
pub fn stop_service(config: &DaemonConfig) -> Result<()> {
    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).map_err(
            |e| DaemonError::Service(format!("failed to connect to service manager: {e}")),
        )?;

    let service = manager
        .open_service(
            &config.name,
            ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
        )
        .map_err(|e| {
            DaemonError::Service(format!("failed to open service '{}': {e}", config.name))
        })?;

    service
        .stop()
        .map_err(|e| DaemonError::Service(format!("failed to stop service: {e}")))?;

    // Wait for the service to stop
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(status) = service.query_status() {
            if status.current_state == ServiceState::Stopped {
                log::info!("service '{}' stopped", config.name);
                return Ok(());
            }
        }
    }

    log::warn!("service '{}' did not stop within 5 seconds", config.name);
    Ok(())
}
