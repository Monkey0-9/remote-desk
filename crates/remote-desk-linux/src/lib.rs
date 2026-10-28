pub mod capture;
pub mod input;

pub use capture::{CaptureError, MockCaptureBackend, RawFrame, ScreenCaptureBackend};
pub use input::{normalize_coordinates, InputError, InputInjectorBackend, MockInputInjector};
