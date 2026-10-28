use remote_desk_core::{
    compute_sas_code, DeviceIdentity, EphemeralKeyExchange, SessionAction, SessionEvent,
    SessionStateMachine,
};
use remote_desk_linux::{
    InputInjectorBackend, MockCaptureBackend, MockInputInjector, ScreenCaptureBackend,
};
use remote_desk_net::IceServerConfig;
use remote_desk_proto::{
    unpack_frame, KeyState, MonitorDescriptor, MouseButtonPayload, MouseMoveAbsPayload,
    MouseMoveRelPayload, MouseWheelPayload, RemoteDeskMessage,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{info, warn};

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
        let shared_secret = host_exchange.compute_shared_secret(peer_ephemeral_pubkey);

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

        let (_seq, msg) = unpack_frame(frame).map_err(|e| HostError::Proto(e.to_string()))?;

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
}
