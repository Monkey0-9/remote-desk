use crate::input::{normalize_coordinates, InputError, InputInjectorBackend, MockInputInjector};
use async_trait::async_trait;
use remote_desk_proto::MouseButtonCode;
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, warn};
use zbus::zvariant::{OwnedObjectPath, Value};
use zbus::Connection;

pub const PORTAL_DEST: &str = "org.freedesktop.portal.Desktop";
pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
pub const REMOTEDESKTOP_IFACE: &str = "org.freedesktop.portal.RemoteDesktop";

pub struct RemoteDesktopPortalInjector {
    conn: Option<Connection>,
    session_handle: Option<OwnedObjectPath>,
    fallback: MockInputInjector,
    monitor_width: u32,
    monitor_height: u32,
    held_keys: HashSet<u32>,
    held_buttons: HashSet<MouseButtonCode>,
    total_events_injected: usize,
    allow_mock: bool,
}

impl RemoteDesktopPortalInjector {
    pub fn new(monitor_width: u32, monitor_height: u32) -> Self {
        Self::with_mock_policy(monitor_width, monitor_height, false)
    }

    pub fn with_mock_policy(monitor_width: u32, monitor_height: u32, allow_mock: bool) -> Self {
        Self {
            conn: None,
            session_handle: None,
            fallback: MockInputInjector::new(monitor_width, monitor_height),
            monitor_width,
            monitor_height,
            held_keys: HashSet::new(),
            held_buttons: HashSet::new(),
            total_events_injected: 0,
            allow_mock,
        }
    }

    pub async fn ensure_session(&mut self) {
        if self.conn.is_some() && self.session_handle.is_some() {
            return;
        }

        let conn = match Connection::session().await {
            Ok(c) => c,
            Err(e) => {
                debug!("DBus session connection unavailable: {}", e);
                return;
            }
        };

        // 1. CreateSession
        let mut session_options: HashMap<&str, Value> = HashMap::new();
        let session_token = format!("rd_input_{}", rand::random::<u32>());
        session_options.insert("session_handle_token", Value::from(session_token.as_str()));

        debug!("Requesting RemoteDesktop CreateSession...");
        let create_reply = match conn
            .call_method(
                Some(PORTAL_DEST),
                PORTAL_PATH,
                Some(REMOTEDESKTOP_IFACE),
                "CreateSession",
                &(session_options,),
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                debug!("RemoteDesktop CreateSession unavailable: {}", e);
                return;
            }
        };

        let session_handle: OwnedObjectPath =
            match create_reply.body().deserialize::<OwnedObjectPath>() {
                Ok(p) => p,
                Err(e) => {
                    debug!("Failed to deserialize session handle path: {}", e);
                    return;
                }
            };

        // 2. SelectDevices (1 = keyboard, 2 = pointer, 4 = touchscreen -> 7 = all)
        let mut select_options: HashMap<&str, Value> = HashMap::new();
        select_options.insert("types", Value::from(7u32));

        if let Err(e) = conn
            .call_method(
                Some(PORTAL_DEST),
                PORTAL_PATH,
                Some(REMOTEDESKTOP_IFACE),
                "SelectDevices",
                &(&session_handle, select_options),
            )
            .await
        {
            debug!("RemoteDesktop SelectDevices unavailable: {}", e);
            return;
        }

        // 3. Start Session
        let start_options: HashMap<&str, Value> = HashMap::new();
        if let Err(e) = conn
            .call_method(
                Some(PORTAL_DEST),
                PORTAL_PATH,
                Some(REMOTEDESKTOP_IFACE),
                "Start",
                &(&session_handle, "", start_options),
            )
            .await
        {
            warn!("RemoteDesktop Start user approval unavailable: {}", e);
            return;
        }

        info!(
            "RemoteDesktop portal input session established: {:?}",
            session_handle
        );

        self.conn = Some(conn);
        self.session_handle = Some(session_handle);
    }

    fn button_to_evdev(button: MouseButtonCode) -> i32 {
        match button {
            MouseButtonCode::Left => 0x110,
            MouseButtonCode::Right => 0x111,
            MouseButtonCode::Middle => 0x112,
            MouseButtonCode::Back => 0x115,
            MouseButtonCode::Forward => 0x116,
        }
    }
}

#[async_trait]
impl InputInjectorBackend for RemoteDesktopPortalInjector {
    async fn move_pointer(&mut self, x: f32, y: f32, relative: bool) -> Result<(), InputError> {
        self.ensure_session().await;

        if (self.conn.is_none() || self.session_handle.is_none()) && !self.allow_mock {
            return Err(InputError::InjectionFailed(
                "RemoteDesktop portal input session unavailable on Wayland. User approval required.".to_string(),
            ));
        }

        if let (Some(conn), Some(session_handle)) = (&self.conn, &self.session_handle) {
            let options: HashMap<&str, Value> = HashMap::new();
            if relative {
                let _ = conn
                    .call_method(
                        Some(PORTAL_DEST),
                        PORTAL_PATH,
                        Some(REMOTEDESKTOP_IFACE),
                        "NotifyPointerMotion",
                        &(session_handle, &options, x as f64, y as f64),
                    )
                    .await;
            } else {
                let (px, py) =
                    normalize_coordinates(x, y, self.monitor_width, self.monitor_height)?;
                let _ = conn
                    .call_method(
                        Some(PORTAL_DEST),
                        PORTAL_PATH,
                        Some(REMOTEDESKTOP_IFACE),
                        "NotifyPointerMotionAbsolute",
                        &(session_handle, &options, 0u32, px as f64, py as f64),
                    )
                    .await;
            }
        }

        self.fallback.move_pointer(x, y, relative).await?;
        self.total_events_injected += 1;
        Ok(())
    }

    async fn mouse_button(
        &mut self,
        button: MouseButtonCode,
        down: bool,
    ) -> Result<(), InputError> {
        self.ensure_session().await;

        if let (Some(conn), Some(session_handle)) = (&self.conn, &self.session_handle) {
            let options: HashMap<&str, Value> = HashMap::new();
            let evdev_btn = Self::button_to_evdev(button);
            let state = if down { 1u32 } else { 0u32 };

            let _ = conn
                .call_method(
                    Some(PORTAL_DEST),
                    PORTAL_PATH,
                    Some(REMOTEDESKTOP_IFACE),
                    "NotifyPointerButton",
                    &(session_handle, &options, evdev_btn, state),
                )
                .await;
        }

        if down {
            self.held_buttons.insert(button);
        } else {
            self.held_buttons.remove(&button);
        }

        self.fallback.mouse_button(button, down).await?;
        self.total_events_injected += 1;
        Ok(())
    }

    async fn mouse_wheel(&mut self, delta_x: i16, delta_y: i16) -> Result<(), InputError> {
        self.ensure_session().await;

        if let (Some(conn), Some(session_handle)) = (&self.conn, &self.session_handle) {
            let options: HashMap<&str, Value> = HashMap::new();
            if delta_y != 0 {
                let steps = delta_y as i32;
                let _ = conn
                    .call_method(
                        Some(PORTAL_DEST),
                        PORTAL_PATH,
                        Some(REMOTEDESKTOP_IFACE),
                        "NotifyPointerAxisDiscrete",
                        &(session_handle, &options, 0u32, steps),
                    )
                    .await;
            }
            if delta_x != 0 {
                let steps = delta_x as i32;
                let _ = conn
                    .call_method(
                        Some(PORTAL_DEST),
                        PORTAL_PATH,
                        Some(REMOTEDESKTOP_IFACE),
                        "NotifyPointerAxisDiscrete",
                        &(session_handle, &options, 1u32, steps),
                    )
                    .await;
            }
        }

        self.fallback.mouse_wheel(delta_x, delta_y).await?;
        self.total_events_injected += 1;
        Ok(())
    }

    async fn keyboard_key(&mut self, scancode: u32, down: bool) -> Result<(), InputError> {
        self.ensure_session().await;

        if let (Some(conn), Some(session_handle)) = (&self.conn, &self.session_handle) {
            let options: HashMap<&str, Value> = HashMap::new();
            let state = if down { 1u32 } else { 0u32 };

            let _ = conn
                .call_method(
                    Some(PORTAL_DEST),
                    PORTAL_PATH,
                    Some(REMOTEDESKTOP_IFACE),
                    "NotifyKeyboardKeycode",
                    &(session_handle, &options, scancode as i32, state),
                )
                .await;
        }

        if down {
            self.held_keys.insert(scancode);
        } else {
            self.held_keys.remove(&scancode);
        }

        self.fallback.keyboard_key(scancode, down).await?;
        self.total_events_injected += 1;
        Ok(())
    }

    async fn release_all(&mut self) -> Result<(), InputError> {
        if let (Some(conn), Some(session_handle)) = (&self.conn, &self.session_handle) {
            let options: HashMap<&str, Value> = HashMap::new();
            for &btn in &self.held_buttons.clone() {
                let evdev_btn = Self::button_to_evdev(btn);
                let _ = conn
                    .call_method(
                        Some(PORTAL_DEST),
                        PORTAL_PATH,
                        Some(REMOTEDESKTOP_IFACE),
                        "NotifyPointerButton",
                        &(session_handle, &options, evdev_btn, 0u32),
                    )
                    .await;
            }
            for &key in &self.held_keys.clone() {
                let _ = conn
                    .call_method(
                        Some(PORTAL_DEST),
                        PORTAL_PATH,
                        Some(REMOTEDESKTOP_IFACE),
                        "NotifyKeyboardKeycode",
                        &(session_handle, &options, key as i32, 0u32),
                    )
                    .await;
            }
        }

        self.held_buttons.clear();
        self.held_keys.clear();
        self.fallback.release_all().await?;
        debug!("RemoteDesktop portal injector released all held keys and buttons");
        Ok(())
    }
}
