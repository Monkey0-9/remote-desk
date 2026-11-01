use remote_desk_core::DeviceIdentity;
use remote_desk_host::{HostConfig, HostService};
use std::env;
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

    let identity = DeviceIdentity::generate();
    let host = match backend_choice.as_str() {
        "linux" => HostService::new_linux(identity, config.clone()),
        "windows" => HostService::new_windows(identity, config.clone()),
        "mock" => HostService::new_mock(identity, config.clone()),
        _ => HostService::new_auto(identity, config.clone()),
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

    info!("Remote-Desk host daemon is running and listening for client handshakes...");
    info!("Press Ctrl+C to terminate host service.");

    if let Some(ms) = duration_ms {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(ms)) => {
                info!("Duration limit of {}ms reached. Shutting down Remote-Desk host...", ms);
                host.terminate_session("Duration limit reached").await?;
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal. Releasing resources...");
                host.terminate_session("Process terminated").await?;
            }
        }
    } else {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal. Releasing resources...");
                host.terminate_session("Process terminated").await?;
            }
        }
    }

    info!("Host daemon stopped cleanly.");
    Ok(())
}
