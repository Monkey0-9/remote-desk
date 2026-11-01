use remote_desk_core::DeviceIdentity;
use remote_desk_host::{HostConfig, HostService};
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();

    println!("============================================================");
    println!("        REMOTE-DESK SECURE HOST STREAMING DAEMON           ");
    println!("============================================================");

    let mut config = HostConfig::default();
    let mut list_monitors_only = false;
    let mut health_check_only = false;
    let mut duration_ms: Option<u64> = None;
    let mut backend_choice = "auto".to_string();
    let mut _display: Option<String> = None;
    let mut _session_portal = false;
    let mut custom_key_path: Option<PathBuf> = None;

    // Lightweight argument parsing without external CLI dependencies
    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--backend" => {
                if i + 1 < args.len() {
                    backend_choice = args[i + 1].to_lowercase();
                    i += 1;
                }
            }
            "--name" => {
                if i + 1 < args.len() {
                    config.device_name = args[i + 1].clone();
                    i += 1;
                }
            }
            "--unattended" => {
                config.unattended = true;
            }
            "--bind" => {
                if i + 1 < args.len() {
                    config.bind_addr = args[i + 1].clone();
                    i += 1;
                }
            }
            "--signal-server" => {
                if i + 1 < args.len() {
                    config.signal_server_url = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--key" => {
                if i + 1 < args.len() {
                    custom_key_path = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--display" => {
                if i + 1 < args.len() {
                    _display = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--session-portal" => {
                _session_portal = true;
            }
            "--list-monitors" => {
                list_monitors_only = true;
            }
            "--health-check" => {
                health_check_only = true;
            }
            "--duration-ms" => {
                if i + 1 < args.len() {
                    if let Ok(ms) = args[i + 1].parse::<u64>() {
                        duration_ms = Some(ms);
                    }
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!("Usage: remote-desk-host [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --backend <TYPE>         Capture & input backend (auto, linux, windows, mock; default: auto)");
                println!("  --name <NAME>            Friendly name of this host");
                println!(
                    "  --unattended             Auto-approve authenticated paired controllers"
                );
                println!("  --bind <ADDR>            Signaling / listening address (default: 127.0.0.1:4443)");
                println!("  --signal-server <URL>    Rendezvous broker URL");
                println!("  --key <PATH>             Path to persistent private identity key (default: ~/.config/remote-desk/identity.key)");
                println!("  --display <DISPLAY>      Target display identifier (e.g. :0)");
                println!(
                    "  --session-portal         Use Wayland XDG Desktop Portal ScreenCast session"
                );
                println!("  --list-monitors          List detected display monitors and exit");
                println!("  --health-check           Check daemon readiness and exit immediately");
                println!(
                    "  --duration-ms <MS>       Run daemon for specified milliseconds then exit"
                );
                println!("  --help, -h               Show this help message");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    // Determine persistent identity path
    let default_key_path = {
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".config")
            .join("remote-desk")
            .join("identity.key")
    };
    let key_file = custom_key_path.unwrap_or(default_key_path);

    let identity = DeviceIdentity::load_or_generate(&key_file)?;
    info!(
        "Persistent identity active from [{}], fingerprint: {}",
        key_file.display(),
        identity.fingerprint_hex()
    );

    let host = match backend_choice.as_str() {
        "linux" => HostService::new_linux(identity, config.clone())?,
        "windows" => HostService::new_windows(identity, config.clone()),
        "mock" => HostService::new_mock(identity, config.clone()),
        _ => match HostService::new_auto(identity.clone(), config.clone()) {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(
                    "Production native backend initialization failed: {}. To run simulation pass --backend mock.",
                    e
                );
                return Err(Box::new(e) as Box<dyn std::error::Error>);
            }
        },
    };

    info!(
        "Host identity initialized: Device ID [{}]",
        host.device_id()
    );
    info!("Backend Driver:  '{}'", backend_choice);
    info!("Host Name:       '{}'", config.device_name);
    info!("Unattended Mode: {}", config.unattended);
    info!("Bind Address:    '{}'", config.bind_addr);

    let monitors = host.enumerate_monitors().await?;
    info!("Detected {} display output(s):", monitors.len());
    for m in &monitors {
        info!(
            "  - Output #{}: {} ({}x{} @ {}Hz, primary: {})",
            m.id, m.name, m.width, m.height, m.refresh_rate, m.is_primary
        );
    }

    if list_monitors_only {
        return Ok(());
    }

    if health_check_only {
        info!("Host daemon status: HEALTHY (ready to accept WebRTC sessions)");
        return Ok(());
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);
    let host_arc = std::sync::Arc::new(host);
    let host_daemon = host_arc.clone();

    let daemon_task = tokio::spawn(async move {
        if let Err(e) = host_daemon.run_daemon(shutdown_rx).await {
            tracing::warn!("Host daemon listener ended with: {:?}", e);
        }
    });

    info!("Remote-Desk host daemon is running and listening for client handshakes...");
    info!("Press Ctrl+C to terminate host service.");

    if let Some(ms) = duration_ms {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(ms)) => {
                info!("Duration limit of {}ms reached. Shutting down Remote-Desk host...", ms);
                let _ = shutdown_tx.send(());
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal. Releasing resources...");
                let _ = shutdown_tx.send(());
            }
        }
    } else {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal. Releasing resources...");
                let _ = shutdown_tx.send(());
            }
        }
    }

    let _ = daemon_task.await;
    info!("Host daemon stopped cleanly.");
    Ok(())
}
