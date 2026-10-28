pub mod capture;
pub mod input;

pub use capture::{
    DxgiCaptureBackend, WindowsCaptureError, WindowsRawFrame, WindowsScreenCaptureBackend,
};
pub use input::{
    normalize_win32_coordinates, SendInputInjector, WindowsInputError, WindowsInputInjectorBackend,
};
