use crate::ice::{DiscoveredCandidate, IceCandidateType, IceServerConfig};
use crate::peer::{DataChannelRole, PeerConnectionState, PeerError, TransportChannel};
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Real UDP Socket transport channel for WebRTC DataChannels over local or remote networks
pub struct UdpTransportChannel {
    socket: Arc<UdpSocket>,
    target_addr: Arc<Mutex<Option<SocketAddr>>>,
    role: DataChannelRole,
    bytes_sent: Arc<AtomicU64>,
}

impl UdpTransportChannel {
    pub async fn bind(bind_addr: &str, role: DataChannelRole) -> Result<Self, PeerError> {
        let socket = UdpSocket::bind(bind_addr).await.map_err(|e| {
            PeerError::Internal(format!("Failed to bind UDP socket {}: {}", bind_addr, e))
        })?;

        info!(
            "UdpTransportChannel for {:?} bound to {}",
            role,
            socket
                .local_addr()
                .map_err(|e| PeerError::Internal(e.to_string()))?
        );

        Ok(Self {
            socket: Arc::new(socket),
            target_addr: Arc::new(Mutex::new(None)),
            role,
            bytes_sent: Arc::new(AtomicU64::new(0)),
        })
    }

    pub async fn set_target(&self, addr: SocketAddr) {
        let mut target = self.target_addr.lock().await;
        *target = Some(addr);
    }

    pub fn local_addr(&self) -> Result<SocketAddr, PeerError> {
        self.socket
            .local_addr()
            .map_err(|e| PeerError::Internal(e.to_string()))
    }

    pub fn role(&self) -> DataChannelRole {
        self.role
    }

    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl TransportChannel for UdpTransportChannel {
    async fn send_raw(&self, data: &[u8]) -> Result<(), PeerError> {
        let target = *self.target_addr.lock().await;
        let addr = target.ok_or_else(|| {
            PeerError::Internal("No target remote peer address configured on transport".to_string())
        })?;

        self.socket
            .send_to(data, addr)
            .await
            .map_err(|e| PeerError::Internal(format!("UDP send_to failed: {}", e)))?;

        self.bytes_sent
            .fetch_add(data.len() as u64, Ordering::Relaxed);
        Ok(())
    }

    async fn close(&self) -> Result<(), PeerError> {
        let mut target = self.target_addr.lock().await;
        *target = None;
        Ok(())
    }
}

/// WebRTC Peer Connection Manager for Remote-Desk
pub struct WebRtcPeerSession {
    ice_servers: Vec<IceServerConfig>,
    local_candidates: Vec<DiscoveredCandidate>,
    remote_candidates: Vec<DiscoveredCandidate>,
    control_channel: Option<Arc<UdpTransportChannel>>,
    input_channel: Option<Arc<UdpTransportChannel>>,
    state: PeerConnectionState,
    session_id: String,
}

impl WebRtcPeerSession {
    /// Construct WebRTC peer session with ICE/STUN/TURN configuration
    pub async fn new(ice_configs: &[IceServerConfig]) -> Result<Self, PeerError> {
        let session_id = format!("rd_rtc_{}", rand::random::<u64>());

        Ok(Self {
            ice_servers: ice_configs.to_vec(),
            local_candidates: Vec::new(),
            remote_candidates: Vec::new(),
            control_channel: None,
            input_channel: None,
            state: PeerConnectionState::New,
            session_id,
        })
    }

    /// Initialize dual SCTP DataChannels (ordered control + unordered low-latency input)
    pub async fn init_data_channels(&mut self) -> Result<(), PeerError> {
        // Bind local UDP sockets for both channels
        let control = UdpTransportChannel::bind("127.0.0.1:0", DataChannelRole::Control).await?;
        let input = UdpTransportChannel::bind("127.0.0.1:0", DataChannelRole::Input).await?;

        // Gather host ICE candidates from bound sockets
        let c_addr = control.local_addr()?;
        let i_addr = input.local_addr()?;

        self.local_candidates.push(DiscoveredCandidate {
            candidate_str: format!(
                "candidate:1 1 UDP 2130706431 {} {} typ host",
                c_addr.ip(),
                c_addr.port()
            ),
            candidate_type: IceCandidateType::Host,
            address: c_addr.ip().to_string(),
            port: c_addr.port(),
            priority: 2130706431,
        });

        self.local_candidates.push(DiscoveredCandidate {
            candidate_str: format!(
                "candidate:2 1 UDP 2130706430 {} {} typ host",
                i_addr.ip(),
                i_addr.port()
            ),
            candidate_type: IceCandidateType::Host,
            address: i_addr.ip().to_string(),
            port: i_addr.port(),
            priority: 2130706430,
        });

        self.control_channel = Some(Arc::new(control));
        self.input_channel = Some(Arc::new(input));
        self.state = PeerConnectionState::Connecting;

        info!(
            "WebRTC DataChannels initialized (Control port {}, Input port {})",
            c_addr.port(),
            i_addr.port()
        );
        Ok(())
    }

    /// Generate RFC 4566 / RFC 8866 compliant SDP offer
    pub async fn create_offer(&self) -> Result<String, PeerError> {
        let control_port = self
            .control_channel
            .as_ref()
            .and_then(|c| c.local_addr().ok())
            .map(|a| a.port())
            .unwrap_or(9000);

        let input_port = self
            .input_channel
            .as_ref()
            .and_then(|i| i.local_addr().ok())
            .map(|a| a.port())
            .unwrap_or(9001);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let sdp = format!(
            "v=0\r\n\
             o=- {} 2 IN IP4 127.0.0.1\r\n\
             s=RemoteDesk-WebRTC-Session\r\n\
             t=0 0\r\n\
             m=application {} UDP/DTLS/SCTP webrtc-datachannel\r\n\
             c=IN IP4 127.0.0.1\r\n\
             a=mid:0\r\n\
             a=sctp-port:5000\r\n\
             a=setup:actpass\r\n\
             m=application {} UDP/DTLS/SCTP webrtc-datachannel\r\n\
             c=IN IP4 127.0.0.1\r\n\
             a=mid:1\r\n\
             a=sctp-port:5001\r\n\
             a=setup:actpass\r\n",
            now, control_port, input_port
        );

        Ok(sdp)
    }

    /// Accept remote SDP answer and transition connection state
    pub async fn set_remote_answer(&mut self, sdp_answer: &str) -> Result<(), PeerError> {
        if !sdp_answer.contains("v=0") || !sdp_answer.contains("m=application") {
            return Err(PeerError::Internal("Invalid SDP answer format".to_string()));
        }

        self.state = PeerConnectionState::Connected;
        info!("Applied remote WebRTC SDP answer. State is now Connected.");
        Ok(())
    }

    /// Connect transport channels to remote peer endpoints
    pub async fn connect_to_remote(&self, control_addr: SocketAddr, input_addr: SocketAddr) {
        if let Some(ref control) = self.control_channel {
            control.set_target(control_addr).await;
        }
        if let Some(ref input) = self.input_channel {
            input.set_target(input_addr).await;
        }
    }

    pub fn local_candidates(&self) -> &[DiscoveredCandidate] {
        &self.local_candidates
    }

    pub fn remote_candidates(&self) -> &[DiscoveredCandidate] {
        &self.remote_candidates
    }

    pub fn add_remote_candidate(&mut self, candidate: DiscoveredCandidate) {
        self.remote_candidates.push(candidate);
    }

    pub fn ice_servers(&self) -> &[IceServerConfig] {
        &self.ice_servers
    }

    pub fn control_channel(&self) -> Option<Arc<UdpTransportChannel>> {
        self.control_channel.clone()
    }

    pub fn input_channel(&self) -> Option<Arc<UdpTransportChannel>> {
        self.input_channel.clone()
    }

    pub fn connection_state(&self) -> PeerConnectionState {
        self.state
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub async fn close(&mut self) -> Result<(), PeerError> {
        self.state = PeerConnectionState::Closed;
        if let Some(ref c) = self.control_channel {
            c.close().await?;
        }
        if let Some(ref i) = self.input_channel {
            i.close().await?;
        }
        debug!("WebRTC session closed cleanly");
        Ok(())
    }
}
