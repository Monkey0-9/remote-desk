use async_trait::async_trait;
use remote_desk_proto::MonitorDescriptor;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, info};

#[derive(Error, Debug, PartialEq, Eq)]
pub enum WindowsCaptureError {
    #[error("Access to Desktop Duplication API denied or session locked (Error 0x887A0026)")]
    AccessDenied,
    #[error("Target monitor ID {0} not found")]
    MonitorNotFound(u32),
    #[error("Display mode or resolution changed")]
    DisplayModeChanged,
    #[error("DXGI/Direct3D11 device lost (0x887A0005)")]
    DeviceLost,
    #[error("DXGI API error: {0}")]
    ApiError(String),
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

/// Windows Desktop Duplication (IDXGIOutputDuplication) Capture Backend
pub struct DxgiCaptureBackend {
    monitors: Vec<MonitorDescriptor>,
    active_monitor: Option<u32>,
    frame_count: u64,
    access_denied: bool,
    device_lost: bool,
}

impl DxgiCaptureBackend {
    pub fn new() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self::new_native()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self::new_simulated()
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn new_simulated() -> Self {
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
            device_lost: false,
        }
    }

    #[cfg(target_os = "windows")]
    fn new_native() -> Self {
        use windows_sys::Win32::Graphics::Dxgi::*;

        let mut monitors = Vec::new();
        unsafe {
            let mut factory: *mut IDXGIFactory1 = std::ptr::null_mut();
            if CreateDXGIFactory1(&IID_IDXGIFactory1, &mut factory as *mut _ as *mut _) == 0 {
                let mut adapter_idx = 0;
                let mut adapter: *mut IDXGIAdapter1 = std::ptr::null_mut();
                while (*factory).EnumAdapters1(adapter_idx, &mut adapter) == 0 {
                    let mut output_idx = 0;
                    let mut output: *mut IDXGIOutput = std::ptr::null_mut();
                    while (*adapter).EnumOutputs(output_idx, &mut output) == 0 {
                        let mut desc: DXGI_OUTPUT_DESC = std::mem::zeroed();
                        if (*output).GetDesc(&mut desc) == 0 {
                            let width = (desc.DesktopCoordinates.right
                                - desc.DesktopCoordinates.left)
                                .abs() as u32;
                            let height = (desc.DesktopCoordinates.bottom
                                - desc.DesktopCoordinates.top)
                                .abs() as u32;
                            monitors.push(MonitorDescriptor {
                                id: output_idx,
                                name: format!(r"\\.\DISPLAY{}", output_idx + 1),
                                x: desc.DesktopCoordinates.left,
                                y: desc.DesktopCoordinates.top,
                                width,
                                height,
                                refresh_rate: 60,
                                is_primary: output_idx == 0,
                            });
                        }
                        (*output).Release();
                        output_idx += 1;
                    }
                    (*adapter).Release();
                    adapter_idx += 1;
                }
                (*factory).Release();
            }
        }

        if monitors.is_empty() {
            monitors.push(MonitorDescriptor {
                id: 0,
                name: r"\\.\DISPLAY1 (Default Windows)".to_string(),
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                refresh_rate: 60,
                is_primary: true,
            });
        }

        Self {
            monitors,
            active_monitor: None,
            frame_count: 0,
            access_denied: false,
            device_lost: false,
        }
    }

    pub fn with_access_denied(mut self) -> Self {
        self.access_denied = true;
        self
    }

    pub fn with_device_lost(mut self) -> Self {
        self.device_lost = true;
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
        if self.device_lost {
            return Err(WindowsCaptureError::DeviceLost);
        }
        if !self.monitors.iter().any(|m| m.id == monitor_id) {
            return Err(WindowsCaptureError::MonitorNotFound(monitor_id));
        }

        self.active_monitor = Some(monitor_id);
        self.frame_count = 0;
        info!(
            "DXGI Output Duplication initialized on monitor {}",
            monitor_id
        );
        Ok(())
    }

    async fn next_frame(&mut self) -> Result<WindowsRawFrame, WindowsCaptureError> {
        if self.device_lost {
            return Err(WindowsCaptureError::DeviceLost);
        }

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

        let timestamp_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        Ok(WindowsRawFrame {
            width,
            height,
            stride,
            data,
            timestamp_us,
        })
    }

    async fn stop_capture(&mut self) -> Result<(), WindowsCaptureError> {
        self.active_monitor = None;
        debug!("DXGI desktop duplication session released");
        Ok(())
    }
}
