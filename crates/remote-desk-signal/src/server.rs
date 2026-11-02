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
    active_peers: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Message>>>>,
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
            let _ = tx.send(Message::Close(None));
            debug!("Sent close frame to peer {}", peer_id);
        }

        Ok(())
    }

    async fn handle_connection(
        mut stream: TcpStream,
        _peer_addr: SocketAddr,
        broker: RendezvousBroker,
        active_peers: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Message>>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Peek initial bytes to distinguish HTTP health probe from WebSocket handshake
        let mut peek_buf = [0u8; 256];
        let bytes_peeked = stream.peek(&mut peek_buf).await?;
        let peek_str = String::from_utf8_lossy(&peek_buf[..bytes_peeked]);

        if peek_str.starts_with("GET /health") || peek_str.starts_with("GET /ready") {
            let mut discard = [0u8; 1024];
            let _ = stream.read(&mut discard).await?;

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

        // Upgrade TCP stream to WebSocket (stream has not been read, handshake is intact)
        let ws_stream = tokio_tungstenite::accept_async(stream).await?;
        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<Message>();
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
                        let _ = outbound_tx.send(Message::Text(serde_json::to_string(&err_resp)?));
                        continue;
                    }

                    match serde_json::from_str::<ClientSignalRequest>(&text) {
                        Ok(ClientSignalRequest::Register {
                            device_id,
                            timestamp_ms,
                            public_key_hex,
                            signature_hex,
                        }) => {
                            // 1. Clock skew check: max 5 minutes (300,000 ms) drift
                            let now_ms = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;
                            if timestamp_ms > 0 && now_ms.abs_diff(timestamp_ms) > 300_000 {
                                let err_resp = ServerSignalResponse::Error {
                                    message: format!(
                                        "Registration rejected: timestamp drift too large ({}ms vs server {}ms)",
                                        timestamp_ms, now_ms
                                    ),
                                };
                                let _ = outbound_tx
                                    .send(Message::Text(serde_json::to_string(&err_resp)?));
                                continue;
                            }

                            // 2. Cryptographic Proof-of-Possession check
                            if let Some(ref sig_str) = signature_hex {
                                let challenge = format!(
                                    "RemoteDesk-Signaling-Register:{}:{}",
                                    timestamp_ms, device_id
                                );
                                let pubkey_bytes = decode_hex_exact::<32>(&public_key_hex);
                                let sig_bytes = decode_hex_exact::<64>(sig_str);

                                let verified = match (pubkey_bytes, sig_bytes) {
                                    (Some(pk), Some(sig)) => {
                                        if let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&pk)
                                        {
                                            let dalek_sig =
                                                ed25519_dalek::Signature::from_bytes(&sig);
                                            use ed25519_dalek::Verifier;
                                            vk.verify(challenge.as_bytes(), &dalek_sig).is_ok()
                                        } else {
                                            false
                                        }
                                    }
                                    _ => false,
                                };

                                if !verified {
                                    let err_resp = ServerSignalResponse::Error {
                                        message: "Registration rejected: invalid cryptographic signature proof-of-possession".to_string(),
                                    };
                                    let _ = outbound_tx
                                        .send(Message::Text(serde_json::to_string(&err_resp)?));
                                    continue;
                                }
                            }

                            if let Err(e) = broker.register_peer(&device_id) {
                                let err_resp = ServerSignalResponse::Error {
                                    message: format!("Registration failed: {:?}", e),
                                };
                                let _ = outbound_tx
                                    .send(Message::Text(serde_json::to_string(&err_resp)?));
                                continue;
                            }

                            registered_device_id = Some(device_id.clone());
                            {
                                let mut peers = active_peers.lock().unwrap();
                                peers.insert(device_id.clone(), outbound_tx.clone());
                            }

                            info!("Peer registered on WebSocket signaling: {}", device_id);
                            let resp = ServerSignalResponse::Registered { device_id };
                            let _ = outbound_tx.send(Message::Text(serde_json::to_string(&resp)?));
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
                                        .send(Message::Text(serde_json::to_string(&err_resp)?));
                                    continue;
                                }
                            };

                            // If target peer is currently connected live, forward immediately
                            let forwarded = {
                                let peers = active_peers.lock().unwrap();
                                if let Some(target_tx) = peers.get(&target_id) {
                                    let incoming = ServerSignalResponse::Incoming {
                                        message: envelope_json.clone(),
                                    };
                                    if let Ok(json_str) = serde_json::to_string(&incoming) {
                                        target_tx.send(Message::Text(json_str)).is_ok()
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
                                        .send(Message::Text(serde_json::to_string(&err_resp)?));
                                    continue;
                                }
                            }

                            let resp = ServerSignalResponse::Routed { target_id };
                            let _ = outbound_tx.send(Message::Text(serde_json::to_string(&resp)?));
                        }
                        Ok(ClientSignalRequest::Poll) => {
                            if let Some(ref device_id) = registered_device_id {
                                let messages = broker.poll_messages(device_id);
                                for msg_bytes in messages {
                                    if let Ok(msg_str) = String::from_utf8(msg_bytes) {
                                        let incoming =
                                            ServerSignalResponse::Incoming { message: msg_str };
                                        let _ = outbound_tx
                                            .send(Message::Text(serde_json::to_string(&incoming)?));
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
                            let _ = outbound_tx.send(Message::Text(serde_json::to_string(&resp)?));
                        }
                        Err(e) => {
                            let err_resp = ServerSignalResponse::Error {
                                message: format!("Invalid JSON request payload: {}", e),
                            };
                            let _ =
                                outbound_tx.send(Message::Text(serde_json::to_string(&err_resp)?));
                        }
                    }
                }
                Message::Ping(data) => {
                    let _ = outbound_tx.send(Message::Pong(data));
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
