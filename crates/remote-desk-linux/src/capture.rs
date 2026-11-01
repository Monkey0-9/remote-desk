use async_trait::async_trait;
use remote_desk_proto::MonitorDescriptor;
use std::env;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxDisplayServer {
    Wayland,
    X11,
    Headless,
}

/// Detects whether current Linux environment is running Wayland, X11, or Headless
pub fn detect_display_server() -> LinuxDisplayServer {
    if env::var("WAYLAND_DISPLAY").is_ok()
        || env::var("XDG_SESSION_TYPE")
            .map(|v| v.to_lowercase() == "wayland")
            .unwrap_or(false)
    {
        LinuxDisplayServer::Wayland
    } else if env::var("DISPLAY").is_ok() {
        LinuxDisplayServer::X11
    } else {
        LinuxDisplayServer::Headless
    }
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

        let width = monitor.width;
        let height = monitor.height;
        let stride = width * 4;
        let size = (stride * height) as usize;

        let mut data = vec![0u8; size];
        let color_val = (self.frame_count % 255) as u8;
        for y in 0..100.min(height) {
            for x in 0..100.min(width) {
                let idx = ((y * stride) + (x * 4)) as usize;
                data[idx] = color_val;
                data[idx + 1] = 128;
                data[idx + 2] = 200;
                data[idx + 3] = 255;
            }
        }

        Ok(RawFrame {
            width,
            height,
            stride,
            data,
            timestamp_us: self.frame_count * 16_666,
        })
    }

    async fn stop_capture(&mut self) -> Result<(), CaptureError> {
        self.active_monitor = None;
        Ok(())
    }
}

pub use crate::portal_capture::WaylandPortalCaptureBackend;
pub use crate::x11_capture::X11ShmCaptureBackend;

pub type PortalCaptureBackend = WaylandPortalCaptureBackend;

/// Helper to create appropriate Linux capture backend based on environment.
/// In production mode (`allow_mock = false`), silent fallback to simulation is strictly rejected.
pub fn create_linux_capture_backend(
    allow_mock: bool,
) -> Result<Box<dyn ScreenCaptureBackend>, CaptureError> {
    match detect_display_server() {
        LinuxDisplayServer::Wayland => Ok(Box::new(WaylandPortalCaptureBackend::new())),
        LinuxDisplayServer::X11 => {
            if let Ok(x11) = X11ShmCaptureBackend::new() {
                Ok(Box::new(x11))
            } else if allow_mock {
                Ok(Box::new(MockCaptureBackend::new()))
            } else {
                Err(CaptureError::BackendError(
                    "Failed to connect to X11 display server. Production capture requires an active X11 display.".to_string(),
                ))
            }
        }
        LinuxDisplayServer::Headless => {
            if allow_mock {
                Ok(Box::new(MockCaptureBackend::new()))
            } else {
                Err(CaptureError::BackendError(
                    "Headless environment detected without active Wayland or X11 session. Production capture unavailable.".to_string(),
                ))
            }
        }
    }
}
