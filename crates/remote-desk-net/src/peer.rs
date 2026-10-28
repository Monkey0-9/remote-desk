use async_trait::async_trait;
use remote_desk_proto::{pack_frame, ProtoError, RemoteDeskMessage};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Error, Debug)]
pub enum PeerError {
    #[error("Protocol packaging error: {0}")]
    Proto(#[from] ProtoError),
    #[error("Channel closed")]
    ChannelClosed,
    #[error("WebRTC internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerConnectionState {
    New,
    Connecting,
    Connected,
    Disconnected,
    Failed,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataChannelRole {
    /// Ordered, reliable channel for topology, session management, clipboard
    Control,
    /// Unordered, low-latency channel for high-frequency pointer movements
    Input,
}

#[async_trait]
pub trait TransportChannel: Send + Sync {
    async fn send_raw(&self, data: &[u8]) -> Result<(), PeerError>;
    async fn close(&self) -> Result<(), PeerError>;
}

/// In-memory mock transport channel for loopback testing and CI
pub struct MockTransportChannel {
    sender: mpsc::Sender<Vec<u8>>,
    role: DataChannelRole,
}

impl MockTransportChannel {
    pub fn new_pair(role: DataChannelRole, buffer_size: usize) -> (Self, mpsc::Receiver<Vec<u8>>) {
        let (tx, rx) = mpsc::channel(buffer_size);
        (Self { sender: tx, role }, rx)
    }

    pub fn role(&self) -> DataChannelRole {
        self.role
    }
}

#[async_trait]
impl TransportChannel for MockTransportChannel {
    async fn send_raw(&self, data: &[u8]) -> Result<(), PeerError> {
        self.sender
            .send(data.to_vec())
            .await
            .map_err(|_| PeerError::ChannelClosed)
    }

    async fn close(&self) -> Result<(), PeerError> {
        Ok(())
    }
}

/// Remote-Desk WebRTC Channel Manager
pub struct RemoteDeskPeerSession {
    control_channel: Arc<dyn TransportChannel>,
    input_channel: Arc<dyn TransportChannel>,
    seq_counter: AtomicU64,
}

impl RemoteDeskPeerSession {
    pub fn new(
        control_channel: Arc<dyn TransportChannel>,
        input_channel: Arc<dyn TransportChannel>,
    ) -> Self {
        Self {
            control_channel,
            input_channel,
            seq_counter: AtomicU64::new(1),
        }
    }

    pub async fn send_control(&self, msg: RemoteDeskMessage) -> Result<(), PeerError> {
        let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);
        let bytes = pack_frame(seq, &msg)?;
        self.control_channel.send_raw(&bytes).await
    }

    pub async fn send_input(&self, msg: RemoteDeskMessage) -> Result<(), PeerError> {
        let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);
        let bytes = pack_frame(seq, &msg)?;
        self.input_channel.send_raw(&bytes).await
    }
}
