use async_trait::async_trait;
use remote_desk_proto::MonitorDescriptor;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum WindowsCaptureError {
    #[error("Access to Desktop Duplication API denied or session locked (Error 0x887A0026)")]
    AccessDenied,
    #[error("Target monitor ID {0} not found")]
    MonitorNotFound(u32),
    #[error("Display mode or resolution changed")]
    DisplayModeChanged,
    #[error("DXGI/Direct3D11 device lost")]
    DeviceLost,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowsRawFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Vec<u8>,
    pub timestamp_us: u64,
}

#[async_trait]
pub trait WindowsScreenCaptureBackend: Send + Sync {
    async fn enumerate_monitors(&self) -> Result<Vec<MonitorDescriptor>, WindowsCaptureError>;
    async fn start_capture(&mut self, monitor_id: u32) -> Result<(), WindowsCaptureError>;
    async fn next_frame(&mut self) -> Result<WindowsRawFrame, WindowsCaptureError>;
    async fn stop_capture(&mut self) -> Result<(), WindowsCaptureError>;
}

/// Windows Desktop Duplication / WGC Capture Backend (supports cross-platform test simulation)
pub struct DxgiCaptureBackend {
    monitors: Vec<MonitorDescriptor>,
    active_monitor: Option<u32>,
    frame_count: u64,
    access_denied: bool,
}

impl DxgiCaptureBackend {
    pub fn new() -> Self {
        Self {
            monitors: vec![
                MonitorDescriptor {
                    id: 0,
                    name: r"\\.\DISPLAY1 (Primary Windows)".to_string(),
                    x: 0,
                    y: 0,
                    width: 2560,
                    height: 1440,
                    refresh_rate: 144,
                    is_primary: true,
                },
                MonitorDescriptor {
                    id: 1,
                    name: r"\\.\DISPLAY2 (Secondary Windows)".to_string(),
                    x: 2560,
                    y: 0,
                    width: 1920,
                    height: 1080,
                    refresh_rate: 60,
                    is_primary: false,
                },
            ],
            active_monitor: None,
            frame_count: 0,
            access_denied: false,
        }
    }

    pub fn with_access_denied(mut self) -> Self {
        self.access_denied = true;
        self
    }
}

impl Default for DxgiCaptureBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WindowsScreenCaptureBackend for DxgiCaptureBackend {
    async fn enumerate_monitors(&self) -> Result<Vec<MonitorDescriptor>, WindowsCaptureError> {
        Ok(self.monitors.clone())
    }

    async fn start_capture(&mut self, monitor_id: u32) -> Result<(), WindowsCaptureError> {
        if self.access_denied {
            return Err(WindowsCaptureError::AccessDenied);
        }
        if !self.monitors.iter().any(|m| m.id == monitor_id) {
            return Err(WindowsCaptureError::MonitorNotFound(monitor_id));
        }
        self.active_monitor = Some(monitor_id);
        self.frame_count = 0;
        Ok(())
    }

    async fn next_frame(&mut self) -> Result<WindowsRawFrame, WindowsCaptureError> {
        let monitor_id = self
            .active_monitor
            .ok_or(WindowsCaptureError::DisplayModeChanged)?;
        let monitor = self
            .monitors
            .iter()
            .find(|m| m.id == monitor_id)
            .ok_or(WindowsCaptureError::MonitorNotFound(monitor_id))?;

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
                data[idx] = 30;
                data[idx + 1] = 144;
                data[idx + 2] = color_val;
                data[idx + 3] = 255;
            }
        }

        Ok(WindowsRawFrame {
            width,
            height,
            stride,
            data,
            timestamp_us: self.frame_count * 6_944, // ~144fps interval
        })
    }

    async fn stop_capture(&mut self) -> Result<(), WindowsCaptureError> {
        self.active_monitor = None;
        Ok(())
    }
}
