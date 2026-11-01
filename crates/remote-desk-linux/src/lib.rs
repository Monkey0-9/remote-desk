pub mod capture;
pub mod input;
pub mod portal_capture;
pub mod x11_capture;

pub use capture::{
    create_linux_capture_backend, detect_display_server, CaptureError, LinuxDisplayServer,
    MockCaptureBackend, PortalCaptureBackend, RawFrame, ScreenCaptureBackend,
    WaylandPortalCaptureBackend, X11ShmCaptureBackend,
};
pub use input::{
    create_linux_input_injector, normalize_coordinates, InputError, InputInjectorBackend,
    MockInputInjector,
};
