use crate::input::{normalize_coordinates, InputError, InputInjectorBackend};
use async_trait::async_trait;
use remote_desk_proto::MouseButtonCode;
use std::collections::HashSet;
use tracing::{debug, info};
use x11rb::connection::Connection;
use x11rb::protocol::xtest::ConnectionExt as XTestExt;
use x11rb::rust_connection::RustConnection;

pub struct XTestInjector {
    conn: RustConnection,
    screen_num: usize,
    monitor_width: u32,
    monitor_height: u32,
    held_keys: HashSet<u32>,
    held_buttons: HashSet<MouseButtonCode>,
}

impl XTestInjector {
    pub fn new(monitor_width: u32, monitor_height: u32) -> Result<Self, InputError> {
        let (conn, screen_num) = x11rb::connect(None).map_err(|e| {
            InputError::InjectionFailed(format!("Cannot connect to X11 display: {}", e))
        })?;

        // Query XTest extension availability
        let xtest_avail = conn
            .xtest_get_version(2, 2)
            .map_err(|e| InputError::InjectionFailed(format!("XTest query failed: {}", e)))?
            .reply()
            .map_err(|e| InputError::InjectionFailed(format!("XTest reply failed: {}", e)))?;

        info!(
            "Initialized genuine X11 XTest injector (v{}.{})",
            xtest_avail.major_version, xtest_avail.minor_version
        );

        Ok(Self {
            conn,
            screen_num,
            monitor_width,
            monitor_height,
            held_keys: HashSet::new(),
            held_buttons: HashSet::new(),
        })
    }

    fn to_x11_button(button: MouseButtonCode) -> u8 {
        match button {
            MouseButtonCode::Left => 1,
            MouseButtonCode::Middle => 2,
            MouseButtonCode::Right => 3,
            MouseButtonCode::Back => 8,
            MouseButtonCode::Forward => 9,
        }
    }
}

#[async_trait]
impl InputInjectorBackend for XTestInjector {
    async fn move_pointer(&mut self, x: f32, y: f32, relative: bool) -> Result<(), InputError> {
        let (px, py) = if relative {
            (x as i16, y as i16)
        } else {
            let (abs_x, abs_y) =
                normalize_coordinates(x, y, self.monitor_width, self.monitor_height)?;
            (abs_x as i16, abs_y as i16)
        };

        let screen = self
            .conn
            .setup()
            .roots
            .get(self.screen_num)
            .ok_or_else(|| InputError::InjectionFailed("Screen lost".to_string()))?;
        let root = screen.root;

        let event_type = 6u8; // MotionNotify
        let detail = if relative { 1 } else { 0 }; // relative vs absolute

        self.conn
            .xtest_fake_input(event_type, detail, 0, root, px, py, 0)
            .map_err(|e| InputError::InjectionFailed(format!("Pointer move failed: {}", e)))?;
        let _ = self.conn.flush();
        Ok(())
    }

    async fn mouse_button(
        &mut self,
        button: MouseButtonCode,
        down: bool,
    ) -> Result<(), InputError> {
        let screen = self
            .conn
            .setup()
            .roots
            .get(self.screen_num)
            .ok_or_else(|| InputError::InjectionFailed("Screen lost".to_string()))?;
        let root = screen.root;

        let event_type = if down { 4u8 } else { 5u8 }; // ButtonPress vs ButtonRelease
        let detail = Self::to_x11_button(button);

        self.conn
            .xtest_fake_input(event_type, detail, 0, root, 0, 0, 0)
            .map_err(|e| InputError::InjectionFailed(format!("Button event failed: {}", e)))?;
        let _ = self.conn.flush();

        if down {
            self.held_buttons.insert(button);
        } else {
            self.held_buttons.remove(&button);
        }
        Ok(())
    }

    async fn mouse_wheel(&mut self, _delta_x: i16, delta_y: i16) -> Result<(), InputError> {
        let screen = self
            .conn
            .setup()
            .roots
            .get(self.screen_num)
            .ok_or_else(|| InputError::InjectionFailed("Screen lost".to_string()))?;
        let root = screen.root;

        // In X11, wheel up is button 4, wheel down is button 5
        let btn = if delta_y > 0 { 4u8 } else { 5u8 };

        // Emulate click (press then release)
        self.conn
            .xtest_fake_input(4, btn, 0, root, 0, 0, 0)
            .map_err(|e| InputError::InjectionFailed(format!("Wheel press failed: {}", e)))?;
        self.conn
            .xtest_fake_input(5, btn, 0, root, 0, 0, 0)
            .map_err(|e| InputError::InjectionFailed(format!("Wheel release failed: {}", e)))?;
        let _ = self.conn.flush();
        Ok(())
    }

    async fn keyboard_key(&mut self, scancode: u32, down: bool) -> Result<(), InputError> {
        let screen = self
            .conn
            .setup()
            .roots
            .get(self.screen_num)
            .ok_or_else(|| InputError::InjectionFailed("Screen lost".to_string()))?;
        let root = screen.root;

        let event_type = if down { 2u8 } else { 3u8 }; // KeyPress vs KeyRelease
        let keycode = (scancode & 0xFF) as u8;

        self.conn
            .xtest_fake_input(event_type, keycode, 0, root, 0, 0, 0)
            .map_err(|e| InputError::InjectionFailed(format!("Key event failed: {}", e)))?;
        let _ = self.conn.flush();

        if down {
            self.held_keys.insert(scancode);
        } else {
            self.held_keys.remove(&scancode);
        }
        Ok(())
    }

    async fn release_all(&mut self) -> Result<(), InputError> {
        if self.held_keys.is_empty() && self.held_buttons.is_empty() {
            return Ok(());
        }

        let screen = self
            .conn
            .setup()
            .roots
            .get(self.screen_num)
            .ok_or_else(|| InputError::InjectionFailed("Screen lost".to_string()))?;
        let root = screen.root;

        // Release all held buttons
        let buttons: Vec<MouseButtonCode> = self.held_buttons.drain().collect();
        for btn in buttons {
            let detail = Self::to_x11_button(btn);
            let _ = self.conn.xtest_fake_input(5, detail, 0, root, 0, 0, 0);
        }

        // Release all held keys
        let keys: Vec<u32> = self.held_keys.drain().collect();
        for scancode in keys {
            let keycode = (scancode & 0xFF) as u8;
            let _ = self.conn.xtest_fake_input(3, keycode, 0, root, 0, 0, 0);
        }

        let _ = self.conn.flush();
        debug!("Released all held keys and buttons via XTest fail-safe");
        Ok(())
    }
}
