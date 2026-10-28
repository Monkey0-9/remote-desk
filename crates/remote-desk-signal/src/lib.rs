use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SignalServerError {
    #[error("Device rate limit exceeded (maximum 5 requests per second)")]
    RateLimitExceeded,
    #[error("Target peer '{0}' not registered on rendezvous broker")]
    PeerNotFound(String),
    #[error("Rendezvous session capacity reached")]
    CapacityExceeded,
}

/// Token bucket rate limiter preventing flooding attacks (THR-DOS-001)
#[derive(Debug)]
pub struct TokenBucket {
    capacity: u32,
    tokens: f64,
    fill_rate_per_sec: f64,
    last_update: Instant,
}

impl TokenBucket {
    pub fn new(capacity: u32, fill_rate_per_sec: f64) -> Self {
        Self {
            capacity,
            tokens: capacity as f64,
            fill_rate_per_sec,
            last_update: Instant::now(),
        }
    }

    pub fn try_acquire(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        self.last_update = now;

        self.tokens = (self.tokens + elapsed * self.fill_rate_per_sec).min(self.capacity as f64);

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Ephemeral Rendezvous Broker routing messages between controllers and hosts
#[derive(Clone)]
pub struct RendezvousBroker {
    // Map of device_id -> registered message mailbox
    mailboxes: Arc<Mutex<HashMap<String, Vec<Vec<u8>>>>>,
    rate_limiters: Arc<Mutex<HashMap<String, TokenBucket>>>,
    max_devices: usize,
}

impl RendezvousBroker {
    pub fn new(max_devices: usize) -> Self {
        Self {
            mailboxes: Arc::new(Mutex::new(HashMap::new())),
            rate_limiters: Arc::new(Mutex::new(HashMap::new())),
            max_devices,
        }
    }

    pub fn register_peer(&self, device_id: &str) -> Result<(), SignalServerError> {
        let mut mailboxes = self.mailboxes.lock().unwrap();
        if mailboxes.len() >= self.max_devices && !mailboxes.contains_key(device_id) {
            return Err(SignalServerError::CapacityExceeded);
        }
        mailboxes.entry(device_id.to_string()).or_default();
        Ok(())
    }

    pub fn unregister_peer(&self, device_id: &str) {
        let mut mailboxes = self.mailboxes.lock().unwrap();
        mailboxes.remove(device_id);

        let mut limiters = self.rate_limiters.lock().unwrap();
        limiters.remove(device_id);
    }

    pub fn route_message(
        &self,
        sender_id: &str,
        target_id: &str,
        payload: Vec<u8>,
    ) -> Result<(), SignalServerError> {
        // Enforce token-bucket rate limit (5 requests per second max)
        {
            let mut limiters = self.rate_limiters.lock().unwrap();
            let bucket = limiters
                .entry(sender_id.to_string())
                .or_insert_with(|| TokenBucket::new(5, 5.0));

            if !bucket.try_acquire() {
                return Err(SignalServerError::RateLimitExceeded);
            }
        }

        // Route payload to target mailbox
        let mut mailboxes = self.mailboxes.lock().unwrap();
        if let Some(queue) = mailboxes.get_mut(target_id) {
            queue.push(payload);
            Ok(())
        } else {
            Err(SignalServerError::PeerNotFound(target_id.to_string()))
        }
    }

    pub fn poll_messages(&self, device_id: &str) -> Vec<Vec<u8>> {
        let mut mailboxes = self.mailboxes.lock().unwrap();
        if let Some(queue) = mailboxes.get_mut(device_id) {
            std::mem::take(queue)
        } else {
            Vec::new()
        }
    }

    pub fn registered_peer_count(&self) -> usize {
        self.mailboxes.lock().unwrap().len()
    }
}
