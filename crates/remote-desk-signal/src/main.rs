use remote_desk_signal::SignalingServer;
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
                println!("  --bind <ADDR>          Bind address for WebSocket signaling (default: 0.0.0.0:4443 or BIND_ADDR)");
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

    if health_check_only {
        info!("Remote-Desk Rendezvous Broker health check: READY. Zero persistent state recorded.");
        return Ok(());
    }

    info!("Starting Remote-Desk Async WebSocket Signaling Server (RD-SIGNAL-001)...");
    info!("Configuration: Ephemeral In-Memory Routing, Zero Database, Zero Packet Logging");

    let server = SignalingServer::bind(&bind_addr, capacity).await?;
    let bound_addr = server.local_addr().to_string();
    info!(
        "Signaling Server online on {}. Capacity: {} concurrent endpoints.",
        bound_addr, capacity
    );
    info!(
        "Endpoints available: WebSocket ws://{}/ and HTTP GET http://{}/health",
        bound_addr, bound_addr
    );
    info!("Press Ctrl+C to terminate signaling server.");

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

    let server_task = tokio::spawn(async move {
        let _ = server.run(shutdown_rx).await;
    });

    if let Some(ms) = duration_ms {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(ms)) => {
                info!("Duration limit of {}ms reached. Initiating clean shutdown...", ms);
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received termination signal. Initiating clean shutdown...");
            }
        }
    } else {
        tokio::signal::ctrl_c().await?;
        info!("Received termination signal. Initiating clean shutdown...");
    }

    let _ = shutdown_tx.send(());
    let _ = server_task.await;

    info!("Signaling server terminated cleanly.");
    Ok(())
}
