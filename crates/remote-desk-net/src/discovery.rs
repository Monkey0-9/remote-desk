use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredPeer {
    pub device_id: String,
    pub name: String,
    pub os_type: String,
    pub address: String,
    pub port: u16,
    pub pubkey_fingerprint: String,
    pub last_seen_ms: u64,
}

pub struct PeerRegistry {
    peers: RwLock<HashMap<String, DiscoveredPeer>>,
    ttl_ms: u64,
}

impl PeerRegistry {
    pub fn new(ttl_ms: u64) -> Self {
        Self {
            peers: RwLock::new(HashMap::new()),
            ttl_ms,
        }
    }

    pub fn upsert_peer(&self, peer: DiscoveredPeer) {
        let mut map = self.peers.write().unwrap();
        map.insert(peer.device_id.clone(), peer);
    }

    pub fn remove_peer(&self, device_id: &str) -> Option<DiscoveredPeer> {
        let mut map = self.peers.write().unwrap();
        map.remove(device_id)
    }

    pub fn list_active_peers(&self, current_time_ms: u64) -> Vec<DiscoveredPeer> {
        let map = self.peers.read().unwrap();
        map.values()
            .filter(|p| {
                if current_time_ms >= p.last_seen_ms {
                    current_time_ms - p.last_seen_ms <= self.ttl_ms
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    }
}

impl Default for PeerRegistry {
    fn default() -> Self {
        Self::new(30_000) // 30 second default TTL
    }
}
