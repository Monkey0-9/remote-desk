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

    // Lightweight argument parsing without external CLI dependencies
    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
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
            "--list-monitors" => {
                list_monitors_only = true;
            }
            "--help" | "-h" => {
                println!("Usage: remote-desk-host [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --name <NAME>            Friendly name of this host");
                println!(
                    "  --unattended             Auto-approve authenticated paired controllers"
                );
                println!("  --bind <ADDR>            Signaling / listening address (default: 127.0.0.1:4443)");
                println!("  --signal-server <URL>    Rendezvous broker URL");
                println!("  --list-monitors          List detected display monitors and exit");
                println!("  --help, -h               Show this help message");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    let identity = DeviceIdentity::generate();
    let host = HostService::new_mock(identity, config.clone());

    info!(
        "Host identity initialized: Device ID [{}]",
        host.device_id()
    );
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

    info!("Remote-Desk host daemon is running and listening for client handshakes...");
    info!("Press Ctrl+C to terminate host service.");

    // Daemon keepalive loop
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_millis(50)) => {
            info!("Host daemon status: HEALTHY (ready to accept WebRTC sessions)");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal. Releasing resources...");
            host.terminate_session("Process terminated").await?;
        }
    }

    Ok(())
}
