use async_trait::async_trait;
use remote_desk_proto::MouseButtonCode;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum InputError {
    #[error("Target monitor resolution not configured")]
    ResolutionNotSet,
    #[error("Coordinate out of bounds: ({x}, {y})")]
    OutOfBounds { x: f32, y: f32 },
    #[error("Input injection rejected: {0}")]
    InjectionFailed(String),
}

/// Normalizes floating point 0.0..1.0 coordinates to target screen pixel coordinates
pub fn normalize_coordinates(
    norm_x: f32,
    norm_y: f32,
    monitor_width: u32,
    monitor_height: u32,
) -> Result<(i32, i32), InputError> {
    if monitor_width == 0 || monitor_height == 0 {
        return Err(InputError::ResolutionNotSet);
    }

    let clamped_x = norm_x.clamp(0.0, 1.0);
    let clamped_y = norm_y.clamp(0.0, 1.0);

    let pixel_x = (clamped_x * (monitor_width.saturating_sub(1) as f32)).round() as i32;
    let pixel_y = (clamped_y * (monitor_height.saturating_sub(1) as f32)).round() as i32;

    Ok((pixel_x, pixel_y))
}

#[async_trait]
pub trait InputInjectorBackend: Send + Sync {
    async fn move_pointer(&mut self, x: f32, y: f32, relative: bool) -> Result<(), InputError>;
    async fn mouse_button(&mut self, button: MouseButtonCode, down: bool)
        -> Result<(), InputError>;
    async fn mouse_wheel(&mut self, delta_x: i16, delta_y: i16) -> Result<(), InputError>;
    async fn keyboard_key(&mut self, scancode: u32, down: bool) -> Result<(), InputError>;
    async fn release_all(&mut self) -> Result<(), InputError>;
}

/// Simulated input injector for testing and verification
pub struct MockInputInjector {
    pub monitor_width: u32,
    pub monitor_height: u32,
    pub current_cursor_x: i32,
    pub current_cursor_y: i32,
    pub held_keys: HashSet<u32>,
    pub held_buttons: HashSet<MouseButtonCode>,
    pub total_events_injected: usize,
}

impl MockInputInjector {
    pub fn new(monitor_width: u32, monitor_height: u32) -> Self {
        Self {
            monitor_width,
            monitor_height,
            current_cursor_x: 0,
            current_cursor_y: 0,
            held_keys: HashSet::new(),
            held_buttons: HashSet::new(),
            total_events_injected: 0,
        }
    }

    pub fn held_keys_count(&self) -> usize {
        self.held_keys.len()
    }

    pub fn held_buttons_count(&self) -> usize {
        self.held_buttons.len()
    }
}

#[async_trait]
impl InputInjectorBackend for MockInputInjector {
    async fn move_pointer(&mut self, x: f32, y: f32, relative: bool) -> Result<(), InputError> {
        if relative {
            self.current_cursor_x = (self.current_cursor_x + x as i32)
                .clamp(0, self.monitor_width.saturating_sub(1) as i32);
            self.current_cursor_y = (self.current_cursor_y + y as i32)
                .clamp(0, self.monitor_height.saturating_sub(1) as i32);
        } else {
            let (px, py) = normalize_coordinates(x, y, self.monitor_width, self.monitor_height)?;
            self.current_cursor_x = px;
            self.current_cursor_y = py;
        }
        self.total_events_injected += 1;
        Ok(())
    }

    async fn mouse_button(
        &mut self,
        button: MouseButtonCode,
        down: bool,
    ) -> Result<(), InputError> {
        if down {
            self.held_buttons.insert(button);
        } else {
            self.held_buttons.remove(&button);
        }
        self.total_events_injected += 1;
        Ok(())
    }

    async fn mouse_wheel(&mut self, _delta_x: i16, _delta_y: i16) -> Result<(), InputError> {
        self.total_events_injected += 1;
        Ok(())
    }

    async fn keyboard_key(&mut self, scancode: u32, down: bool) -> Result<(), InputError> {
        if down {
            self.held_keys.insert(scancode);
        } else {
            self.held_keys.remove(&scancode);
        }
        self.total_events_injected += 1;
        Ok(())
    }

    async fn release_all(&mut self) -> Result<(), InputError> {
        let keys_released = self.held_keys.len();
        let buttons_released = self.held_buttons.len();
        self.held_keys.clear();
        self.held_buttons.clear();
        self.total_events_injected += keys_released + buttons_released;
        Ok(())
    }
}

pub use crate::uinput_injector::UinputInjector;
pub use crate::xtest_injector::XTestInjector;

/// Linux Input Injector Factory
///
/// Prioritizes native Linux `/dev/uinput` injector, falls back to X11 XTest injector,
/// and finally provides `MockInputInjector` in headless/CI environments.
pub fn create_linux_input_injector(
    monitor_width: u32,
    monitor_height: u32,
) -> Box<dyn InputInjectorBackend> {
    if let Ok(uinput) = UinputInjector::new(monitor_width, monitor_height) {
        Box::new(uinput)
    } else if let Ok(xtest) = XTestInjector::new(monitor_width, monitor_height) {
        Box::new(xtest)
    } else {
        Box::new(MockInputInjector::new(monitor_width, monitor_height))
    }
}
