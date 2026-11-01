use remote_desk_proto::{MouseMoveAbsPayload, RemoteDeskMessage};
use std::collections::VecDeque;

/// Coalesces high-frequency pointer movements while preserving order of clicks and keystrokes
#[derive(Debug, Default)]
pub struct InputEventCoalescer {
    queue: VecDeque<RemoteDeskMessage>,
    pending_move: Option<MouseMoveAbsPayload>,
    total_received: usize,
    total_coalesced: usize,
}

impl InputEventCoalescer {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            pending_move: None,
            total_received: 0,
            total_coalesced: 0,
        }
    }

    /// Push an event into the coalescer
    pub fn push(&mut self, event: RemoteDeskMessage) {
        self.total_received += 1;
        match event {
            RemoteDeskMessage::MouseMoveAbs(m) => {
                if let Some(ref existing) = self.pending_move {
                    if existing.monitor_id == m.monitor_id {
                        self.total_coalesced += 1;
                    } else {
                        self.flush_pending_move();
                    }
                }
                self.pending_move = Some(m);
            }
            other => {
                // Must flush pending move first so preceding mouse movement occurs before clicks/keys
                self.flush_pending_move();
                self.queue.push_back(other);
            }
        }
    }

    fn flush_pending_move(&mut self) {
        if let Some(m) = self.pending_move.take() {
            self.queue.push_back(RemoteDeskMessage::MouseMoveAbs(m));
        }
    }

    /// Drain all queued and coalesced events
    pub fn drain(&mut self) -> Vec<RemoteDeskMessage> {
        self.flush_pending_move();
        self.queue.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.queue.len() + if self.pending_move.is_some() { 1 } else { 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty() && self.pending_move.is_none()
    }

    pub fn reduction_ratio(&self) -> f64 {
        if self.total_received == 0 {
            0.0
        } else {
            self.total_coalesced as f64 / self.total_received as f64
        }
    }
}
