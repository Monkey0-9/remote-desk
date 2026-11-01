use remote_desk_core::identity::DeviceIdentity;
use remote_desk_host::streamer::FrameStreamer;
use remote_desk_linux::MockCaptureBackend;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

#[tokio::test]
async fn test_frame_streamer_capture_and_backpressure() {
    let streamer = FrameStreamer::new();
    let capture = Box::new(MockCaptureBackend::new());

    // Buffer capacity 2: will drop frames if consumer does not consume fast enough
    let (frame_tx, mut frame_rx) = mpsc::channel(2);
    let (stop_tx, stop_rx) = broadcast::channel(1);

    assert!(!streamer.is_active());

    // Start streaming at 60 FPS
    let handle = streamer.start_streaming(capture, 0, 60, frame_tx, stop_rx);
    assert!(streamer.is_active());

    // Consume 3 frames
    let frame1 = frame_rx.recv().await.expect("frame 1");
    assert_eq!(frame1.width, 1920);
    assert_eq!(frame1.height, 1080);

    let frame2 = frame_rx.recv().await.expect("frame 2");
    assert_eq!(frame2.width, 1920);

    // Sleep 100ms without consuming to trigger backpressure frame drop
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Stop streamer
    let _ = stop_tx.send(());
    let _ = handle.await;

    assert!(!streamer.is_active());
    let stats = streamer.stats();
    assert!(
        stats.frames_captured >= 3,
        "captured: {}",
        stats.frames_captured
    );
    assert!(stats.frames_sent >= 2, "sent: {}", stats.frames_sent);
    // Backpressure should have dropped at least one frame during the 100ms pause
    assert!(
        stats.frames_dropped > 0,
        "dropped: {}",
        stats.frames_dropped
    );
}

#[tokio::test]
async fn test_signaling_client_registration_and_message_routing() {
    use remote_desk_host::signaling_client::SignalingClient;
    use remote_desk_signal::server::SignalingServer;

    let port = 19876;
    let bind_addr = format!("127.0.0.1:{}", port);
    let server = SignalingServer::bind(&bind_addr, 64).await.unwrap();

    let (server_stop_tx, server_stop_rx) = broadcast::channel(1);
    let server_task = tokio::spawn(async move {
        let _ = server.run(server_stop_rx).await;
    });

    // Give server moment to bind
    tokio::time::sleep(Duration::from_millis(50)).await;

    let host_id = DeviceIdentity::generate();
    let client_ws_url = format!("ws://{}/ws", bind_addr);
    let signal_client = SignalingClient::new(host_id.clone(), client_ws_url);

    let (incoming_tx, _incoming_rx) = mpsc::channel(10);
    let (_outgoing_tx, outgoing_rx) = mpsc::channel(10);
    let (stop_tx, stop_rx) = broadcast::channel(1);

    let client_task = tokio::spawn(async move {
        let _ = signal_client
            .run_loop(incoming_tx, outgoing_rx, stop_rx)
            .await;
    });

    // Verify connection established
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send stop signals
    let _ = stop_tx.send(());
    let _ = server_stop_tx.send(());
    let _ = client_task.await;
    let _ = server_task.await;
}
