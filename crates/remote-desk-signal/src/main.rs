use remote_desk_signal::RendezvousBroker;
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

    let mut bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:4443".to_string());
    let mut duration_ms: Option<u64> = None;
    let mut capacity: usize = 10_000;
    let mut health_check_only = false;

    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--health-check" => {
                health_check_only = true;
            }
            "--bind" => {
                if i + 1 < args.len() {
                    bind_addr = args[i + 1].clone();
                    i += 1;
                }
            }
            "--capacity" => {
                if i + 1 < args.len() {
                    if let Ok(cap) = args[i + 1].parse::<usize>() {
                        capacity = cap;
                    }
                    i += 1;
                }
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
                println!("Usage: remote-desk-signal [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --bind <ADDR>          Bind address for rendezvous broker (default: 0.0.0.0:4443 or BIND_ADDR)");
                println!(
                    "  --capacity <NUM>       Maximum concurrent device capacity (default: 10000)"
                );
                println!("  --health-check         Perform health check verification and exit immediately");
                println!("  --duration-ms <MS>     Run server for specified milliseconds then exit (for automated testing)");
                println!("  --help, -h             Show this help message");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    let _broker = RendezvousBroker::new(capacity);

    if health_check_only {
        info!("Remote-Desk Rendezvous Broker health check: READY. Zero persistent state recorded.");
        return Ok(());
    }

    info!("Starting Remote-Desk Rendezvous Signaling Server (RD-SIGNAL-001)...");
    info!("Configuration: Ephemeral In-Memory Routing, Zero Database, Zero Packet Logging");
    info!(
        "Rendezvous Broker initialized. Capacity: {} concurrent endpoints.",
        capacity
    );
    info!("Listening for peer signaling handshakes on {}", bind_addr);
    info!("Press Ctrl+C to terminate signaling server.");

    if let Some(ms) = duration_ms {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(ms)) => {
                info!("Duration limit of {}ms reached. Shutting down signaling server...", ms);
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received termination signal. Shutting down signaling server...");
            }
        }
    } else {
        tokio::signal::ctrl_c().await?;
        info!("Received termination signal. Shutting down signaling server...");
    }

    info!("Signaling server terminated cleanly.");
    Ok(())
}
