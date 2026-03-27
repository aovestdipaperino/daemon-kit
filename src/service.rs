//! Service installation — launchd (macOS), systemd (Linux), Windows Service.

use std::path::PathBuf;

use crate::config::DaemonConfig;
use crate::error::{DaemonError, Result};

/// Handles installing/uninstalling autostart services.
pub struct ServiceInstaller<'a> {
    config: &'a DaemonConfig,
}

impl<'a> ServiceInstaller<'a> {
    pub fn new(config: &'a DaemonConfig) -> Self {
        Self { config }
    }

    /// Install the autostart service for the current platform.
    pub fn install(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        return self.install_launchd();

        #[cfg(target_os = "linux")]
        return self.install_systemd();

        #[cfg(target_os = "windows")]
        return self.install_windows_service();

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        Err(DaemonError::Unsupported(
            "autostart not supported on this platform".to_string(),
        ))
    }

    /// Uninstall the autostart service.
    pub fn uninstall(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        return self.uninstall_launchd();

        #[cfg(target_os = "linux")]
        return self.uninstall_systemd();

        #[cfg(target_os = "windows")]
        return self.uninstall_windows_service();

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        Err(DaemonError::Unsupported(
            "autostart not supported on this platform".to_string(),
        ))
    }

    /// Check if an autostart service is installed.
    pub fn is_installed(&self) -> bool {
        self.service_path().is_some_and(|p| p.exists())
    }

    /// Returns the platform-specific service file path.
    fn service_path(&self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        #[cfg(target_os = "macos")]
        {
            Some(home.join(format!(
                "Library/LaunchAgents/com.{}.plist",
                self.config.name
            )))
        }
        #[cfg(target_os = "linux")]
        {
            Some(home.join(format!(
                ".config/systemd/user/{}.service",
                self.config.name
            )))
        }
        #[cfg(target_os = "windows")]
        {
            // Windows services are registered in the SCM, not as files.
            // We check via the service manager in is_installed() instead.
            None
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            None
        }
    }

    // -----------------------------------------------------------------------
    // macOS — launchd
    // -----------------------------------------------------------------------

    #[cfg(target_os = "macos")]
    fn install_launchd(&self) -> Result<()> {
        let plist_path = self.service_path().ok_or_else(|| {
            DaemonError::Service("cannot determine home directory".to_string())
        })?;

        if let Some(parent) = plist_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let bin = self.config.executable.display();
        let args: String = self
            .config
            .service_args
            .iter()
            .map(|a| format!("        <string>{a}</string>"))
            .collect::<Vec<_>>()
            .join("\n");

        let log_path = self
            .config
            .log_file
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "/tmp/daemon-kit.log".to_string());

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.{name}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
{args}
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{log_path}</string>
    <key>StandardErrorPath</key>
    <string>{log_path}</string>
</dict>
</plist>"#,
            name = self.config.name,
        );

        std::fs::write(&plist_path, plist)?;
        log::info!("wrote {}", plist_path.display());

        let status = std::process::Command::new("launchctl")
            .args(["load", &plist_path.to_string_lossy()])
            .status();

        match status {
            Ok(s) if s.success() => log::info!("loaded launchd service"),
            _ => log::warn!(
                "could not load service — run: launchctl load {}",
                plist_path.display()
            ),
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn uninstall_launchd(&self) -> Result<()> {
        let Some(plist_path) = self.service_path() else {
            return Ok(());
        };
        if !plist_path.exists() {
            log::info!("no launchd service found");
            return Ok(());
        }
        let _ = std::process::Command::new("launchctl")
            .args(["unload", &plist_path.to_string_lossy()])
            .status();
        std::fs::remove_file(&plist_path)?;
        log::info!("removed launchd service");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Linux — systemd
    // -----------------------------------------------------------------------

    #[cfg(target_os = "linux")]
    fn install_systemd(&self) -> Result<()> {
        let unit_path = self.service_path().ok_or_else(|| {
            DaemonError::Service("cannot determine home directory".to_string())
        })?;

        if let Some(parent) = unit_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let bin = self.config.executable.display();
        let args = self.config.service_args.join(" ");

        let unit = format!(
            r#"[Unit]
Description={description}

[Service]
ExecStart={bin} {args}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#,
            description = self.config.description,
        );

        std::fs::write(&unit_path, unit)?;
        log::info!("wrote {}", unit_path.display());

        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();

        let status = std::process::Command::new("systemctl")
            .args(["--user", "enable", "--now", &self.config.name])
            .status();

        match status {
            Ok(s) if s.success() => log::info!("enabled and started systemd service"),
            _ => log::warn!(
                "could not enable service — run: systemctl --user enable --now {}",
                self.config.name
            ),
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn uninstall_systemd(&self) -> Result<()> {
        let Some(unit_path) = self.service_path() else {
            return Ok(());
        };
        if !unit_path.exists() {
            log::info!("no systemd service found");
            return Ok(());
        }
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", "--now", &self.config.name])
            .status();
        std::fs::remove_file(&unit_path)?;
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        log::info!("removed systemd service");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Windows — Windows Service via SCM
    // -----------------------------------------------------------------------

    #[cfg(target_os = "windows")]
    fn install_windows_service(&self) -> Result<()> {
        use windows_service::service::{
            ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType,
        };
        use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CREATE_SERVICE,
        )
        .map_err(|e| DaemonError::Service(format!("failed to connect to SCM: {e}")))?;

        let mut launch_args = vec![self.config.executable.to_string_lossy().to_string()];
        launch_args.extend(self.config.service_args.clone());

        let service_info = ServiceInfo {
            name: OsString::from(&self.config.name),
            display_name: OsString::from(&self.config.description),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: self.config.executable.clone(),
            launch_arguments: self
                .config
                .service_args
                .iter()
                .map(|s| std::ffi::OsString::from(s))
                .collect(),
            dependencies: vec![],
            account_name: None,
            account_password: None,
        };

        manager
            .create_service(&service_info, ServiceAccess::CHANGE_CONFIG)
            .map_err(|e| DaemonError::Service(format!("failed to create service: {e}")))?;

        log::info!("installed Windows service '{}'", self.config.name);
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn uninstall_windows_service(&self) -> Result<()> {
        use windows_service::service::ServiceAccess;
        use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

        let manager =
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
                .map_err(|e| DaemonError::Service(format!("failed to connect to SCM: {e}")))?;

        let service = manager
            .open_service(&self.config.name, ServiceAccess::DELETE)
            .map_err(|e| {
                DaemonError::Service(format!(
                    "failed to open service '{}': {e}",
                    self.config.name
                ))
            })?;

        service
            .delete()
            .map_err(|e| DaemonError::Service(format!("failed to delete service: {e}")))?;

        log::info!("removed Windows service '{}'", self.config.name);
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn is_installed_windows(&self) -> bool {
        use windows_service::service::ServiceAccess;
        use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

        let Ok(manager) =
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        else {
            return false;
        };
        manager
            .open_service(&self.config.name, ServiceAccess::QUERY_STATUS)
            .is_ok()
    }
}
