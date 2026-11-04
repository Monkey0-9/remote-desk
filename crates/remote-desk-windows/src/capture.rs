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
    #[error("Windows capture API error: {0}")]
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

/// Windows Desktop Capture Backend
pub struct DxgiCaptureBackend {
    monitors: Vec<MonitorDescriptor>,
    active_monitor: Option<u32>,
    frame_count: u64,
    access_denied: bool,
    device_lost: bool,
    allow_mock: bool,
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
            allow_mock: false,
        }
    }

    #[cfg(target_os = "windows")]
    fn new_native() -> Self {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;

        let mut monitors = Vec::new();
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN) as u32;
            let height = GetSystemMetrics(SM_CYSCREEN) as u32;

            if width > 0 && height > 0 {
                monitors.push(MonitorDescriptor {
                    id: 0,
                    name: r"\\.\DISPLAY1 (Primary Screen)".to_string(),
                    x: 0,
                    y: 0,
                    width,
                    height,
                    refresh_rate: 60,
                    is_primary: true,
                });
            }
        }

        if monitors.is_empty() {
            monitors.push(MonitorDescriptor {
                id: 0,
                name: r"\\.\DISPLAY1 (Default Screen)".to_string(),
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
            allow_mock: false,
        }
    }

    pub fn with_allow_mock(mut self, allow: bool) -> Self {
        self.allow_mock = allow;
        self
    }

    pub fn is_mock_allowed(&self) -> bool {
        self.allow_mock
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
        info!("Windows screen capture active on monitor {}", monitor_id);
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

        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::Graphics::Gdi::*;
            use windows_sys::Win32::UI::WindowsAndMessaging::*;

            let h_wnd = GetDesktopWindow();
            let h_dc = GetDC(h_wnd);
            if !h_dc.is_null() {
                let mem_dc = CreateCompatibleDC(h_dc);
                let h_bitmap = CreateCompatibleBitmap(h_dc, width as i32, height as i32);
                let old_bmp = SelectObject(mem_dc, h_bitmap);

                let blt_res = BitBlt(
                    mem_dc,
                    0,
                    0,
                    width as i32,
                    height as i32,
                    h_dc,
                    monitor.x,
                    monitor.y,
                    SRCCOPY,
                );

                if blt_res != 0 {
                    let mut bmi: BITMAPINFO = std::mem::zeroed();
                    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                    bmi.bmiHeader.biWidth = width as i32;
                    bmi.bmiHeader.biHeight = -(height as i32); // Top-down
                    bmi.bmiHeader.biPlanes = 1;
                    bmi.bmiHeader.biBitCount = 32;
                    bmi.bmiHeader.biCompression = BI_RGB;

                    let size = (stride * height) as usize;
                    let mut data = vec![0u8; size];

                    let get_res = GetDIBits(
                        mem_dc,
                        h_bitmap,
                        0,
                        height,
                        data.as_mut_ptr() as *mut _,
                        &mut bmi,
                        DIB_RGB_COLORS,
                    );

                    SelectObject(mem_dc, old_bmp);
                    DeleteObject(h_bitmap);
                    DeleteDC(mem_dc);
                    ReleaseDC(h_wnd, h_dc);

                    if get_res > 0 {
                        let timestamp_us = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_micros() as u64;

                        return Ok(WindowsRawFrame {
                            width,
                            height,
                            stride,
                            data,
                            timestamp_us,
                        });
                    }
                } else {
                    SelectObject(mem_dc, old_bmp);
                    DeleteObject(h_bitmap);
                    DeleteDC(mem_dc);
                    ReleaseDC(h_wnd, h_dc);
                }
            }
        }

        if !self.allow_mock {
            return Err(WindowsCaptureError::ApiError(
                "Windows native display capture failed or is unavailable, and mock fallback is disabled"
                    .to_string(),
            ));
        }

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
        debug!("Windows screen capture session released");
        Ok(())
    }
}
