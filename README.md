# daemon-kit

Cross-platform daemon/service toolkit for Rust. Provides a unified API for daemonizing processes, managing PID files, and installing system services across macOS, Linux, and Windows.

## Platform backends

| Platform | Daemonize | Signals | Service |
|----------|-----------|---------|---------|
| macOS | `daemonize2` (fork/setsid) | `nix` (SIGTERM/SIGKILL) | launchd plist |
| Linux | `daemonize2` (fork/setsid) | `nix` (SIGTERM/SIGKILL) | systemd user unit |
| Windows | `windows-service` (SCM) | SCM stop control | Windows Service |

## Usage

```rust
use daemon_kit::{Daemon, DaemonConfig};

let config = DaemonConfig::new("my-daemon")
    .pid_dir("~/.my-app")
    .log_file("~/.my-app/daemon.log")
    .description("My background service");

let daemon = Daemon::new(config);

// Check if already running
if let Some(pid) = daemon.running_pid() {
    println!("Already running as PID {pid}");
    return;
}

// Start (forks on Unix, registers with SCM on Windows)
daemon.start(false, || {
    // Your long-running daemon logic here
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
    #[allow(unreachable_code)]
    Ok(())
}).unwrap();
```

### Foreground mode

Pass `true` to `start()` to skip forking — useful for debugging or when managed by an external service manager:

```rust
daemon.start(true, || {
    // Runs in the current process
    Ok(())
}).unwrap();
```

### Stop and status

```rust
// Check status
if daemon.is_running() {
    println!("Daemon is running");
}

// Stop (SIGTERM on Unix, SCM stop on Windows)
daemon.stop().unwrap();
```

### Service installation

Install an autostart service so the daemon starts on boot/login:

```rust
// Install (launchd plist / systemd unit / Windows Service)
daemon.install_service().unwrap();

// Check if installed
if daemon.is_service_installed() {
    println!("Autostart enabled");
}

// Uninstall
daemon.uninstall_service().unwrap();
```

## Configuration

`DaemonConfig` supports builder-style configuration:

```rust
let config = DaemonConfig::new("my-daemon")
    .pid_dir("/var/run/my-daemon")       // PID file directory
    .log_file("/var/log/my-daemon.log")  // stdout/stderr redirect
    .executable("/usr/bin/my-daemon")    // binary path for service files
    .service_args(vec!["--foreground".into()]) // args for service manager
    .description("My daemon service");   // human-readable description
```

## PID file management

The `PidFile` type is also available standalone:

```rust
use daemon_kit::PidFile;

let pf = PidFile::new("/tmp/my-daemon.pid");
pf.write().unwrap();              // Write current PID
let pid = pf.read();              // Read PID (Option<u32>)
let alive = pf.alive_pid();      // Read + check if process alive
pf.remove();                     // Clean up
```

Stale PID files (process no longer running) are automatically cleaned up by `alive_pid()`.

## License

MIT
