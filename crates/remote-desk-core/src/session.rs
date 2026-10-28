use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Disconnected,
    Authenticating {
        peer_id: String,
    },
    WaitingApproval {
        peer_id: String,
        sas_code: String,
    },
    Active {
        peer_id: String,
        active_monitor: u32,
    },
    Paused {
        peer_id: String,
    },
    Closed {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    InitiateHandshake { peer_id: String },
    SasGenerated { sas_code: String },
    ApprovalGranted,
    ApprovalDenied,
    TransportConnected,
    SwitchMonitor(u32),
    Pause,
    Resume,
    Disconnect(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionAction {
    None,
    PromptUserApproval { peer_id: String, sas_code: String },
    StartStreaming { peer_id: String },
    RevokeAllInput,
    CloseTransport { reason: String },
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SessionError {
    #[error("Invalid state transition from {current:?} on event {event:?}")]
    InvalidTransition { current: String, event: String },
}

pub struct SessionStateMachine {
    state: SessionState,
}

impl SessionStateMachine {
    pub fn new() -> Self {
        Self {
            state: SessionState::Disconnected,
        }
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, SessionState::Active { .. })
    }

    pub fn can_inject_input(&self) -> bool {
        self.is_active()
    }

    pub fn transition(&mut self, event: SessionEvent) -> Result<SessionAction, SessionError> {
        let (next_state, action) = match (&self.state, event.clone()) {
            (SessionState::Disconnected, SessionEvent::InitiateHandshake { peer_id }) => (
                SessionState::Authenticating { peer_id },
                SessionAction::None,
            ),
            (SessionState::Authenticating { peer_id }, SessionEvent::SasGenerated { sas_code }) => {
                (
                    SessionState::WaitingApproval {
                        peer_id: peer_id.clone(),
                        sas_code: sas_code.clone(),
                    },
                    SessionAction::PromptUserApproval {
                        peer_id: peer_id.clone(),
                        sas_code,
                    },
                )
            }
            (SessionState::WaitingApproval { peer_id, .. }, SessionEvent::ApprovalGranted) => (
                SessionState::Active {
                    peer_id: peer_id.clone(),
                    active_monitor: 0,
                },
                SessionAction::StartStreaming {
                    peer_id: peer_id.clone(),
                },
            ),
            (SessionState::WaitingApproval { .. }, SessionEvent::ApprovalDenied) => (
                SessionState::Closed {
                    reason: "User denied pairing request".to_string(),
                },
                SessionAction::CloseTransport {
                    reason: "Permission denied".to_string(),
                },
            ),
            (SessionState::Active { peer_id, .. }, SessionEvent::SwitchMonitor(mon_id)) => (
                SessionState::Active {
                    peer_id: peer_id.clone(),
                    active_monitor: mon_id,
                },
                SessionAction::None,
            ),
            (SessionState::Active { peer_id, .. }, SessionEvent::Pause) => (
                SessionState::Paused {
                    peer_id: peer_id.clone(),
                },
                SessionAction::RevokeAllInput,
            ),
            (SessionState::Paused { peer_id }, SessionEvent::Resume) => (
                SessionState::Active {
                    peer_id: peer_id.clone(),
                    active_monitor: 0,
                },
                SessionAction::None,
            ),
            (
                SessionState::Active { .. }
                | SessionState::Paused { .. }
                | SessionState::WaitingApproval { .. }
                | SessionState::Authenticating { .. },
                SessionEvent::Disconnect(reason),
            ) => (
                SessionState::Closed {
                    reason: reason.clone(),
                },
                SessionAction::RevokeAllInput,
            ),
            _ => {
                return Err(SessionError::InvalidTransition {
                    current: format!("{:?}", self.state),
                    event: format!("{:?}", event),
                });
            }
        };

        self.state = next_state;
        Ok(action)
    }
}

impl Default for SessionStateMachine {
    fn default() -> Self {
        Self::new()
    }
}
