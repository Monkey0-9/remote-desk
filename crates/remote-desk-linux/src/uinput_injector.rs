use crate::input::{normalize_coordinates, InputError, InputInjectorBackend};
use async_trait::async_trait;
use remote_desk_proto::MouseButtonCode;
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use tracing::{debug, info};

pub struct UinputInjector {
    file: Option<File>,
    monitor_width: u32,
    monitor_height: u32,
    held_keys: HashSet<u32>,
    held_buttons: HashSet<MouseButtonCode>,
}

impl UinputInjector {
    pub fn new(monitor_width: u32, monitor_height: u32) -> Result<Self, InputError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uinput")
            .or_else(|_| {
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open("/dev/input/uinput")
            })
            .map_err(|e| {
                InputError::InjectionFailed(format!("Failed to open /dev/uinput: {}", e))
            })?;

        info!("Successfully opened /dev/uinput device handle");

        Ok(Self {
            file: Some(file),
            monitor_width,
            monitor_height,
            held_keys: HashSet::new(),
            held_buttons: HashSet::new(),
        })
    }

    fn write_event(&self, type_: u16, code: u16, value: i32) -> Result<(), InputError> {
        if let Some(ref file) = self.file {
            let fd = file.as_raw_fd();
            let mut ev: libc::input_event = unsafe { std::mem::zeroed() };
            ev.type_ = type_;
            ev.code = code;
            ev.value = value;

            let bytes = unsafe {
                std::slice::from_raw_parts(
                    &ev as *const _ as *const u8,
                    std::mem::size_of::<libc::input_event>(),
                )
            };

            let written =
                unsafe { libc::write(fd, bytes.as_ptr() as *const libc::c_void, bytes.len()) };

            if written < 0 {
                return Err(InputError::InjectionFailed(
                    "uinput write failed".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn sync(&self) -> Result<(), InputError> {
        self.write_event(0, 0, 0) // EV_SYN, SYN_REPORT
    }
}

#[async_trait]
impl InputInjectorBackend for UinputInjector {
    async fn move_pointer(&mut self, x: f32, y: f32, relative: bool) -> Result<(), InputError> {
        if relative {
            self.write_event(2, 0, x as i32)?; // EV_REL, REL_X
            self.write_event(2, 1, y as i32)?; // EV_REL, REL_Y
        } else {
            let (abs_x, abs_y) =
                normalize_coordinates(x, y, self.monitor_width, self.monitor_height)?;
            self.write_event(3, 0, abs_x)?; // EV_ABS, ABS_X
            self.write_event(3, 1, abs_y)?; // EV_ABS, ABS_Y
        }
        self.sync()
    }

    async fn mouse_button(
        &mut self,
        button: MouseButtonCode,
        down: bool,
    ) -> Result<(), InputError> {
        let code = match button {
            MouseButtonCode::Left => 0x110,    // BTN_LEFT
            MouseButtonCode::Right => 0x111,   // BTN_RIGHT
            MouseButtonCode::Middle => 0x112,  // BTN_MIDDLE
            MouseButtonCode::Back => 0x115,    // BTN_SIDE
            MouseButtonCode::Forward => 0x116, // BTN_EXTRA
        };

        let val = if down { 1 } else { 0 };
        self.write_event(1, code, val)?; // EV_KEY
        self.sync()?;

        if down {
            self.held_buttons.insert(button);
        } else {
            self.held_buttons.remove(&button);
        }
        Ok(())
    }

    async fn mouse_wheel(&mut self, delta_x: i16, delta_y: i16) -> Result<(), InputError> {
        if delta_y != 0 {
            self.write_event(2, 8, delta_y as i32)?; // EV_REL, REL_WHEEL
        }
        if delta_x != 0 {
            self.write_event(2, 6, delta_x as i32)?; // EV_REL, REL_HWHEEL
        }
        self.sync()
    }

    async fn keyboard_key(&mut self, scancode: u32, down: bool) -> Result<(), InputError> {
        let val = if down { 1 } else { 0 };
        self.write_event(1, (scancode & 0xFFFF) as u16, val)?; // EV_KEY
        self.sync()?;

        if down {
            self.held_keys.insert(scancode);
        } else {
            self.held_keys.remove(&scancode);
        }
        Ok(())
    }

    async fn release_all(&mut self) -> Result<(), InputError> {
        let buttons: Vec<MouseButtonCode> = self.held_buttons.drain().collect();
        for btn in buttons {
            let code = match btn {
                MouseButtonCode::Left => 0x110,
                MouseButtonCode::Right => 0x111,
                MouseButtonCode::Middle => 0x112,
                MouseButtonCode::Back => 0x115,
                MouseButtonCode::Forward => 0x116,
            };
            let _ = self.write_event(1, code, 0);
        }

        let keys: Vec<u32> = self.held_keys.drain().collect();
        for scancode in keys {
            let _ = self.write_event(1, (scancode & 0xFFFF) as u16, 0);
        }

        let _ = self.sync();
        debug!("Released all held keys and buttons via uinput fail-safe");
        Ok(())
    }
}
