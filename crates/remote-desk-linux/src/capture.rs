use async_trait::async_trait;
use remote_desk_proto::MonitorDescriptor;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum CaptureError {
    #[error("User denied portal capture permission")]
    PermissionDenied,
    #[error("Target monitor ID {0} not found")]
    MonitorNotFound(u32),
    #[error("Display disconnected")]
    DisplayDisconnected,
    #[error("Underlying PipeWire/X11 error: {0}")]
    BackendError(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Vec<u8>,
    pub timestamp_us: u64,
}

#[async_trait]
pub trait ScreenCaptureBackend: Send + Sync {
    async fn enumerate_monitors(&self) -> Result<Vec<MonitorDescriptor>, CaptureError>;
    async fn start_capture(&mut self, monitor_id: u32) -> Result<(), CaptureError>;
    async fn next_frame(&mut self) -> Result<RawFrame, CaptureError>;
    async fn stop_capture(&mut self) -> Result<(), CaptureError>;
}

/// Simulated capture backend for headless testing, CI, and loopback verification
pub struct MockCaptureBackend {
    monitors: Vec<MonitorDescriptor>,
    active_monitor: Option<u32>,
    frame_count: u64,
    should_deny_permission: bool,
}

impl MockCaptureBackend {
    pub fn new() -> Self {
        Self {
            monitors: vec![
                MonitorDescriptor {
                    id: 0,
                    name: "eDP-1 (Virtual Linux)".to_string(),
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                    refresh_rate: 60,
                    is_primary: true,
                },
                MonitorDescriptor {
                    id: 1,
                    name: "HDMI-1 (External)".to_string(),
                    x: 1920,
                    y: 0,
                    width: 2560,
                    height: 1440,
                    refresh_rate: 120,
                    is_primary: false,
                },
            ],
            active_monitor: None,
            frame_count: 0,
            should_deny_permission: false,
        }
    }

    pub fn with_permission_denial(mut self) -> Self {
        self.should_deny_permission = true;
        self
    }
}

impl Default for MockCaptureBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ScreenCaptureBackend for MockCaptureBackend {
    async fn enumerate_monitors(&self) -> Result<Vec<MonitorDescriptor>, CaptureError> {
        Ok(self.monitors.clone())
    }

    async fn start_capture(&mut self, monitor_id: u32) -> Result<(), CaptureError> {
        if self.should_deny_permission {
            return Err(CaptureError::PermissionDenied);
        }
        if !self.monitors.iter().any(|m| m.id == monitor_id) {
            return Err(CaptureError::MonitorNotFound(monitor_id));
        }
        self.active_monitor = Some(monitor_id);
        self.frame_count = 0;
        Ok(())
    }

    async fn next_frame(&mut self) -> Result<RawFrame, CaptureError> {
        let monitor_id = self
            .active_monitor
            .ok_or(CaptureError::DisplayDisconnected)?;
        let monitor = self
            .monitors
            .iter()
            .find(|m| m.id == monitor_id)
            .ok_or(CaptureError::MonitorNotFound(monitor_id))?;

        self.frame_count += 1;

        // Generate synthetic RGBA frame: alternating pattern per frame count
        let width = monitor.width;
        let height = monitor.height;
        let stride = width * 4;
        let size = (stride * height) as usize;

        let mut data = vec![0u8; size];
        let color_val = (self.frame_count % 255) as u8;
        // Fill top-left 100x100 rect with dynamic color indicator
        for y in 0..100.min(height) {
            for x in 0..100.min(width) {
                let idx = ((y * stride) + (x * 4)) as usize;
                data[idx] = color_val; // R
                data[idx + 1] = 128; // G
                data[idx + 2] = 200; // B
                data[idx + 3] = 255; // A
            }
        }

        Ok(RawFrame {
            width,
            height,
            stride,
            data,
            timestamp_us: self.frame_count * 16_666, // ~60fps interval
        })
    }

    async fn stop_capture(&mut self) -> Result<(), CaptureError> {
        self.active_monitor = None;
        Ok(())
    }
}
