use remote_desk_linux::{
    normalize_coordinates, CaptureError, InputInjectorBackend, MockCaptureBackend,
    MockInputInjector, ScreenCaptureBackend,
};
use remote_desk_proto::MouseButtonCode;

#[tokio::test]
async fn test_mock_screen_capture_and_frame_delivery() {
    let mut capture = MockCaptureBackend::new();
    let monitors = capture.enumerate_monitors().await.unwrap();
    assert_eq!(monitors.len(), 2);
    assert_eq!(monitors[0].width, 1920);
    assert_eq!(monitors[0].height, 1080);

    capture.start_capture(monitors[0].id).await.unwrap();

    for i in 1..=5 {
        let frame = capture.next_frame().await.unwrap();
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);
        assert_eq!(frame.stride, 1920 * 4);
        assert_eq!(frame.data.len(), (1920 * 1080 * 4) as usize);
        assert_eq!(frame.timestamp_us, i * 16_666);
    }

    capture.stop_capture().await.unwrap();
}

#[tokio::test]
async fn test_permission_denied_handled_gracefully() {
    let mut capture = MockCaptureBackend::new().with_permission_denial();
    let err = capture.start_capture(0).await.unwrap_err();
    assert_eq!(err, CaptureError::PermissionDenied);
}

#[test]
fn test_coordinate_normalization() {
    // Top-left
    let (x0, y0) = normalize_coordinates(0.0, 0.0, 1920, 1080).unwrap();
    assert_eq!((x0, y0), (0, 0));

    // Bottom-right
    let (x1, y1) = normalize_coordinates(1.0, 1.0, 1920, 1080).unwrap();
    assert_eq!((x1, y1), (1919, 1079));

    // Center
    let (xc, yc) = normalize_coordinates(0.5, 0.5, 1920, 1080).unwrap();
    assert_eq!((xc, yc), (960, 540));

    // Clamping out-of-bounds inputs safely
    let (x_neg, y_neg) = normalize_coordinates(-0.5, -10.0, 1920, 1080).unwrap();
    assert_eq!((x_neg, y_neg), (0, 0));

    let (x_ovr, y_ovr) = normalize_coordinates(1.5, 2.0, 1920, 1080).unwrap();
    assert_eq!((x_ovr, y_ovr), (1919, 1079));
}

#[tokio::test]
async fn test_stuck_key_revocation_on_disconnect() {
    let mut injector = MockInputInjector::new(1920, 1080);

    // Simulate user holding multiple modifier keys and mouse buttons
    injector.keyboard_key(42, true).await.unwrap(); // Shift down
    injector.keyboard_key(29, true).await.unwrap(); // Ctrl down
    injector.keyboard_key(56, true).await.unwrap(); // Alt down

    injector
        .mouse_button(MouseButtonCode::Left, true)
        .await
        .unwrap();
    injector
        .mouse_button(MouseButtonCode::Right, true)
        .await
        .unwrap();

    assert_eq!(injector.held_keys_count(), 3);
    assert_eq!(injector.held_buttons_count(), 2);

    // Connection terminates -> Immediate revocation trigger
    injector.release_all().await.unwrap();

    // Verify all keys and buttons are guaranteed released
    assert_eq!(injector.held_keys_count(), 0);
    assert_eq!(injector.held_buttons_count(), 0);
}
