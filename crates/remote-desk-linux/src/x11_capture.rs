use crate::capture::{CaptureError, RawFrame, ScreenCaptureBackend};
use async_trait::async_trait;
use remote_desk_proto::MonitorDescriptor;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info};
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as RandrExt;
use x11rb::protocol::shm::ConnectionExt as ShmExt;
use x11rb::protocol::xproto::{ConnectionExt as XProtoExt, ImageFormat};
use x11rb::rust_connection::RustConnection;

struct ShmHandle {
    shmid: i32,
    shmaddr: *mut u8,
    shmseg: u32,
    size: usize,
}

// Access is guarded by mutable reference &mut self on X11ShmCaptureBackend
unsafe impl Send for ShmHandle {}
unsafe impl Sync for ShmHandle {}

impl Drop for ShmHandle {
    fn drop(&mut self) {
        if !self.shmaddr.is_null() && self.shmaddr != libc::MAP_FAILED as *mut u8 {
            unsafe {
                libc::shmdt(self.shmaddr as *const _);
            }
        }
        if self.shmid >= 0 {
            unsafe {
                libc::shmctl(self.shmid, libc::IPC_RMID, std::ptr::null_mut());
            }
        }
    }
}

pub struct X11ShmCaptureBackend {
    conn: Option<RustConnection>,
    screen_num: usize,
    monitors: Vec<MonitorDescriptor>,
    active_monitor: Option<MonitorDescriptor>,
    shm: Option<ShmHandle>,
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
                    shm: None,
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

    fn init_shm(&mut self, width: u32, height: u32) {
        let conn = match self.conn.as_ref() {
            Some(c) => c,
            None => return,
        };

        // Check if SHM is supported by X server
        if conn.shm_query_version().is_err() {
            debug!("X11 server does not support SHM extension; using get_image fallback");
            return;
        }

        let stride = width * 4;
        let size = (stride * height) as usize;

        let shmid = unsafe { libc::shmget(libc::IPC_PRIVATE, size, libc::IPC_CREAT | 0o600) };
        if shmid < 0 {
            debug!("libc::shmget failed for size {}", size);
            return;
        }

        let shmaddr = unsafe { libc::shmat(shmid, std::ptr::null(), 0) as *mut u8 };
        if shmaddr.is_null() || shmaddr == libc::MAP_FAILED as *mut u8 {
            unsafe {
                libc::shmctl(shmid, libc::IPC_RMID, std::ptr::null_mut());
            }
            debug!("libc::shmat failed");
            return;
        }

        let shmseg = match conn.generate_id() {
            Ok(id) => id,
            Err(_) => {
                unsafe {
                    libc::shmdt(shmaddr as *const _);
                    libc::shmctl(shmid, libc::IPC_RMID, std::ptr::null_mut());
                }
                return;
            }
        };

        if let Err(e) = conn.shm_attach(shmseg, shmid as u32, false) {
            unsafe {
                libc::shmdt(shmaddr as *const _);
                libc::shmctl(shmid, libc::IPC_RMID, std::ptr::null_mut());
            }
            debug!("X11 shm_attach failed: {}", e);
            return;
        }

        info!("Zero-copy MIT-SHM segment initialized: {} bytes", size);
        self.shm = Some(ShmHandle {
            shmid,
            shmaddr,
            shmseg,
            size,
        });
    }

    fn cleanup_shm(&mut self) {
        if let Some(shm) = self.shm.take() {
            if let Some(conn) = self.conn.as_ref() {
                let _ = conn.shm_detach(shm.shmseg);
            }
            // ShmHandle Drop handles shmdt and shmctl
        }
    }
}

impl Drop for X11ShmCaptureBackend {
    fn drop(&mut self) {
        self.cleanup_shm();
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

        self.cleanup_shm();
        self.init_shm(mon.width, mon.height);

        self.active_monitor = Some(mon.clone());
        self.frame_count = 0;
        info!(
            "Started X11 capture on monitor {}: {}x{} at ({},{}) (SHM zero-copy: {})",
            monitor_id,
            mon.width,
            mon.height,
            mon.x,
            mon.y,
            self.shm.is_some()
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

        let width = monitor.width;
        let height = monitor.height;
        let stride = width * 4;
        let timestamp_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        // Path A: Zero-copy MIT-SHM
        if let Some(shm) = &self.shm {
            if let Ok(cookie) = conn.shm_get_image(
                root,
                monitor.x as i16,
                monitor.y as i16,
                width as u16,
                height as u16,
                !0,
                ImageFormat::Z_PIXMAP.into(),
                shm.shmseg,
                0,
            ) {
                if cookie.reply().is_ok() {
                    let mut data = vec![0u8; shm.size];
                    unsafe {
                        std::ptr::copy_nonoverlapping(shm.shmaddr, data.as_mut_ptr(), shm.size);
                    }
                    self.frame_count += 1;
                    return Ok(RawFrame {
                        width,
                        height,
                        stride,
                        data,
                        timestamp_us,
                    });
                }
            }
        }

        // Path B: Standard X11 get_image fallback
        let reply = conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                root,
                monitor.x as i16,
                monitor.y as i16,
                width as u16,
                height as u16,
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
        let data = reply.data;

        Ok(RawFrame {
            width,
            height,
            stride,
            data,
            timestamp_us,
        })
    }

    async fn stop_capture(&mut self) -> Result<(), CaptureError> {
        self.cleanup_shm();
        self.active_monitor = None;
        debug!("Stopped X11 capture session");
        Ok(())
    }
}
