use async_trait::async_trait;
use remote_desk_proto::MouseButtonCode;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum WindowsInputError {
    #[error("Coordinate out of bounds")]
    OutOfBounds,
    #[error("SendInput API call failed (Win32 error {0})")]
    SendInputFailed(u32),
}

/// Normalizes floating point 0.0..1.0 to Win32 MOUSEEVENTF_ABSOLUTE 0..65535 space
pub fn normalize_win32_coordinates(norm_x: f32, norm_y: f32) -> (u16, u16) {
    let clamped_x = norm_x.clamp(0.0, 1.0);
    let clamped_y = norm_y.clamp(0.0, 1.0);

    let win_x = (clamped_x * 65535.0).round() as u16;
    let win_y = (clamped_y * 65535.0).round() as u16;

    (win_x, win_y)
}

#[async_trait]
pub trait WindowsInputInjectorBackend: Send + Sync {
    async fn move_pointer(&mut self, norm_x: f32, norm_y: f32) -> Result<(), WindowsInputError>;
    async fn move_pointer_relative(&mut self, dx: i16, dy: i16) -> Result<(), WindowsInputError>;
    async fn mouse_button(
        &mut self,
        button: MouseButtonCode,
        down: bool,
    ) -> Result<(), WindowsInputError>;
    async fn mouse_wheel(&mut self, delta: i16) -> Result<(), WindowsInputError>;
    async fn keyboard_key(&mut self, scancode: u32, down: bool) -> Result<(), WindowsInputError>;
    async fn release_all(&mut self) -> Result<(), WindowsInputError>;
}

/// Simulated Windows SendInput injector for testing and verification
pub struct SendInputInjector {
    pub current_x: u16,
    pub current_y: u16,
    pub held_keys: HashSet<u32>,
    pub held_buttons: HashSet<MouseButtonCode>,
    pub injected_count: usize,
}

impl SendInputInjector {
    pub fn new() -> Self {
        Self {
            current_x: 32768, // Center of screen
            current_y: 32768,
            held_keys: HashSet::new(),
            held_buttons: HashSet::new(),
            injected_count: 0,
        }
    }
}

impl Default for SendInputInjector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WindowsInputInjectorBackend for SendInputInjector {
    async fn move_pointer(&mut self, norm_x: f32, norm_y: f32) -> Result<(), WindowsInputError> {
        let (x, y) = normalize_win32_coordinates(norm_x, norm_y);
        self.current_x = x;
        self.current_y = y;
        self.injected_count += 1;
        Ok(())
    }

    async fn move_pointer_relative(&mut self, dx: i16, dy: i16) -> Result<(), WindowsInputError> {
        self.current_x = (self.current_x as i32 + dx as i32).clamp(0, 65535) as u16;
        self.current_y = (self.current_y as i32 + dy as i32).clamp(0, 65535) as u16;
        self.injected_count += 1;
        Ok(())
    }

    async fn mouse_button(
        &mut self,
        button: MouseButtonCode,
        down: bool,
    ) -> Result<(), WindowsInputError> {
        if down {
            self.held_buttons.insert(button);
        } else {
            self.held_buttons.remove(&button);
        }
        self.injected_count += 1;
        Ok(())
    }

    async fn mouse_wheel(&mut self, _delta: i16) -> Result<(), WindowsInputError> {
        self.injected_count += 1;
        Ok(())
    }

    async fn keyboard_key(&mut self, scancode: u32, down: bool) -> Result<(), WindowsInputError> {
        if down {
            self.held_keys.insert(scancode);
        } else {
            self.held_keys.remove(&scancode);
        }
        self.injected_count += 1;
        Ok(())
    }

    async fn release_all(&mut self) -> Result<(), WindowsInputError> {
        let released = self.held_keys.len() + self.held_buttons.len();
        self.held_keys.clear();
        self.held_buttons.clear();
        self.injected_count += released;
        Ok(())
    }
}
