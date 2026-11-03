use remote_desk_proto::MouseButtonCode;
use remote_desk_windows::{
    normalize_win32_coordinates, DxgiCaptureBackend, SendInputInjector, WindowsCaptureError,
    WindowsInputInjectorBackend, WindowsScreenCaptureBackend,
};

#[test]
fn test_normalize_win32_coordinates() {
    assert_eq!(normalize_win32_coordinates(0.0, 0.0), (0, 0));
    assert_eq!(normalize_win32_coordinates(1.0, 1.0), (65535, 65535));
    assert_eq!(normalize_win32_coordinates(0.5, 0.5), (32768, 32768));

    // Clamp out-of-bounds coordinates
    assert_eq!(normalize_win32_coordinates(-0.5, 1.5), (0, 65535));
}

#[tokio::test]
async fn test_dxgi_capture_and_frame_delivery() {
    let mut backend = DxgiCaptureBackend::new().with_allow_mock(true);
    let monitors = backend.enumerate_monitors().await.unwrap();
    assert!(!monitors.is_empty());
    assert_eq!(monitors[0].id, 0);

    // Start capture on primary display
    backend.start_capture(monitors[0].id).await.unwrap();

    let frame = backend.next_frame().await.unwrap();
    assert!(frame.width > 0);
    assert!(frame.height > 0);
    assert_eq!(frame.stride, frame.width * 4);
    assert_eq!(frame.data.len(), (frame.stride * frame.height) as usize);

    backend.stop_capture().await.unwrap();
}

#[tokio::test]
async fn test_dxgi_access_denied_handling() {
    let mut backend = DxgiCaptureBackend::new().with_access_denied();
    let err = backend.start_capture(0).await.unwrap_err();
    assert_eq!(err, WindowsCaptureError::AccessDenied);
}

#[tokio::test]
async fn test_windows_send_input_and_fail_safe_release() {
    let mut injector = SendInputInjector::new();

    // Inject absolute pointer move
    injector.move_pointer(0.25, 0.75).await.unwrap();
    assert_eq!(injector.current_x, 16384);
    assert_eq!(injector.current_y, 49151);

    // Inject button press
    injector
        .mouse_button(MouseButtonCode::Left, true)
        .await
        .unwrap();
    assert_eq!(injector.held_buttons.len(), 1);

    // Inject key press
    injector.keyboard_key(0x1E, true).await.unwrap(); // 'A' key
    assert_eq!(injector.held_keys.len(), 1);

    // Session drops -> Trigger fail safe release
    injector.release_all().await.unwrap();
    assert_eq!(injector.held_buttons.len(), 0);
    assert_eq!(injector.held_keys.len(), 0);
}

#[tokio::test]
async fn test_dxgi_fail_closed_without_mock() {
    let mut backend = DxgiCaptureBackend::new().with_allow_mock(false);
    backend.start_capture(0).await.unwrap();

    // When mock is disallowed and native display capture is unavailable, next_frame must return error
    #[cfg(not(target_os = "windows"))]
    {
        let res = backend.next_frame().await;
        assert!(matches!(res, Err(WindowsCaptureError::ApiError(_))));
    }
}
