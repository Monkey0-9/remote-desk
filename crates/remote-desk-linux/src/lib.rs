pub mod capture;
pub mod input;

pub use capture::{
    create_linux_capture_backend, detect_display_server, CaptureError, LinuxDisplayServer,
    MockCaptureBackend, PortalCaptureBackend, RawFrame, ScreenCaptureBackend,
};
pub use input::{
    create_linux_input_injector, normalize_coordinates, InputError, InputInjectorBackend,
    MockInputInjector,
};
