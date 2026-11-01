use crate::capture::{CaptureError, RawFrame, ScreenCaptureBackend};
use async_trait::async_trait;
use remote_desk_proto::MonitorDescriptor;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info};
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as RandrExt;
use x11rb::protocol::xproto::{ConnectionExt as XProtoExt, ImageFormat};
use x11rb::rust_connection::RustConnection;

pub struct X11ShmCaptureBackend {
    conn: Option<RustConnection>,
    screen_num: usize,
    monitors: Vec<MonitorDescriptor>,
    active_monitor: Option<MonitorDescriptor>,
    frame_count: u64,
}

impl X11ShmCaptureBackend {
    pub fn new() -> Result<Self, CaptureError> {
        match x11rb::connect(None) {
            Ok((conn, screen_num)) => {
                let mut backend = Self {
                    conn: Some(conn),
                    screen_num,
                    monitors: Vec::new(),
                    active_monitor: None,
                    frame_count: 0,
                };
                let _ = backend.refresh_monitors();
                Ok(backend)
            }
            Err(e) => Err(CaptureError::BackendError(format!(
                "Failed to connect to X11 display: {}",
                e
            ))),
        }
    }

    fn refresh_monitors(&mut self) -> Result<Vec<MonitorDescriptor>, CaptureError> {
        let conn = self
            .conn
            .as_ref()
            .ok_or(CaptureError::DisplayDisconnected)?;
        let setup = conn.setup();
        let screen = setup
            .roots
            .get(self.screen_num)
            .ok_or_else(|| CaptureError::BackendError("Screen not found".to_string()))?;
        let root = screen.root;

        let mut descriptors = Vec::new();

        // Query XRandR outputs
        if let Ok(res_cookie) = conn.randr_get_screen_resources_current(root) {
            if let Ok(resources) = res_cookie.reply() {
                let mut id = 0;
                for &crtc in &resources.crtcs {
                    if let Ok(crtc_info_cookie) =
                        conn.randr_get_crtc_info(crtc, resources.config_timestamp)
                    {
                        if let Ok(crtc_info) = crtc_info_cookie.reply() {
                            if crtc_info.width > 0 && crtc_info.height > 0 {
                                descriptors.push(MonitorDescriptor {
                                    id,
                                    name: format!("XRandR-Output-{}", id),
                                    x: crtc_info.x as i32,
                                    y: crtc_info.y as i32,
                                    width: crtc_info.width as u32,
                                    height: crtc_info.height as u32,
                                    refresh_rate: 60,
                                    is_primary: id == 0,
                                });
                                id += 1;
                            }
                        }
                    }
                }
            }
        }

        // Fallback to primary root screen if RandR returns no active CRTCs
        if descriptors.is_empty() {
            descriptors.push(MonitorDescriptor {
                id: 0,
                name: "X11-Root-Display".to_string(),
                x: 0,
                y: 0,
                width: screen.width_in_pixels as u32,
                height: screen.height_in_pixels as u32,
                refresh_rate: 60,
                is_primary: true,
            });
        }

        self.monitors = descriptors.clone();
        Ok(descriptors)
    }
}

#[async_trait]
impl ScreenCaptureBackend for X11ShmCaptureBackend {
    async fn enumerate_monitors(&self) -> Result<Vec<MonitorDescriptor>, CaptureError> {
        if self.monitors.is_empty() {
            Err(CaptureError::DisplayDisconnected)
        } else {
            Ok(self.monitors.clone())
        }
    }

    async fn start_capture(&mut self, monitor_id: u32) -> Result<(), CaptureError> {
        let mon = self
            .monitors
            .iter()
            .find(|m| m.id == monitor_id)
            .cloned()
            .ok_or(CaptureError::MonitorNotFound(monitor_id))?;
        self.active_monitor = Some(mon);
        self.frame_count = 0;
        info!(
            "Started genuine X11 capture on monitor {}: {}x{} at ({},{})",
            monitor_id,
            self.active_monitor.as_ref().unwrap().width,
            self.active_monitor.as_ref().unwrap().height,
            self.active_monitor.as_ref().unwrap().x,
            self.active_monitor.as_ref().unwrap().y
        );
        Ok(())
    }

    async fn next_frame(&mut self) -> Result<RawFrame, CaptureError> {
        let conn = self
            .conn
            .as_ref()
            .ok_or(CaptureError::DisplayDisconnected)?;
        let monitor = self
            .active_monitor
            .as_ref()
            .ok_or(CaptureError::DisplayDisconnected)?;

        let screen = conn
            .setup()
            .roots
            .get(self.screen_num)
            .ok_or_else(|| CaptureError::BackendError("Screen lost".to_string()))?;
        let root = screen.root;

        // Perform X11 image acquisition
        let reply = conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                root,
                monitor.x as i16,
                monitor.y as i16,
                monitor.width as u16,
                monitor.height as u16,
                !0,
            )
            .map_err(|e| {
                CaptureError::BackendError(format!("X11 get_image request failed: {}", e))
            })?
            .reply()
            .map_err(|e| {
                CaptureError::BackendError(format!("X11 get_image reply failed: {}", e))
            })?;

        self.frame_count += 1;
        let timestamp_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        let stride = monitor.width * 4;
        let data = reply.data;

        Ok(RawFrame {
            width: monitor.width,
            height: monitor.height,
            stride,
            data,
            timestamp_us,
        })
    }

    async fn stop_capture(&mut self) -> Result<(), CaptureError> {
        self.active_monitor = None;
        debug!("Stopped X11 capture session");
        Ok(())
    }
}
