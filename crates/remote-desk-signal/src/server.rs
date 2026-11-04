use crate::{RendezvousBroker, SignalServerError};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};

pub const MAX_MESSAGE_SIZE: usize = 128 * 1024; // 128 KB max signaling message
pub const DEFAULT_MAILBOX_CAPACITY: usize = 64;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum ClientSignalRequest {
    #[serde(rename = "register")]
    Register {
        device_id: String,
        timestamp_ms: u64,
        public_key_hex: String,
        #[serde(default)]
        signature_hex: Option<String>,
    },
    #[serde(rename = "route")]
    Route {
        target_id: String,
        envelope_json: String,
    },
    #[serde(rename = "poll")]
    Poll,
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerSignalResponse {
    #[serde(rename = "registered")]
    Registered { device_id: String },
    #[serde(rename = "routed")]
    Routed { target_id: String },
    #[serde(rename = "incoming")]
    Incoming { message: String },
    #[serde(rename = "pong")]
    Pong { timestamp_ms: u64 },
    #[serde(rename = "error")]
    Error { message: String },
}

pub struct SignalingServer {
    listener: TcpListener,
    broker: RendezvousBroker,
    active_peers: Arc<Mutex<HashMap<String, mpsc::Sender<Message>>>>,
    bind_addr: String,
}

impl SignalingServer {
    pub async fn bind(addr: &str, capacity: usize) -> Result<Self, SignalServerError> {
        let listener = TcpListener::bind(addr).await.map_err(|_e| {
            SignalServerError::CapacityExceeded // Or IO error wrapped
        })?;

        let local_addr = listener
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| addr.to_string());

        info!("SignalingServer bound to TCP socket {}", local_addr);

        Ok(Self {
            listener,
            broker: RendezvousBroker::new(capacity),
            active_peers: Arc::new(Mutex::new(HashMap::new())),
            bind_addr: local_addr,
        })
    }

    pub fn local_addr(&self) -> &str {
        &self.bind_addr
    }

    pub fn broker(&self) -> &RendezvousBroker {
        &self.broker
    }

    pub async fn run(
        self,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), SignalServerError> {
        let broker = self.broker;
        let active_peers = self.active_peers;

        loop {
            tokio::select! {
                accept_res = self.listener.accept() => {
                    match accept_res {
                        Ok((stream, peer_addr)) => {
                            let broker_clone = broker.clone();
                            let active_peers_clone = active_peers.clone();
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_connection(stream, peer_addr, broker_clone, active_peers_clone).await {
                                    debug!("Connection from {} closed: {:?}", peer_addr, e);
                                }
                            });
                        }
                        Err(e) => {
                            warn!("Accept error on TCP listener: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("SignalingServer received shutdown broadcast. Stopping listener loop.");
                    break;
                }
            }
        }

        // Notify and close active connections
        let mut peers = active_peers.lock().unwrap();
        for (peer_id, tx) in peers.drain() {
            let _ = tx.try_send(Message::Close(None));
            debug!("Sent close frame to peer {}", peer_id);
        }

        Ok(())
    }

    async fn handle_connection(
        mut stream: TcpStream,
        _peer_addr: SocketAddr,
        broker: RendezvousBroker,
        active_peers: Arc<Mutex<HashMap<String, mpsc::Sender<Message>>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Peek initial bytes to distinguish HTTP health probe from WebSocket handshake
        let mut peek_buf = [0u8; 1024];
        let bytes_peeked = stream.peek(&mut peek_buf).await?;
        let peek_str = String::from_utf8_lossy(&peek_buf[..bytes_peeked]);

        let is_http = peek_str.starts_with("GET ")
            || peek_str.starts_with("POST ")
            || peek_str.starts_with("HEAD ")
            || peek_str.starts_with("OPTIONS ");

        let is_websocket_upgrade = peek_str.to_lowercase().contains("upgrade: websocket");

        if is_http && !is_websocket_upgrade {
            let mut discard = vec![0u8; bytes_peeked.max(2048)];
            let _ = stream.read(&mut discard[..bytes_peeked]).await?;

            let first_line = peek_str.lines().next().unwrap_or("");
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            let raw_path = parts.get(1).copied().unwrap_or("/");
            let path = raw_path.split('?').next().unwrap_or("/");

            match path {
                "/health" | "/ready" => {
                    let peer_count = broker.registered_peer_count();
                    let body = format!(
                        "{{\"status\":\"ok\",\"service\":\"remote-desk-signal\",\"registered_peers\":{}}}\n",
                        peer_count
                    );
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    stream.write_all(response.as_bytes()).await?;
                    stream.flush().await?;
                    return Ok(());
                }
                "/api/info" | "/status" => {
                    let peer_count = broker.registered_peer_count();
                    let body = format!(
                        "{{\"status\":\"online\",\"service\":\"remote-desk-signal\",\"registered_peers\":{},\"capacity\":{}}}\n",
                        peer_count,
                        broker.capacity()
                    );
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    stream.write_all(response.as_bytes()).await?;
                    stream.flush().await?;
                    return Ok(());
                }
                "/favicon.ico" => {
                    let response = "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n";
                    stream.write_all(response.as_bytes()).await?;
                    stream.flush().await?;
                    return Ok(());
                }
                _ => {
                    let html = generate_signal_portal_html(&broker);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        html.len(),
                        html
                    );
                    stream.write_all(response.as_bytes()).await?;
                    stream.flush().await?;
                    return Ok(());
                }
            }
        }

        // Upgrade TCP stream to WebSocket (stream has not been read, handshake is intact)
        let ws_stream = tokio_tungstenite::accept_async(stream).await?;
        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        let (outbound_tx, mut outbound_rx) = mpsc::channel::<Message>(64);
        let mut registered_device_id: Option<String> = None;

        // Task to write outbound WebSocket messages to client
        let write_task = tokio::spawn(async move {
            while let Some(msg) = outbound_rx.recv().await {
                if ws_tx.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Read incoming WebSocket frames
        while let Some(msg_result) = ws_rx.next().await {
            let msg = match msg_result {
                Ok(m) => m,
                Err(_) => break,
            };

            match msg {
                Message::Text(text) => {
                    if text.len() > MAX_MESSAGE_SIZE {
                        let err_resp = ServerSignalResponse::Error {
                            message: "Message exceeds 128KB maximum limit".to_string(),
                        };
                        let _ = outbound_tx
                            .send(Message::Text(serde_json::to_string(&err_resp)?))
                            .await;
                        continue;
                    }

                    match serde_json::from_str::<ClientSignalRequest>(&text) {
                        Ok(ClientSignalRequest::Register {
                            device_id,
                            timestamp_ms,
                            public_key_hex,
                            signature_hex,
                        }) => {
                            // 1. Mandatory Ed25519 signature proof-of-possession
                            let sig_str = match signature_hex.as_deref() {
                                Some(s) if !s.trim().is_empty() => s.trim(),
                                _ => {
                                    let err_resp = ServerSignalResponse::Error {
                                        message: "Registration rejected: missing mandatory Ed25519 signature proof-of-possession".to_string(),
                                    };
                                    let _ = outbound_tx
                                        .send(Message::Text(serde_json::to_string(&err_resp)?))
                                        .await;
                                    continue;
                                }
                            };

                            // 2. Freshness check: max 30 seconds (30,000 ms) clock skew
                            let now_ms = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;
                            if timestamp_ms == 0 || now_ms.abs_diff(timestamp_ms) > 30_000 {
                                let err_resp = ServerSignalResponse::Error {
                                    message: format!(
                                        "Registration rejected: timestamp delta {}ms exceeds 30000ms freshness window",
                                        now_ms.abs_diff(timestamp_ms)
                                    ),
                                };
                                let _ = outbound_tx
                                    .send(Message::Text(serde_json::to_string(&err_resp)?))
                                    .await;
                                continue;
                            }

                            // 3. Decode public key and signature
                            let pubkey_bytes = decode_hex_exact::<32>(&public_key_hex);
                            let sig_bytes = decode_hex_exact::<64>(sig_str);

                            let (pk, sig) = match (pubkey_bytes, sig_bytes) {
                                (Some(pk), Some(sig)) => (pk, sig),
                                _ => {
                                    let err_resp = ServerSignalResponse::Error {
                                        message: "Registration rejected: invalid public key or signature hex encoding".to_string(),
                                    };
                                    let _ = outbound_tx
                                        .send(Message::Text(serde_json::to_string(&err_resp)?))
                                        .await;
                                    continue;
                                }
                            };

                            // 4. Validate device_id binds to public key
                            let derived_id = remote_desk_core::identity::DeviceIdentity::derive_device_id_from_bytes(&pk);
                            if device_id != derived_id {
                                let err_resp = ServerSignalResponse::Error {
                                    message: format!(
                                        "Registration rejected: claimed device_id '{}' does not match public key derived ID '{}'",
                                        device_id, derived_id
                                    ),
                                };
                                let _ = outbound_tx
                                    .send(Message::Text(serde_json::to_string(&err_resp)?))
                                    .await;
                                continue;
                            }

                            // 5. Cryptographic Proof-of-Possession check
                            let challenge = format!(
                                "RemoteDesk-Signaling-Register:{}:{}",
                                timestamp_ms, device_id
                            );

                            let verified =
                                if let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&pk) {
                                    let dalek_sig = ed25519_dalek::Signature::from_bytes(&sig);
                                    use ed25519_dalek::Verifier;
                                    vk.verify(challenge.as_bytes(), &dalek_sig).is_ok()
                                } else {
                                    false
                                };

                            if !verified {
                                let err_resp = ServerSignalResponse::Error {
                                    message: "Registration rejected: invalid cryptographic signature proof-of-possession".to_string(),
                                };
                                let _ = outbound_tx
                                    .send(Message::Text(serde_json::to_string(&err_resp)?))
                                    .await;
                                continue;
                            }

                            if let Err(e) = broker.register_peer(&device_id) {
                                let err_resp = ServerSignalResponse::Error {
                                    message: format!("Registration failed: {:?}", e),
                                };
                                let _ = outbound_tx
                                    .send(Message::Text(serde_json::to_string(&err_resp)?))
                                    .await;
                                continue;
                            }

                            registered_device_id = Some(device_id.clone());
                            {
                                let mut peers = active_peers.lock().unwrap();
                                peers.insert(device_id.clone(), outbound_tx.clone());
                            }

                            info!("Peer registered on WebSocket signaling: {}", device_id);
                            let resp = ServerSignalResponse::Registered { device_id };
                            let _ = outbound_tx
                                .send(Message::Text(serde_json::to_string(&resp)?))
                                .await;
                        }
                        Ok(ClientSignalRequest::Route {
                            target_id,
                            envelope_json,
                        }) => {
                            let sender_id = match registered_device_id.as_deref() {
                                Some(id) => id,
                                None => {
                                    let err_resp = ServerSignalResponse::Error {
                                        message: "Unauthenticated: Must register device before routing messages".to_string(),
                                    };
                                    let _ = outbound_tx
                                        .send(Message::Text(serde_json::to_string(&err_resp)?))
                                        .await;
                                    continue;
                                }
                            };

                            // Check rate limit on sender
                            if !broker.check_rate_limit(sender_id) {
                                let err_resp = ServerSignalResponse::Error {
                                    message: "Route failed: RateLimitExceeded".to_string(),
                                };
                                let _ = outbound_tx
                                    .send(Message::Text(serde_json::to_string(&err_resp)?))
                                    .await;
                                continue;
                            }

                            // Verify claimed sender identity in envelope
                            if let Ok(val) =
                                serde_json::from_str::<serde_json::Value>(&envelope_json)
                            {
                                if let Some(claimed_sender) =
                                    val.get("sender_device_id").and_then(|s| s.as_str())
                                {
                                    if claimed_sender != sender_id {
                                        let err_resp = ServerSignalResponse::Error {
                                            message: "Route rejected: envelope sender does not match authenticated peer ID".to_string(),
                                        };
                                        let _ = outbound_tx
                                            .send(Message::Text(serde_json::to_string(&err_resp)?))
                                            .await;
                                        continue;
                                    }
                                }
                            }

                            // If target peer is currently connected live, forward immediately via bounded channel
                            let forwarded = {
                                let peers = active_peers.lock().unwrap();
                                if let Some(target_tx) = peers.get(&target_id) {
                                    let incoming = ServerSignalResponse::Incoming {
                                        message: envelope_json.clone(),
                                    };
                                    if let Ok(json_str) = serde_json::to_string(&incoming) {
                                        target_tx.try_send(Message::Text(json_str)).is_ok()
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            };

                            if !forwarded {
                                // Store in target's mailbox queue
                                if let Err(e) = broker.route_message(
                                    sender_id,
                                    &target_id,
                                    envelope_json.into_bytes(),
                                ) {
                                    let err_resp = ServerSignalResponse::Error {
                                        message: format!("Route failed: {:?}", e),
                                    };
                                    let _ = outbound_tx
                                        .send(Message::Text(serde_json::to_string(&err_resp)?))
                                        .await;
                                    continue;
                                }
                            }

                            let resp = ServerSignalResponse::Routed { target_id };
                            let _ = outbound_tx
                                .send(Message::Text(serde_json::to_string(&resp)?))
                                .await;
                        }
                        Ok(ClientSignalRequest::Poll) => {
                            if let Some(ref device_id) = registered_device_id {
                                let messages = broker.poll_messages(device_id);
                                for msg_bytes in messages {
                                    if let Ok(msg_str) = String::from_utf8(msg_bytes) {
                                        let incoming =
                                            ServerSignalResponse::Incoming { message: msg_str };
                                        let _ = outbound_tx
                                            .send(Message::Text(serde_json::to_string(&incoming)?))
                                            .await;
                                    }
                                }
                            }
                        }
                        Ok(ClientSignalRequest::Ping) => {
                            let now_ms = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;
                            let resp = ServerSignalResponse::Pong {
                                timestamp_ms: now_ms,
                            };
                            let _ = outbound_tx
                                .send(Message::Text(serde_json::to_string(&resp)?))
                                .await;
                        }
                        Err(e) => {
                            let err_resp = ServerSignalResponse::Error {
                                message: format!("Invalid JSON request payload: {}", e),
                            };
                            let _ = outbound_tx
                                .send(Message::Text(serde_json::to_string(&err_resp)?))
                                .await;
                        }
                    }
                }
                Message::Ping(data) => {
                    let _ = outbound_tx.send(Message::Pong(data)).await;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        // Cleanup on disconnect
        if let Some(ref device_id) = registered_device_id {
            let mut peers = active_peers.lock().unwrap();
            peers.remove(device_id);
            broker.unregister_peer(device_id);
            debug!("Peer unregistered on WebSocket disconnect: {}", device_id);
        }

        let _ = write_task.await;
        Ok(())
    }
}

fn decode_hex_exact<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn generate_signal_portal_html(broker: &RendezvousBroker) -> String {
    let peer_count = broker.registered_peer_count();
    let capacity = broker.capacity();

    format!(
        r###"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Remote-Desk Rendezvous Broker &bull; Signaling Node</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap">
    <style>
        :root {{
            --bg-base: #08090c;
            --bg-surface: #0e1118;
            --bg-card: #131722;
            --border: rgba(255, 255, 255, 0.08);
            --border-hover: rgba(255, 255, 255, 0.16);
            --text-primary: #f8fafc;
            --text-secondary: #94a3b8;
            --text-muted: #64748b;
            --accent: #38bdf8;
            --emerald: #10b981;
            --font-sans: 'IBM Plex Sans', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            --font-mono: 'JetBrains Mono', SFMono-Regular, Menlo, Monaco, Consolas, monospace;
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            font-family: var(--font-sans);
            background: var(--bg-base);
            color: var(--text-primary);
            line-height: 1.6;
            min-height: 100vh;
            display: flex;
            flex-direction: column;
            align-items: center;
            padding: 2.5rem 1rem;
            -webkit-font-smoothing: antialiased;
        }}
        .container {{ width: 100%; max-width: 880px; }}
        header {{
            display: flex;
            flex-direction: column;
            align-items: center;
            text-align: center;
            margin-bottom: 2rem;
            gap: 0.75rem;
        }}
        .header-top {{
            display: inline-flex;
            align-items: center;
            gap: 0.75rem;
        }}
        .logo-mark {{
            width: 38px;
            height: 38px;
            background: linear-gradient(135deg, #1e293b 0%, #0f172a 100%);
            border: 1px solid rgba(56, 189, 248, 0.3);
            border-radius: 10px;
            display: flex;
            align-items: center;
            justify-content: center;
            box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
        }}
        .status-pill {{
            display: inline-flex;
            align-items: center;
            gap: 0.5rem;
            background: rgba(16, 185, 129, 0.12);
            color: var(--emerald);
            border: 1px solid rgba(16, 185, 129, 0.28);
            border-radius: 9999px;
            padding: 0.35rem 0.85rem;
            font-size: 0.775rem;
            font-weight: 600;
            letter-spacing: 0.05em;
            text-transform: uppercase;
        }}
        .status-dot {{
            width: 7px;
            height: 7px;
            background: var(--emerald);
            border-radius: 50%;
            box-shadow: 0 0 8px var(--emerald);
        }}
        h1 {{
            font-size: 2rem;
            font-weight: 700;
            letter-spacing: -0.025em;
            color: #ffffff;
            margin-top: 0.25rem;
        }}
        .subtitle {{
            color: var(--text-secondary);
            font-size: 0.95rem;
            max-width: 600px;
        }}
        .grid {{
            display: grid;
            grid-template-columns: 1fr;
            gap: 1.5rem;
            margin-bottom: 1.75rem;
        }}
        @media (min-width: 680px) {{
            .grid {{ grid-template-columns: 1fr 1fr; }}
            .grid-full {{ grid-column: span 2; }}
        }}
        .card {{
            background: var(--bg-card);
            border: 1px solid var(--border);
            border-radius: 14px;
            padding: 1.5rem;
            box-shadow: 0 4px 20px -2px rgba(0, 0, 0, 0.4);
            display: flex;
            flex-direction: column;
        }}
        .card-header {{
            display: flex;
            align-items: center;
            justify-content: space-between;
            margin-bottom: 1.25rem;
            padding-bottom: 0.75rem;
            border-bottom: 1px solid var(--border);
        }}
        .card-title {{
            font-size: 1.05rem;
            font-weight: 600;
            color: #ffffff;
            letter-spacing: -0.01em;
            display: flex;
            align-items: center;
            gap: 0.5rem;
        }}
        .field {{ margin-bottom: 1rem; }}
        .field:last-child {{ margin-bottom: 0; }}
        .field-label {{
            font-size: 0.725rem;
            text-transform: uppercase;
            letter-spacing: 0.06em;
            color: var(--text-muted);
            margin-bottom: 0.35rem;
            font-weight: 600;
        }}
        .field-value {{
            font-size: 0.95rem;
            font-weight: 500;
            color: var(--text-primary);
        }}
        .mono {{
            font-family: var(--font-mono);
            background: #080a0f;
            border: 1px solid var(--border);
            padding: 0.3rem 0.6rem;
            border-radius: 6px;
            font-size: 0.85rem;
            color: var(--accent);
            display: inline-block;
        }}
        .hud-grid {{
            display: grid;
            grid-template-columns: repeat(2, 1fr);
            gap: 0.75rem;
            margin-bottom: 1.25rem;
        }}
        @media (min-width: 640px) {{
            .hud-grid {{ grid-template-columns: repeat(4, 1fr); }}
        }}
        .hud-metric {{
            background: #080a0f;
            border: 1px solid var(--border);
            border-radius: 10px;
            padding: 0.85rem 1rem;
        }}
        .hud-label {{
            font-size: 0.7rem;
            text-transform: uppercase;
            letter-spacing: 0.06em;
            color: var(--text-muted);
            margin-bottom: 0.25rem;
            font-weight: 600;
        }}
        .hud-value {{
            font-family: var(--font-mono);
            font-size: 1.15rem;
            font-weight: 700;
            color: #ffffff;
            font-variant-numeric: tabular-nums;
        }}
        .guide-step {{
            display: flex;
            gap: 1rem;
            margin-bottom: 1rem;
            align-items: flex-start;
        }}
        .guide-step:last-child {{ margin-bottom: 0; }}
        .step-num {{
            background: rgba(56, 189, 248, 0.15);
            color: var(--accent);
            border: 1px solid var(--accent);
            width: 26px;
            height: 26px;
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-weight: 700;
            font-size: 0.8rem;
            flex-shrink: 0;
            margin-top: 2px;
        }}
        .step-text {{ color: var(--text-secondary); font-size: 0.925rem; }}
        .step-text strong {{ color: var(--text-primary); }}
        .link {{ color: var(--accent); text-decoration: none; font-weight: 500; }}
        .link:hover {{ text-decoration: underline; }}
        footer {{
            text-align: center;
            color: var(--text-muted);
            font-size: 0.825rem;
            border-top: 1px solid var(--border);
            padding-top: 1.5rem;
            width: 100%;
            margin-top: 1rem;
        }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <div class="header-top">
                <div class="logo-mark">
                    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#38bdf8" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <circle cx="12" cy="12" r="10"></circle>
                        <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"></path>
                        <path d="M2 12h20"></path>
                    </svg>
                </div>
                <div class="status-pill">
                    <span class="status-dot"></span>
                    RENDEZVOUS BROKER ONLINE
                </div>
            </div>
            <h1>Remote-Desk Rendezvous Broker</h1>
            <p class="subtitle">Ephemeral In-Memory WebRTC Signaling Broker & Zero-Knowledge Peer Discovery Node</p>
        </header>

        <div class="grid">
            <div class="card">
                <div class="card-header">
                    <div class="card-title">Broker Status & Telemetry</div>
                </div>
                <div class="hud-grid" style="grid-template-columns: 1fr 1fr; margin-bottom: 0.75rem;">
                    <div class="hud-metric">
                        <div class="hud-label">REGISTERED PEERS</div>
                        <div class="hud-value">{peer_count} active</div>
                    </div>
                    <div class="hud-metric">
                        <div class="hud-label">ENDPOINT CAPACITY</div>
                        <div class="hud-value">{capacity} max</div>
                    </div>
                </div>
                <div class="field">
                    <div class="field-label">Storage Architecture</div>
                    <div class="field-value">Zero-Knowledge In-Memory Store (Zero Disk Logs)</div>
                </div>
                <div class="field">
                    <div class="field-label">Cryptographic Verification</div>
                    <div class="field-value">Ed25519 Proof-of-Possession Challenge Handshake</div>
                </div>
            </div>

            <div class="card">
                <div class="card-header">
                    <div class="card-title">Protocols & Interfaces</div>
                </div>
                <div class="field">
                    <div class="field-label">WebSocket Rendezvous URL</div>
                    <div class="mono">ws://HOST:PORT/</div>
                </div>
                <div class="field">
                    <div class="field-label">Wire Protocol</div>
                    <div class="field-value">JSON Envelopes over RFC 6455 Frames</div>
                </div>
                <div class="field">
                    <div class="field-label">Health & Probes</div>
                    <div class="field-value">
                        <a href="/health" class="link">/health</a> &bull; 
                        <a href="/ready" class="link">/ready</a> &bull; 
                        <a href="/api/info" class="link">/api/info</a>
                    </div>
                </div>
                <div class="field">
                    <div class="field-label">Anti-Abuse Rate Limiting</div>
                    <div class="field-value">Token Bucket (5 req/s burst tolerance per device ID)</div>
                </div>
            </div>

            <div class="card grid-full">
                <div class="card-header">
                    <div class="card-title">Host Configuration Guide</div>
                </div>
                <div class="guide-step">
                    <div class="step-num">1</div>
                    <div class="step-text">
                        Launch your host daemon pointing to this rendezvous broker:
                        <br/>
                        <code class="mono" style="margin-top: 0.5rem; display: block;">cargo run -p remote-desk-host -- --signal-server ws://THIS_IP:PORT/</code>
                    </div>
                </div>
                <div class="guide-step">
                    <div class="step-num">2</div>
                    <div class="step-text">
                        The host daemon automatically registers its Ed25519 public key and publishes presence.
                    </div>
                </div>
                <div class="guide-step">
                    <div class="step-num">3</div>
                    <div class="step-text">
                        Remote controllers connect to the same broker and route end-to-end encrypted SDP offers and answers.
                    </div>
                </div>
            </div>
        </div>

        <footer>
            Remote-Desk Rendezvous Broker v0.1.0 &bull; Ephemeral Security &bull; Zero Central Database
        </footer>
    </div>
</body>
</html>"###,
        peer_count = peer_count,
        capacity = capacity
    )
}
