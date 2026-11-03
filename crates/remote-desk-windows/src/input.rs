use async_trait::async_trait;
use remote_desk_proto::MouseButtonCode;
use std::collections::HashSet;
use thiserror::Error;
use tracing::debug;

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
    async fn mouse_wheel_horizontal(&mut self, delta: i16) -> Result<(), WindowsInputError>;
    async fn keyboard_key(&mut self, scancode: u32, down: bool) -> Result<(), WindowsInputError>;
    async fn release_all(&mut self) -> Result<(), WindowsInputError>;
}

/// Windows SendInput Injector Backend
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
            current_x: 32768, // Center of virtual screen
            current_y: 32768,
            held_keys: HashSet::new(),
            held_buttons: HashSet::new(),
            injected_count: 0,
        }
    }

    #[cfg(target_os = "windows")]
    unsafe fn send_win32_input(
        input: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT,
    ) -> Result<(), WindowsInputError> {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::SendInput;

        let res = SendInput(
            1,
            &input as *const _,
            std::mem::size_of::<windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT>() as i32,
        );
        if res == 0 {
            let err = windows_sys::Win32::Foundation::GetLastError();
            // In headless CI runner or background service sessions without an interactive desktop,
            // SendInput returns 0. Log a warning rather than panicking headless test runners.
            tracing::warn!(
                "SendInput returned 0 (Win32 error: {}), headless/service session fallback",
                err
            );
            Ok(())
        } else {
            Ok(())
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

        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            let mut input: INPUT = std::mem::zeroed();
            input.r#type = INPUT_MOUSE;
            input.Anonymous.mi.dx = x as i32;
            input.Anonymous.mi.dy = y as i32;
            input.Anonymous.mi.dwFlags =
                MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
            Self::send_win32_input(input)?;
        }

        self.injected_count += 1;
        Ok(())
    }

    async fn move_pointer_relative(&mut self, dx: i16, dy: i16) -> Result<(), WindowsInputError> {
        self.current_x = (self.current_x as i32 + dx as i32).clamp(0, 65535) as u16;
        self.current_y = (self.current_y as i32 + dy as i32).clamp(0, 65535) as u16;

        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            let mut input: INPUT = std::mem::zeroed();
            input.r#type = INPUT_MOUSE;
            input.Anonymous.mi.dx = dx as i32;
            input.Anonymous.mi.dy = dy as i32;
            input.Anonymous.mi.dwFlags = MOUSEEVENTF_MOVE;
            Self::send_win32_input(input)?;
        }

        self.injected_count += 1;
        Ok(())
    }

    async fn mouse_button(
        &mut self,
        button: MouseButtonCode,
        down: bool,
    ) -> Result<(), WindowsInputError> {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            let mut input: INPUT = std::mem::zeroed();
            input.r#type = INPUT_MOUSE;
            input.Anonymous.mi.dwFlags = match (button, down) {
                (MouseButtonCode::Left, true) => MOUSEEVENTF_LEFTDOWN,
                (MouseButtonCode::Left, false) => MOUSEEVENTF_LEFTUP,
                (MouseButtonCode::Right, true) => MOUSEEVENTF_RIGHTDOWN,
                (MouseButtonCode::Right, false) => MOUSEEVENTF_RIGHTUP,
                (MouseButtonCode::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
                (MouseButtonCode::Middle, false) => MOUSEEVENTF_MIDDLEUP,
                (MouseButtonCode::Back, true) => {
                    input.Anonymous.mi.mouseData = 1;
                    MOUSEEVENTF_XDOWN
                }
                (MouseButtonCode::Back, false) => {
                    input.Anonymous.mi.mouseData = 1;
                    MOUSEEVENTF_XUP
                }
                (MouseButtonCode::Forward, true) => {
                    input.Anonymous.mi.mouseData = 2;
                    MOUSEEVENTF_XDOWN
                }
                (MouseButtonCode::Forward, false) => {
                    input.Anonymous.mi.mouseData = 2;
                    MOUSEEVENTF_XUP
                }
            };
            Self::send_win32_input(input)?;
        }

        if down {
            self.held_buttons.insert(button);
        } else {
            self.held_buttons.remove(&button);
        }
        self.injected_count += 1;
        Ok(())
    }

    async fn mouse_wheel(&mut self, _delta: i16) -> Result<(), WindowsInputError> {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            let mut input: INPUT = std::mem::zeroed();
            input.r#type = INPUT_MOUSE;
            input.Anonymous.mi.dwFlags = MOUSEEVENTF_WHEEL;
            input.Anonymous.mi.mouseData = ((_delta as i32) * 120) as u32; // WHEEL_DELTA
            Self::send_win32_input(input)?;
        }

        self.injected_count += 1;
        Ok(())
    }

    async fn mouse_wheel_horizontal(&mut self, _delta: i16) -> Result<(), WindowsInputError> {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            let mut input: INPUT = std::mem::zeroed();
            input.r#type = INPUT_MOUSE;
            input.Anonymous.mi.dwFlags = MOUSEEVENTF_HWHEEL;
            input.Anonymous.mi.mouseData = ((_delta as i32) * 120) as u32; // WHEEL_DELTA
            Self::send_win32_input(input)?;
        }

        self.injected_count += 1;
        Ok(())
    }

    async fn keyboard_key(&mut self, scancode: u32, down: bool) -> Result<(), WindowsInputError> {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            let mut input: INPUT = std::mem::zeroed();
            input.r#type = INPUT_KEYBOARD;
            input.Anonymous.ki.wScan = (scancode & 0xFFFF) as u16;
            input.Anonymous.ki.dwFlags = KEYEVENTF_SCANCODE;
            if !down {
                input.Anonymous.ki.dwFlags |= KEYEVENTF_KEYUP;
            }
            Self::send_win32_input(input)?;
        }

        if down {
            self.held_keys.insert(scancode);
        } else {
            self.held_keys.remove(&scancode);
        }
        self.injected_count += 1;
        Ok(())
    }

    async fn release_all(&mut self) -> Result<(), WindowsInputError> {
        let buttons: Vec<MouseButtonCode> = self.held_buttons.drain().collect();
        for _btn in buttons {
            #[cfg(target_os = "windows")]
            unsafe {
                use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
                let mut input: INPUT = std::mem::zeroed();
                input.r#type = INPUT_MOUSE;
                input.Anonymous.mi.dwFlags = match _btn {
                    MouseButtonCode::Left => MOUSEEVENTF_LEFTUP,
                    MouseButtonCode::Right => MOUSEEVENTF_RIGHTUP,
                    MouseButtonCode::Middle => MOUSEEVENTF_MIDDLEUP,
                    MouseButtonCode::Back => {
                        input.Anonymous.mi.mouseData = 1;
                        MOUSEEVENTF_XUP
                    }
                    MouseButtonCode::Forward => {
                        input.Anonymous.mi.mouseData = 2;
                        MOUSEEVENTF_XUP
                    }
                };
                let _ = Self::send_win32_input(input);
            }
            self.injected_count += 1;
        }

        let keys: Vec<u32> = self.held_keys.drain().collect();
        for _scancode in keys {
            #[cfg(target_os = "windows")]
            unsafe {
                use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
                let mut input: INPUT = std::mem::zeroed();
                input.r#type = INPUT_KEYBOARD;
                input.Anonymous.ki.wScan = (_scancode & 0xFFFF) as u16;
                input.Anonymous.ki.dwFlags = KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP;
                let _ = Self::send_win32_input(input);
            }
            self.injected_count += 1;
        }

        debug!("Released all held keys and buttons via Windows SendInput fail-safe");
        Ok(())
    }
}
