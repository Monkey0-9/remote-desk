use crate::input::{normalize_coordinates, InputError, InputInjectorBackend};
use async_trait::async_trait;
use remote_desk_proto::MouseButtonCode;
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use tracing::{debug, info};

const UI_SET_EVBIT: libc::c_ulong = 0x40045564;
const UI_SET_KEYBIT: libc::c_ulong = 0x40045565;
const UI_SET_RELBIT: libc::c_ulong = 0x40045566;
const UI_SET_ABSBIT: libc::c_ulong = 0x40045567;
const UI_DEV_CREATE: libc::c_ulong = 0x5501;
const UI_DEV_DESTROY: libc::c_ulong = 0x5502;

const EV_SYN: libc::c_int = 0x00;
const EV_KEY: libc::c_int = 0x01;
const EV_REL: libc::c_int = 0x02;
const EV_ABS: libc::c_int = 0x03;

const REL_X: libc::c_int = 0x00;
const REL_Y: libc::c_int = 0x01;
const REL_HWHEEL: libc::c_int = 0x06;
const REL_WHEEL: libc::c_int = 0x08;

const ABS_X: usize = 0x00;
const ABS_Y: usize = 0x01;

const BTN_LEFT: libc::c_int = 0x110;
const BTN_RIGHT: libc::c_int = 0x111;
const BTN_MIDDLE: libc::c_int = 0x112;
const BTN_SIDE: libc::c_int = 0x115;
const BTN_EXTRA: libc::c_int = 0x116;

#[repr(C)]
struct UinputUserDev {
    name: [libc::c_char; 80],
    id_bustype: u16,
    id_vendor: u16,
    id_product: u16,
    id_version: u16,
    ff_effects_max: u32,
    absmax: [i32; 64],
    absmin: [i32; 64],
    absfuzz: [i32; 64],
    absflat: [i32; 64],
}

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
                InputError::InjectionFailed(format!(
                    "Failed to open /dev/uinput: {}. To enable Linux native input injection without running as root, add the udev rule 'KERNEL==\"uinput\", SUBSYSTEM==\"misc\", MODE=\"0660\", GROUP=\"input\"' and ensure current user is added to group 'input'.",
                    e
                ))
            })?;

        let fd = file.as_raw_fd();
        unsafe {
            // Configure event types
            libc::ioctl(fd, UI_SET_EVBIT, EV_KEY);
            libc::ioctl(fd, UI_SET_EVBIT, EV_REL);
            libc::ioctl(fd, UI_SET_EVBIT, EV_ABS);
            libc::ioctl(fd, UI_SET_EVBIT, EV_SYN);

            // Enable mouse buttons
            for btn in [BTN_LEFT, BTN_RIGHT, BTN_MIDDLE, BTN_SIDE, BTN_EXTRA] {
                libc::ioctl(fd, UI_SET_KEYBIT, btn);
            }
            // Enable keyboard keys (1..255)
            for k in 1..256 {
                libc::ioctl(fd, UI_SET_KEYBIT, k as libc::c_int);
            }

            // Enable relative pointer and wheel axes
            for rel in [REL_X, REL_Y, REL_WHEEL, REL_HWHEEL] {
                libc::ioctl(fd, UI_SET_RELBIT, rel);
            }

            // Enable absolute pointer axes
            libc::ioctl(fd, UI_SET_ABSBIT, ABS_X as libc::c_int);
            libc::ioctl(fd, UI_SET_ABSBIT, ABS_Y as libc::c_int);

            // Write user dev setup
            let mut udev: UinputUserDev = std::mem::zeroed();
            let dev_name = b"RemoteDesk Virtual Input\0";
            for (i, &b) in dev_name.iter().enumerate() {
                udev.name[i] = b as libc::c_char;
            }
            udev.id_bustype = 0x03; // BUS_USB
            udev.id_vendor = 0x1234;
            udev.id_product = 0x5678;
            udev.id_version = 1;
            udev.absmax[ABS_X] = monitor_width as i32;
            udev.absmax[ABS_Y] = monitor_height as i32;

            let written = libc::write(
                fd,
                &udev as *const _ as *const libc::c_void,
                std::mem::size_of::<UinputUserDev>(),
            );
            if written < 0 {
                return Err(InputError::InjectionFailed(
                    "Failed to write uinput_user_dev structure".to_string(),
                ));
            }

            let create_res = libc::ioctl(fd, UI_DEV_CREATE);
            if create_res < 0 {
                return Err(InputError::InjectionFailed(
                    "UI_DEV_CREATE ioctl failed".to_string(),
                ));
            }
        }

        info!("Successfully initialized genuine /dev/uinput virtual device node");

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

impl Drop for UinputInjector {
    fn drop(&mut self) {
        if let Some(ref file) = self.file {
            let fd = file.as_raw_fd();
            unsafe {
                let _ = libc::ioctl(fd, UI_DEV_DESTROY);
            }
            debug!("Destroyed uinput virtual input device handle");
        }
    }
}
