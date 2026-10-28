use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IceCandidateType {
    Host,
    ServerReflexive,
    PeerReflexive,
    Relay,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IceServerConfig {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

impl IceServerConfig {
    pub fn default_stun_servers() -> Vec<Self> {
        vec![
            IceServerConfig {
                urls: vec![
                    "stun:stun.l.google.com:19302".to_string(),
                    "stun:stun1.l.google.com:19302".to_string(),
                ],
                username: None,
                credential: None,
            },
            IceServerConfig {
                urls: vec!["stun:stun.cloudflare.com:3478".to_string()],
                username: None,
                credential: None,
            },
        ]
    }

    pub fn turn_relay(url: String, username: String, credential: String) -> Self {
        Self {
            urls: vec![url],
            username: Some(username),
            credential: Some(credential),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredCandidate {
    pub candidate_str: String,
    pub candidate_type: IceCandidateType,
    pub address: String,
    pub port: u16,
    pub priority: u32,
}
