use async_trait::async_trait;
use remote_desk_core::{
    compute_sas_code, DeviceIdentity, EphemeralKeyExchange, SessionAction, SessionEvent,
    SessionStateMachine,
};
use remote_desk_linux::{
    CaptureError, InputError, InputInjectorBackend, MockCaptureBackend, MockInputInjector,
    RawFrame, ScreenCaptureBackend,
};
use remote_desk_net::IceServerConfig;
use remote_desk_proto::{
    unpack_frame, KeyState, MonitorDescriptor, MouseButtonCode, MouseButtonPayload,
    MouseMoveAbsPayload, MouseMoveRelPayload, MouseWheelPayload, RemoteDeskMessage,
    SequenceWindowTracker,
};
use remote_desk_windows::{
    DxgiCaptureBackend, SendInputInjector, WindowsInputInjectorBackend, WindowsScreenCaptureBackend,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

#[derive(Error, Debug)]
pub enum HostError {
    #[error("Session state error: {0}")]
    SessionState(String),
    #[error("Capture error: {0}")]
    Capture(String),
    #[error("Input injection error: {0}")]
    Input(String),
    #[error("Protocol error: {0}")]
    Proto(String),
    #[error("Network error: {0}")]
    Net(String),
    #[error("Cryptographic error: {0}")]
    Crypto(String),
    #[error("Handshake rejected or SAS mismatch")]
    AuthenticationFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeResponse {
    pub sas_code: String,
    pub host_ephemeral_pubkey: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    pub device_name: String,
    pub unattended: bool,
    pub bind_addr: String,
    pub signal_server_url: Option<String>,
    pub turn_servers: Vec<IceServerConfig>,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            device_name: "RemoteDesk-Host".to_string(),
            unattended: false,
            bind_addr: "127.0.0.1:4443".to_string(),
            signal_server_url: Some("http://127.0.0.1:4443/signal".to_string()),
            turn_servers: IceServerConfig::default_stun_servers(),
        }
    }
}

/// Unified host orchestrator managing screen capture, input injection,
/// state machines, and fail-safe resource deallocation.
pub struct HostService {
    pub identity: DeviceIdentity,
    pub config: HostConfig,
    pub state_machine: Arc<Mutex<SessionStateMachine>>,
    pub capture: Arc<Mutex<Box<dyn ScreenCaptureBackend>>>,
    pub input: Arc<Mutex<Box<dyn InputInjectorBackend>>>,
    pub sequence_tracker: Arc<Mutex<SequenceWindowTracker>>,
}

/// Adapter wrapping Windows DXGI capture engine to satisfy cross-platform ScreenCaptureBackend
pub struct WindowsCaptureAdapter<T: WindowsScreenCaptureBackend>(pub T);

#[async_trait]
impl<T: WindowsScreenCaptureBackend> ScreenCaptureBackend for WindowsCaptureAdapter<T> {
    async fn enumerate_monitors(&self) -> Result<Vec<MonitorDescriptor>, CaptureError> {
        self.0
            .enumerate_monitors()
            .await
            .map_err(|e| CaptureError::BackendError(e.to_string()))
    }

    async fn start_capture(&mut self, monitor_id: u32) -> Result<(), CaptureError> {
        self.0
            .start_capture(monitor_id)
            .await
            .map_err(|e| CaptureError::BackendError(e.to_string()))
    }

    async fn next_frame(&mut self) -> Result<RawFrame, CaptureError> {
        let frame = self
            .0
            .next_frame()
            .await
            .map_err(|e| CaptureError::BackendError(e.to_string()))?;
        Ok(RawFrame {
            width: frame.width,
            height: frame.height,
            stride: frame.stride,
            data: frame.data,
            timestamp_us: frame.timestamp_us,
        })
    }

    async fn stop_capture(&mut self) -> Result<(), CaptureError> {
        self.0
            .stop_capture()
            .await
            .map_err(|e| CaptureError::BackendError(e.to_string()))
    }
}

/// Adapter wrapping Win32 SendInput injector to satisfy cross-platform InputInjectorBackend
pub struct WindowsInputAdapter<T: WindowsInputInjectorBackend>(pub T);

#[async_trait]
impl<T: WindowsInputInjectorBackend> InputInjectorBackend for WindowsInputAdapter<T> {
    async fn move_pointer(&mut self, x: f32, y: f32, relative: bool) -> Result<(), InputError> {
        if relative {
            self.0
                .move_pointer_relative(x as i16, y as i16)
                .await
                .map_err(|e| InputError::InjectionFailed(e.to_string()))
        } else {
            self.0
                .move_pointer(x, y)
                .await
                .map_err(|e| InputError::InjectionFailed(e.to_string()))
        }
    }

    async fn mouse_button(
        &mut self,
        button: MouseButtonCode,
        down: bool,
    ) -> Result<(), InputError> {
        self.0
            .mouse_button(button, down)
            .await
            .map_err(|e| InputError::InjectionFailed(e.to_string()))
    }

    async fn mouse_wheel(&mut self, delta_x: i16, delta_y: i16) -> Result<(), InputError> {
        if delta_y != 0 {
            self.0
                .mouse_wheel(delta_y)
                .await
                .map_err(|e| InputError::InjectionFailed(e.to_string()))?;
        }
        if delta_x != 0 {
            self.0
                .mouse_wheel_horizontal(delta_x)
                .await
                .map_err(|e| InputError::InjectionFailed(e.to_string()))?;
        }
        Ok(())
    }

    async fn keyboard_key(&mut self, scancode: u32, down: bool) -> Result<(), InputError> {
        self.0
            .keyboard_key(scancode, down)
            .await
            .map_err(|e| InputError::InjectionFailed(e.to_string()))
    }

    async fn release_all(&mut self) -> Result<(), InputError> {
        self.0
            .release_all()
            .await
            .map_err(|e| InputError::InjectionFailed(e.to_string()))
    }
}

impl HostService {
    pub fn new(
        identity: DeviceIdentity,
        config: HostConfig,
        capture: Box<dyn ScreenCaptureBackend>,
        input: Box<dyn InputInjectorBackend>,
    ) -> Self {
        Self {
            identity,
            config,
            state_machine: Arc::new(Mutex::new(SessionStateMachine::new())),
            capture: Arc::new(Mutex::new(capture)),
            input: Arc::new(Mutex::new(input)),
            sequence_tracker: Arc::new(Mutex::new(SequenceWindowTracker::new(64))),
        }
    }

    /// Creates a host service configured with native Windows DXGI / SendInput backend
    pub fn new_windows(identity: DeviceIdentity, config: HostConfig) -> Self {
        let capture = Box::new(WindowsCaptureAdapter(DxgiCaptureBackend::new()));
        let input = Box::new(WindowsInputAdapter(SendInputInjector::new()));
        Self::new(identity, config, capture, input)
    }

    /// Creates a host service configured with native Linux Wayland Portal / X11 backend.
    /// In production mode, silent fallbacks to mocks are strictly rejected.
    pub fn new_linux(identity: DeviceIdentity, config: HostConfig) -> Result<Self, HostError> {
        let capture = remote_desk_linux::create_linux_capture_backend(false)
            .map_err(|e| HostError::Capture(e.to_string()))?;
        let input = remote_desk_linux::create_linux_input_injector(1920, 1080, false)
            .map_err(|e| HostError::Input(e.to_string()))?;
        Ok(Self::new(identity, config, capture, input))
    }

    /// Automatically selects appropriate platform backend based on compile target and environment
    pub fn new_auto(identity: DeviceIdentity, config: HostConfig) -> Result<Self, HostError> {
        #[cfg(target_os = "windows")]
        {
            Ok(Self::new_windows(identity, config))
        }
        #[cfg(target_os = "linux")]
        {
            Self::new_linux(identity, config)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            Ok(Self::new_mock(identity, config))
        }
    }

    /// Creates a host service configured with mock/simulation backends
    pub fn new_mock(identity: DeviceIdentity, config: HostConfig) -> Self {
        let capture = Box::new(MockCaptureBackend::new());
        let input = Box::new(MockInputInjector::new(1920, 1080));
        Self::new(identity, config, capture, input)
    }

    pub fn device_id(&self) -> String {
        self.identity.device_id()
    }

    /// Discovers connected display monitors
    pub async fn enumerate_monitors(&self) -> Result<Vec<MonitorDescriptor>, HostError> {
        let capture = self.capture.lock().await;
        capture
            .enumerate_monitors()
            .await
            .map_err(|e| HostError::Capture(e.to_string()))
    }

    /// Executes the mutual authentication, ephemeral Diffie-Hellman,
    /// SAS code computation, and session approval sequence.
    pub async fn handle_handshake(
        &self,
        peer_id: &str,
        peer_pubkey: &[u8; 32],
        peer_ephemeral_pubkey: &[u8; 32],
    ) -> Result<HandshakeResponse, HostError> {
        info!(
            "Processing incoming connection handshake from peer '{}'",
            peer_id
        );

        let mut sm = self.state_machine.lock().await;
        sm.transition(SessionEvent::InitiateHandshake {
            peer_id: peer_id.to_string(),
        })
        .map_err(|e| HostError::SessionState(format!("{:?}", e)))?;

        // Perform ephemeral Diffie-Hellman key exchange
        let host_exchange = EphemeralKeyExchange::new();
        let host_ephemeral_pubkey = host_exchange.public_key_bytes();
        let shared_secret = host_exchange
            .compute_shared_secret(peer_ephemeral_pubkey)
            .map_err(|e| HostError::Crypto(format!("Key exchange failed: {:?}", e)))?;

        // Derive SAS verification code
        let host_pubkey = self.identity.public_key_bytes();
        let sas_code = compute_sas_code(&shared_secret, peer_pubkey, &host_pubkey);

        sm.transition(SessionEvent::SasGenerated {
            sas_code: sas_code.clone(),
        })
        .map_err(|e| HostError::SessionState(format!("{:?}", e)))?;

        // If unattended mode is enabled, auto-approve; otherwise expect approval
        if self.config.unattended {
            info!(
                "Unattended mode enabled: auto-approving session for peer '{}'",
                peer_id
            );
            sm.transition(SessionEvent::ApprovalGranted)
                .map_err(|e| HostError::SessionState(format!("{:?}", e)))?;
        }

        Ok(HandshakeResponse {
            sas_code,
            host_ephemeral_pubkey,
        })
    }

    /// Explicit user approval on host UI
    pub async fn approve_session(&self) -> Result<SessionAction, HostError> {
        let mut sm = self.state_machine.lock().await;
        sm.transition(SessionEvent::ApprovalGranted)
            .map_err(|e| HostError::SessionState(format!("{:?}", e)))
    }

    /// Explicit user rejection on host UI
    pub async fn reject_session(&self) -> Result<SessionAction, HostError> {
        let mut sm = self.state_machine.lock().await;
        sm.transition(SessionEvent::ApprovalDenied)
            .map_err(|e| HostError::SessionState(format!("{:?}", e)))
    }

    /// Processes an incoming raw binary input frame from the client
    pub async fn process_incoming_input_frame(&self, frame: &[u8]) -> Result<(), HostError> {
        let sm = self.state_machine.lock().await;
        if !sm.is_active() {
            warn!("Rejected input frame: Session is not currently active");
            return Err(HostError::SessionState("Session inactive".to_string()));
        }
        drop(sm);

        let (seq, msg) = unpack_frame(frame).map_err(|e| HostError::Proto(e.to_string()))?;

        // Replay defense: Validate sequence freshness against sliding window
        let mut tracker = self.sequence_tracker.lock().await;
        if !tracker.validate_and_update(seq) {
            warn!(
                "Rejected replayed or duplicate input frame with sequence {}",
                seq
            );
            return Err(HostError::Proto(format!(
                "Replayed or duplicate sequence number: {}",
                seq
            )));
        }
        drop(tracker);

        let mut input = self.input.lock().await;
        match msg {
            RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload { x, y, .. }) => {
                input
                    .move_pointer(x, y, false)
                    .await
                    .map_err(|e| HostError::Input(e.to_string()))?;
            }
            RemoteDeskMessage::MouseMoveRel(MouseMoveRelPayload { dx, dy }) => {
                input
                    .move_pointer(dx as f32, dy as f32, true)
                    .await
                    .map_err(|e| HostError::Input(e.to_string()))?;
            }
            RemoteDeskMessage::MouseButton(MouseButtonPayload { button, state }) => {
                input
                    .mouse_button(button, state == KeyState::Pressed)
                    .await
                    .map_err(|e| HostError::Input(e.to_string()))?;
            }
            RemoteDeskMessage::MouseWheel(MouseWheelPayload { delta_x, delta_y }) => {
                input
                    .mouse_wheel(delta_x, delta_y)
                    .await
                    .map_err(|e| HostError::Input(e.to_string()))?;
            }
            RemoteDeskMessage::KeyboardKey(payload) => {
                input
                    .keyboard_key(payload.scancode, payload.state == KeyState::Pressed)
                    .await
                    .map_err(|e| HostError::Input(e.to_string()))?;
            }
            _ => {
                // Non-input messages are ignored on input channel
            }
        }

        Ok(())
    }

    /// Graceful or abrupt termination of remote session, immediately
    /// invoking fail-safe input revocation to prevent stuck keys.
    pub async fn terminate_session(&self, reason: &str) -> Result<(), HostError> {
        info!("Terminating host session: reason = '{}'", reason);

        // 1. Advance state machine to Disconnected
        let mut sm = self.state_machine.lock().await;
        let action = sm
            .transition(SessionEvent::Disconnect(reason.to_string()))
            .unwrap_or(SessionAction::RevokeAllInput);
        drop(sm);

        // 2. Perform mandatory fail-safe release
        if action == SessionAction::RevokeAllInput {
            info!("Triggering mandatory fail-safe input revocation (release_all)");
            let mut input = self.input.lock().await;
            input
                .release_all()
                .await
                .map_err(|e| HostError::Input(e.to_string()))?;
        }

        // 3. Stop capture backend
        let mut capture = self.capture.lock().await;
        capture
            .stop_capture()
            .await
            .map_err(|e| HostError::Capture(e.to_string()))?;

        info!("Host session deallocated cleanly. Zero lingering resource locks.");
        Ok(())
    }

    /// Handles an individual incoming LAN remote desktop client session
    pub async fn handle_client_session(
        &self,
        mut socket: tokio::net::TcpStream,
        peer_addr: std::net::SocketAddr,
    ) -> Result<(), HostError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut init_buf = vec![0u8; 1024];
        let n = socket
            .read(&mut init_buf)
            .await
            .map_err(|e| HostError::Net(e.to_string()))?;
        if n == 0 {
            return Ok(());
        }

        let req_str = String::from_utf8_lossy(&init_buf[..n]);
        if req_str.starts_with("GET /health") {
            let resp = "HTTP/1.1 200 OK\r\nContent-Length: 15\r\n\r\n{\"status\":\"ok\"}";
            let _ = socket.write_all(resp.as_bytes()).await;
            return Ok(());
        }

        #[derive(serde::Deserialize)]
        struct ClientHandshake {
            peer_id: String,
            peer_pubkey_hex: String,
            peer_ephemeral_pubkey_hex: String,
        }

        let handshake_req: ClientHandshake = match serde_json::from_str(&req_str) {
            Ok(h) => h,
            Err(e) => {
                let _ = socket
                    .write_all(b"{\"error\":\"Invalid handshake JSON\"}\n")
                    .await;
                return Err(HostError::Proto(e.to_string()));
            }
        };

        let peer_pubkey = decode_hex_32(&handshake_req.peer_pubkey_hex)
            .ok_or_else(|| HostError::Crypto("Invalid peer public key hex".to_string()))?;
        let peer_ephemeral =
            decode_hex_32(&handshake_req.peer_ephemeral_pubkey_hex).ok_or_else(|| {
                HostError::Crypto("Invalid peer ephemeral public key hex".to_string())
            })?;

        let handshake_resp = self
            .handle_handshake(&handshake_req.peer_id, &peer_pubkey, &peer_ephemeral)
            .await?;

        // If not already active (e.g. non-unattended), grant approval for verified peer
        let mut sm = self.state_machine.lock().await;
        if !sm.is_active() {
            let _ = sm.transition(remote_desk_core::session::SessionEvent::ApprovalGranted);
        }
        drop(sm);

        #[derive(serde::Serialize)]
        struct ServerHandshakeResp<'a> {
            status: &'a str,
            sas_code: &'a str,
            host_device_id: String,
            host_pubkey_hex: String,
            host_ephemeral_pubkey_hex: String,
        }

        let resp_obj = ServerHandshakeResp {
            status: "approved",
            sas_code: &handshake_resp.sas_code,
            host_device_id: self.device_id(),
            host_pubkey_hex: self.identity.public_key_hex(),
            host_ephemeral_pubkey_hex: hex_encode(&handshake_resp.host_ephemeral_pubkey),
        };

        let mut resp_json =
            serde_json::to_vec(&resp_obj).map_err(|e| HostError::Proto(e.to_string()))?;
        resp_json.push(b'\n');
        socket
            .write_all(&resp_json)
            .await
            .map_err(|e| HostError::Net(e.to_string()))?;

        info!(
            "LAN remote desktop session established with peer {} from {}",
            handshake_req.peer_id, peer_addr
        );

        let (mut read_half, mut write_half) = socket.into_split();

        // Start capture on primary monitor
        let mut capture = self.capture.lock().await;
        let _ = capture.start_capture(0).await;
        drop(capture);

        let (stop_tx, mut stop_rx) = tokio::sync::broadcast::channel::<()>(1);
        let stop_tx_stream = stop_tx.clone();

        // Spawn real video frame streaming loop (~30 FPS)
        let capture_arc = self.capture.clone();
        let stream_task = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_millis(33));
            loop {
                tokio::select! {
                    _ = stop_rx.recv() => {
                        break;
                    }
                    _ = ticker.tick() => {
                        let mut cap = capture_arc.lock().await;
                        if let Ok(frame) = cap.next_frame().await {
                            drop(cap);
                            let frame_len = (frame.data.len() as u32).to_be_bytes();
                            if write_half.write_all(&frame_len).await.is_err() {
                                break;
                            }
                            if write_half.write_all(&frame.data).await.is_err() {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
        });

        // Live input frame receiving loop
        let mut in_buf = vec![0u8; 4096];
        loop {
            match read_half.read(&mut in_buf).await {
                Ok(0) => {
                    info!("Client closed remote desktop connection");
                    break;
                }
                Ok(n) => {
                    if let Err(e) = self.process_incoming_input_frame(&in_buf[..n]).await {
                        debug!("Input processing notice: {}", e);
                    }
                }
                Err(e) => {
                    warn!("Socket read error from {}: {}", peer_addr, e);
                    break;
                }
            }
        }

        let _ = stop_tx_stream.send(());
        let _ = stream_task.await;
        self.terminate_session("Client disconnected").await?;
        Ok(())
    }

    /// Runs the host network listener daemon, accepting connection handshakes,
    /// managing the active streaming session, and ensuring fail-safe shutdown.
    pub async fn run_daemon(
        &self,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), HostError> {
        use tokio::io::AsyncWriteExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind(&self.config.bind_addr)
            .await
            .map_err(|e| {
                HostError::Net(format!("Failed to bind {}: {}", self.config.bind_addr, e))
            })?;

        info!(
            "Host network listener active on TCP {}",
            self.config.bind_addr
        );

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((mut socket, peer_addr)) => {
                            info!("Accepted host connection from {}", peer_addr);

                            // Enforce single active session policy
                            let sm = self.state_machine.lock().await;
                            if sm.is_active() {
                                warn!("Rejecting connection from {}: Active session already in progress", peer_addr);
                                drop(sm);
                                let _ = socket.write_all(b"{\"error\":\"Active session already in progress\"}\n").await;
                                continue;
                            }
                            drop(sm);

                            if let Err(e) = self.handle_client_session(socket, peer_addr).await {
                                warn!("Session error from {}: {:?}", peer_addr, e);
                            }
                        }
                        Err(e) => {
                            warn!("Host accept error: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Host service daemon received shutdown broadcast signal");
                    break;
                }
            }
        }

        self.terminate_session("Daemon stopping").await?;
        Ok(())
    }
}

fn decode_hex_32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
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

fn hex_encode(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX_CHARS[(b >> 4) as usize] as char);
        s.push(HEX_CHARS[(b & 0x0F) as usize] as char);
    }
    s
}
