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

/// Computes RFC 8445 Section 5.1.2.1 candidate priority value:
/// priority = (2^24)*(type preference) + (2^8)*(local preference) + (2^0)*(256 - component ID)
pub fn compute_ice_priority(
    candidate_type: IceCandidateType,
    local_pref: u16,
    component_id: u8,
) -> u32 {
    let type_pref: u32 = match candidate_type {
        IceCandidateType::Host => 126,
        IceCandidateType::PeerReflexive => 110,
        IceCandidateType::ServerReflexive => 100,
        IceCandidateType::Relay => 0,
    };
    (type_pref << 24) | ((local_pref as u32) << 8) | ((256 - component_id as u32) & 0xFF)
}

impl DiscoveredCandidate {
    /// Parses an RFC 5245 / RFC 8445 candidate attribute line (e.g. "a=candidate:1 1 UDP 2130706431 192.168.1.50 9000 typ host")
    pub fn from_sdp_attribute(line: &str) -> Result<Self, String> {
        let trimmed = line.trim();
        let stripped = trimmed.strip_prefix("a=").unwrap_or(trimmed);
        let cand_body = stripped
            .strip_prefix("candidate:")
            .ok_or_else(|| "Missing candidate: prefix in SDP attribute".to_string())?;

        let tokens: Vec<&str> = cand_body.split_whitespace().collect();
        if tokens.len() < 8 {
            return Err(format!(
                "Malformed candidate line (expected >= 8 tokens, got {})",
                tokens.len()
            ));
        }

        // tokens: [0]=foundation, [1]=component, [2]=transport, [3]=priority, [4]=address, [5]=port, [6]="typ", [7]=type
        let priority = tokens[3]
            .parse::<u32>()
            .map_err(|e| format!("Invalid priority in candidate: {}", e))?;
        let address = tokens[4].to_string();
        let port = tokens[5]
            .parse::<u16>()
            .map_err(|e| format!("Invalid port in candidate: {}", e))?;

        let candidate_type = match tokens[7] {
            "host" => IceCandidateType::Host,
            "srflx" => IceCandidateType::ServerReflexive,
            "prflx" => IceCandidateType::PeerReflexive,
            "relay" => IceCandidateType::Relay,
            other => return Err(format!("Unknown candidate type: {}", other)),
        };

        Ok(Self {
            candidate_str: format!("candidate:{}", cand_body),
            candidate_type,
            address,
            port,
            priority,
        })
    }

    /// Formats as standard RFC 8866 SDP attribute line
    pub fn to_sdp_attribute(&self, foundation: u32, component_id: u8) -> String {
        let typ_str = match self.candidate_type {
            IceCandidateType::Host => "host",
            IceCandidateType::ServerReflexive => "srflx",
            IceCandidateType::PeerReflexive => "prflx",
            IceCandidateType::Relay => "relay",
        };
        format!(
            "a=candidate:{} {} UDP {} {} {} typ {}",
            foundation, component_id, self.priority, self.address, self.port, typ_str
        )
    }
}
