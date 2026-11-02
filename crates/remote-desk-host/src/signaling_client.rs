use futures_util::{SinkExt, StreamExt};
use remote_desk_core::identity::DeviceIdentity;
use remote_desk_net::signaling::{SignalingEnvelope, SignalingPayload};
use remote_desk_signal::server::{ClientSignalRequest, ServerSignalResponse};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};

#[derive(Error, Debug)]
pub enum SignalingClientError {
    #[error("WebSocket connection error: {0}")]
    ConnectionError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Server returned error: {0}")]
    ServerError(String),
    #[error("Signaling client closed")]
    Closed,
}

pub struct SignalingClient {
    identity: DeviceIdentity,
    signal_server_url: String,
    is_connected: Arc<AtomicBool>,
}

impl SignalingClient {
    pub fn new(identity: DeviceIdentity, signal_server_url: String) -> Self {
        Self {
            identity,
            signal_server_url,
            is_connected: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }

    /// Connects to the signaling rendezvous server, registers this device,
    /// and listens for incoming signaling envelopes from controllers.
    pub async fn run_loop(
        &self,
        incoming_tx: mpsc::Sender<SignalingEnvelope<SignalingPayload>>,
        mut outgoing_rx: mpsc::Receiver<(String, SignalingEnvelope<SignalingPayload>)>,
        mut stop_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), SignalingClientError> {
        info!(
            "Connecting host signaling client to {}",
            self.signal_server_url
        );

        let (ws_stream, _) = connect_async(&self.signal_server_url)
            .await
            .map_err(|e| SignalingClientError::ConnectionError(e.to_string()))?;

        let (mut write, mut read) = ws_stream.split();
        self.is_connected.store(true, Ordering::SeqCst);

        // 1. Send Register Request with Proof-of-Possession Ed25519 signature
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let challenge = format!(
            "RemoteDesk-Signaling-Register:{}:{}",
            now_ms,
            self.identity.device_id()
        );
        let sig_bytes = self.identity.sign(challenge.as_bytes());
        let sig_hex = hex_encode(&sig_bytes);

        let reg_req = ClientSignalRequest::Register {
            device_id: self.identity.device_id(),
            timestamp_ms: now_ms,
            public_key_hex: self.identity.public_key_hex(),
            signature_hex: Some(sig_hex),
        };

        let reg_json = serde_json::to_string(&reg_req)
            .map_err(|e| SignalingClientError::SerializationError(e.to_string()))?;

        write
            .send(Message::Text(reg_json))
            .await
            .map_err(|e| SignalingClientError::ConnectionError(e.to_string()))?;

        info!(
            "Host device [{}] registered with signaling rendezvous server",
            self.identity.device_id()
        );

        loop {
            tokio::select! {
                _ = stop_rx.recv() => {
                    debug!("Signaling client received stop signal");
                    break;
                }
                msg_opt = read.next() => {
                    match msg_opt {
                        Some(Ok(Message::Text(text))) => {
                            if let Ok(resp) = serde_json::from_str::<ServerSignalResponse>(&text) {
                                match resp {
                                    ServerSignalResponse::Registered { device_id } => {
                                        debug!("Registration confirmed for device {}", device_id);
                                    }
                                    ServerSignalResponse::Incoming { message } => {
                                        if let Ok(envelope) = serde_json::from_str::<SignalingEnvelope<SignalingPayload>>(&message) {
                                            let now = SystemTime::now()
                                                .duration_since(UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_millis() as u64;
                                            if envelope.verify_signature(now).is_ok() {
                                                let _ = incoming_tx.send(envelope).await;
                                            } else {
                                                warn!("Received incoming signaling envelope with invalid signature; discarding");
                                            }
                                        }
                                    }
                                    ServerSignalResponse::Error { message } => {
                                        warn!("Signaling server reported error: {}", message);
                                    }
                                    ServerSignalResponse::Pong { .. } | ServerSignalResponse::Routed { .. } => {}
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            info!("Signaling connection closed by server");
                            break;
                        }
                        Some(Err(e)) => {
                            warn!("Signaling stream error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
                Some((target_id, envelope)) = outgoing_rx.recv() => {
                    if let Ok(envelope_json) = serde_json::to_string(&envelope) {
                        let route_req = ClientSignalRequest::Route {
                            target_id,
                            envelope_json,
                        };
                        if let Ok(req_json) = serde_json::to_string(&route_req) {
                            let _ = write.send(Message::Text(req_json)).await;
                        }
                    }
                }
            }
        }

        self.is_connected.store(false, Ordering::SeqCst);
        Ok(())
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX_CHARS[(b >> 4) as usize] as char);
        s.push(HEX_CHARS[(b & 0x0F) as usize] as char);
    }
    s
}
