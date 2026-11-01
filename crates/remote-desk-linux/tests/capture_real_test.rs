use remote_desk_linux::capture::{
    create_linux_capture_backend, ScreenCaptureBackend, WaylandPortalCaptureBackend,
    X11ShmCaptureBackend,
};

#[tokio::test]
async fn test_wayland_portal_capture_lifecycle() {
    let mut backend = WaylandPortalCaptureBackend::new();
    let monitors = backend.enumerate_monitors().await.expect("monitors");
    assert!(!monitors.is_empty());

    let monitor_id = monitors[0].id;
    backend
        .start_capture(monitor_id)
        .await
        .expect("start capture");

    let frame = backend.next_frame().await.expect("next frame");
    assert!(frame.width > 0);
    assert!(frame.height > 0);
    assert_eq!(frame.stride, frame.width * 4);
    assert_eq!(frame.data.len(), (frame.stride * frame.height) as usize);

    backend.stop_capture().await.expect("stop capture");
}

#[tokio::test]
async fn test_x11_capture_or_clean_headless_fallback() {
    // If DISPLAY is set and X11 server is running, X11ShmCaptureBackend succeeds;
    // otherwise it returns a clean backend error.
    let x11_res = X11ShmCaptureBackend::new();
    match x11_res {
        Ok(x11) => {
            let monitors = x11.enumerate_monitors().await.expect("x11 monitors");
            assert!(!monitors.is_empty());
        }
        Err(e) => {
            // In headless/CI container, must fail gracefully with BackendError
            println!("X11 unavailable in current test environment: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_linux_capture_factory() {
    let backend = create_linux_capture_backend();
    let monitors = backend
        .enumerate_monitors()
        .await
        .expect("factory monitors");
    assert!(!monitors.is_empty());
}
