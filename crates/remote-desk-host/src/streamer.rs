use remote_desk_linux::{RawFrame, ScreenCaptureBackend};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct StreamStats {
    pub frames_captured: u64,
    pub frames_sent: u64,
    pub frames_dropped: u64,
}

pub struct FrameStreamer {
    stats_captured: Arc<AtomicU64>,
    stats_sent: Arc<AtomicU64>,
    stats_dropped: Arc<AtomicU64>,
    is_running: Arc<AtomicBool>,
}

impl FrameStreamer {
    pub fn new() -> Self {
        Self {
            stats_captured: Arc::new(AtomicU64::new(0)),
            stats_sent: Arc::new(AtomicU64::new(0)),
            stats_dropped: Arc::new(AtomicU64::new(0)),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn stats(&self) -> StreamStats {
        StreamStats {
            frames_captured: self.stats_captured.load(Ordering::Relaxed),
            frames_sent: self.stats_sent.load(Ordering::Relaxed),
            frames_dropped: self.stats_dropped.load(Ordering::Relaxed),
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Spawns a real-time frame streaming task that pulls frames from the capture backend
    /// at the target FPS, applying real-time backpressure frame-dropping if the network
    /// output queue is saturated.
    pub fn start_streaming(
        &self,
        mut capture: Box<dyn ScreenCaptureBackend>,
        monitor_id: u32,
        target_fps: u32,
        frame_tx: mpsc::Sender<RawFrame>,
        mut stop_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        let is_running = self.is_running.clone();
        let stats_captured = self.stats_captured.clone();
        let stats_sent = self.stats_sent.clone();
        let stats_dropped = self.stats_dropped.clone();

        is_running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            info!(
                "Starting frame streamer on monitor {} @ {} FPS target",
                monitor_id, target_fps
            );

            if let Err(e) = capture.start_capture(monitor_id).await {
                warn!("Failed to start capture backend: {}", e);
                is_running.store(false, Ordering::SeqCst);
                return;
            }

            let frame_duration = Duration::from_micros(1_000_000 / target_fps.max(1) as u64);
            let mut next_tick = Instant::now();

            while is_running.load(Ordering::Relaxed) {
                tokio::select! {
                    _ = stop_rx.recv() => {
                        debug!("Streamer task received stop broadcast");
                        break;
                    }
                    _ = tokio::time::sleep_until(tokio::time::Instant::from_std(next_tick)) => {
                        next_tick += frame_duration;
                        // Avoid drift if frame acquisition took longer than period
                        if next_tick < Instant::now() {
                            next_tick = Instant::now() + frame_duration;
                        }

                        match capture.next_frame().await {
                            Ok(frame) => {
                                stats_captured.fetch_add(1, Ordering::Relaxed);

                                // Try non-blocking send; if buffer is full, drop frame to preserve real-time low latency
                                match frame_tx.try_send(frame) {
                                    Ok(_) => {
                                        stats_sent.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Err(mpsc::error::TrySendError::Full(_)) => {
                                        stats_dropped.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Err(mpsc::error::TrySendError::Closed(_)) => {
                                        debug!("Downstream frame receiver closed");
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Error acquiring next frame: {:?}", e);
                                break;
                            }
                        }
                    }
                }
            }

            let _ = capture.stop_capture().await;
            is_running.store(false, Ordering::SeqCst);
            info!("Frame streamer task stopped cleanly");
        })
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }
}

impl Default for FrameStreamer {
    fn default() -> Self {
        Self::new()
    }
}
