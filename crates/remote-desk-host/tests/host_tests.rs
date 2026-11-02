use remote_desk_core::{DeviceIdentity, EphemeralKeyExchange};
use remote_desk_host::{HostConfig, HostService};
use remote_desk_proto::{
    pack_frame, KeyState, KeyboardKeyPayload, MouseButtonCode, MouseButtonPayload,
    MouseMoveAbsPayload, RemoteDeskMessage,
};

#[tokio::test]
async fn test_host_initialization_and_monitors() {
    let identity = DeviceIdentity::generate();
    let host = HostService::new_mock(identity, HostConfig::default());

    assert!(host.device_id().starts_with("rd-"));
    let monitors = host.enumerate_monitors().await.unwrap();
    assert!(!monitors.is_empty());
    assert_eq!(monitors[0].id, 0);
    assert!(monitors[0].is_primary);
}

#[tokio::test]
async fn test_host_handshake_and_attended_approval() {
    let host_identity = DeviceIdentity::generate();
    let client_identity = DeviceIdentity::generate();

    let host = HostService::new_mock(host_identity, HostConfig::default());

    let client_exchange = EphemeralKeyExchange::new();
    let client_ephemeral_pub = client_exchange.public_key_bytes();

    // 1. Host processes handshake
    let response = host
        .handle_handshake(
            &client_identity.device_id(),
            &client_identity.public_key_bytes(),
            &client_ephemeral_pub,
        )
        .await
        .unwrap();

    assert_eq!(response.sas_code.len(), 7); // Format: "XXX-XXX"

    // 2. User approves session on host
    let action = host.approve_session().await.unwrap();
    assert!(matches!(
        action,
        remote_desk_core::SessionAction::StartStreaming { .. }
    ));
}

#[tokio::test]
async fn test_host_unattended_mode_auto_approval() {
    let host_identity = DeviceIdentity::generate();
    let client_identity = DeviceIdentity::generate();

    let config = HostConfig {
        unattended: true,
        ..Default::default()
    };

    let host = HostService::new_mock(host_identity, config);

    let client_exchange = EphemeralKeyExchange::new();
    let client_ephemeral_pub = client_exchange.public_key_bytes();

    // Handshake should directly activate session in unattended mode
    let _sas = host
        .handle_handshake(
            &client_identity.device_id(),
            &client_identity.public_key_bytes(),
            &client_ephemeral_pub,
        )
        .await
        .unwrap();

    let sm = host.state_machine.lock().await;
    assert!(sm.is_active(), "Session must be active in unattended mode");
}

#[tokio::test]
async fn test_host_input_processing_and_failsafe_revocation() {
    let host_identity = DeviceIdentity::generate();
    let client_identity = DeviceIdentity::generate();

    let config = HostConfig {
        unattended: true,
        ..Default::default()
    };

    let host = HostService::new_mock(host_identity, config);

    // Establish active session
    let client_exchange = EphemeralKeyExchange::new();
    host.handle_handshake(
        &client_identity.device_id(),
        &client_identity.public_key_bytes(),
        &client_exchange.public_key_bytes(),
    )
    .await
    .unwrap();

    // 1. Send MouseMoveAbs (0.5, 0.5)
    let move_msg = RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload {
        monitor_id: 0,
        x: 0.5,
        y: 0.5,
    });
    let packed_move = pack_frame(1, &move_msg).unwrap();
    host.process_incoming_input_frame(&packed_move)
        .await
        .unwrap();

    // 2. Send MouseButton down
    let click_msg = RemoteDeskMessage::MouseButton(MouseButtonPayload {
        button: MouseButtonCode::Left,
        state: KeyState::Pressed,
    });
    let packed_click = pack_frame(2, &click_msg).unwrap();
    host.process_incoming_input_frame(&packed_click)
        .await
        .unwrap();

    // 3. Send KeyboardKey down (Shift key scancode 0x2A)
    let key_msg = RemoteDeskMessage::KeyboardKey(KeyboardKeyPayload {
        scancode: 0x2A,
        state: KeyState::Pressed,
        modifiers: 0,
    });
    let packed_key = pack_frame(3, &key_msg).unwrap();
    host.process_incoming_input_frame(&packed_key)
        .await
        .unwrap();

    // 4. Abrupt disconnect -> Trigger fail-safe release
    host.terminate_session("Client network drop").await.unwrap();

    let sm = host.state_machine.lock().await;
    assert!(!sm.is_active());
}

#[tokio::test]
async fn test_host_rejects_input_when_session_inactive() {
    let host_identity = DeviceIdentity::generate();
    let host = HostService::new_mock(host_identity, HostConfig::default());

    let move_msg = RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload {
        monitor_id: 0,
        x: 0.5,
        y: 0.5,
    });
    let packed_move = pack_frame(1, &move_msg).unwrap();

    // Inactive host must reject input frame
    let res = host.process_incoming_input_frame(&packed_move).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_host_windows_backend_instantiation_and_capture_pipeline() {
    let identity = DeviceIdentity::generate();
    let host = HostService::new_windows(identity, HostConfig::default());

    assert!(host.device_id().starts_with("rd-"));
    let monitors = host.enumerate_monitors().await.unwrap();
    assert_eq!(monitors.len(), 2);
    assert_eq!(monitors[0].id, 0);

    // Verify Windows capture backend starts and delivers frame through adapter
    {
        let mut capture = host.capture.lock().await;
        capture.start_capture(0).await.unwrap();
        let frame = capture.next_frame().await.unwrap();
        assert_eq!(frame.width, 2560);
        assert_eq!(frame.height, 1440);
        capture.stop_capture().await.unwrap();
    }
}

#[tokio::test]
async fn test_host_auto_backend_selection() {
    let identity = DeviceIdentity::generate();
    let host_res = HostService::new_auto(identity.clone(), HostConfig::default());
    let host = match host_res {
        Ok(h) => h,
        Err(_) => HostService::new_mock(identity, HostConfig::default()),
    };

    assert!(host.device_id().starts_with("rd-"));
    let monitors = host.enumerate_monitors().await.unwrap();
    assert!(!monitors.is_empty());
}

#[tokio::test]
async fn test_host_http_web_portal_and_status_endpoints() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let port = 19998;
    let bind_addr = format!("127.0.0.1:{}", port);

    let identity = DeviceIdentity::generate();
    let expected_device_id = identity.device_id();
    let config = HostConfig {
        bind_addr: bind_addr.clone(),
        device_name: "Portal-Test-Host".to_string(),
        unattended: false,
        signal_server_url: None,
        turn_servers: vec![],
    };

    let host = HostService::new_mock(identity, config);
    let host_arc = std::sync::Arc::new(host);
    let host_daemon = host_arc.clone();

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let daemon_task = tokio::spawn(async move {
        let _ = host_daemon.run_daemon(shutdown_rx).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 1. Test GET / returns 200 OK with HTML Portal containing device_id
    {
        let mut client = tokio::net::TcpStream::connect(&bind_addr).await.unwrap();
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut resp_buf = Vec::new();
        client.read_to_end(&mut resp_buf).await.unwrap();
        let resp = String::from_utf8_lossy(&resp_buf);
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        assert!(resp.contains("Content-Type: text/html"));
        assert!(resp.contains(&expected_device_id));
        assert!(resp.contains("Remote-Desk Host"));
    }

    // 2. Test GET /signal returns 200 OK with HTML Portal
    {
        let mut client = tokio::net::TcpStream::connect(&bind_addr).await.unwrap();
        client
            .write_all(b"GET /signal HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut resp_buf = Vec::new();
        client.read_to_end(&mut resp_buf).await.unwrap();
        let resp = String::from_utf8_lossy(&resp_buf);
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        assert!(resp.contains("Content-Type: text/html"));
    }

    // 3. Test GET /health returns 200 OK JSON
    {
        let mut client = tokio::net::TcpStream::connect(&bind_addr).await.unwrap();
        client
            .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut resp_buf = Vec::new();
        client.read_to_end(&mut resp_buf).await.unwrap();
        let resp = String::from_utf8_lossy(&resp_buf);
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        assert!(resp.contains("application/json"));
        assert!(resp.contains("\"status\":\"ok\""));
    }

    // 4. Test GET /api/info returns 200 OK JSON
    {
        let mut client = tokio::net::TcpStream::connect(&bind_addr).await.unwrap();
        client
            .write_all(b"GET /api/info HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut resp_buf = Vec::new();
        client.read_to_end(&mut resp_buf).await.unwrap();
        let resp = String::from_utf8_lossy(&resp_buf);
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        assert!(resp.contains("application/json"));
        assert!(resp.contains(&expected_device_id));
        assert!(resp.contains("Portal-Test-Host"));
    }

    // 5. Test GET /favicon.ico returns 204 No Content
    {
        let mut client = tokio::net::TcpStream::connect(&bind_addr).await.unwrap();
        client
            .write_all(b"GET /favicon.ico HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut resp_buf = Vec::new();
        client.read_to_end(&mut resp_buf).await.unwrap();
        let resp = String::from_utf8_lossy(&resp_buf);
        assert!(resp.starts_with("HTTP/1.1 204 No Content"));
    }

    // 6. Test GET /api/overview returns 200 OK JSON with interfaces and status
    {
        let mut client = tokio::net::TcpStream::connect(&bind_addr).await.unwrap();
        client
            .write_all(b"GET /api/overview HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut resp_buf = Vec::new();
        client.read_to_end(&mut resp_buf).await.unwrap();
        let resp = String::from_utf8_lossy(&resp_buf);
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        assert!(resp.contains("application/json"));
        assert!(resp.contains("\"device_id\""));
        assert!(resp.contains("\"interfaces\""));
        assert!(resp.contains("\"system_state\""));
    }

    // 7. Test GET /api/devices returns 200 OK JSON
    {
        let mut client = tokio::net::TcpStream::connect(&bind_addr).await.unwrap();
        client
            .write_all(b"GET /api/devices HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut resp_buf = Vec::new();
        client.read_to_end(&mut resp_buf).await.unwrap();
        let resp = String::from_utf8_lossy(&resp_buf);
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        assert!(resp.contains("application/json"));
        assert!(resp.contains("\"trusted\""));
        assert!(resp.contains("\"available\""));
    }

    // 8. Test GET /api/performance returns 200 OK JSON
    {
        let mut client = tokio::net::TcpStream::connect(&bind_addr).await.unwrap();
        client
            .write_all(b"GET /api/performance HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut resp_buf = Vec::new();
        client.read_to_end(&mut resp_buf).await.unwrap();
        let resp = String::from_utf8_lossy(&resp_buf);
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        assert!(resp.contains("application/json"));
        assert!(resp.contains("\"fps_delivered\""));
        assert!(resp.contains("\"encode_latency_ms\""));
    }

    // 9. Test GET /api/qrcode returns 200 OK image/svg+xml with valid SVG QR code
    {
        let mut client = tokio::net::TcpStream::connect(&bind_addr).await.unwrap();
        client
            .write_all(b"GET /api/qrcode HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut resp_buf = Vec::new();
        client.read_to_end(&mut resp_buf).await.unwrap();
        let resp = String::from_utf8_lossy(&resp_buf);
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        assert!(resp.contains("image/svg+xml"));
        assert!(resp.contains("<svg"));
        assert!(resp.contains("</svg>"));
    }

    let _ = shutdown_tx.send(());
    let _ = daemon_task.await;
}

#[test]
fn test_qr_code_svg_generation() {
    let svg = remote_desk_host::service::generate_qr_svg("remotedesk://192.168.1.50:4444");
    assert!(svg.contains("<svg"));
    assert!(svg.contains("</svg>"));
    assert!(svg.len() > 100);
}

#[test]
fn test_network_interfaces_discovery() {
    let ifaces = remote_desk_host::discover_network_interfaces();
    assert!(
        !ifaces.is_empty(),
        "Should discover at least loopback or primary interface"
    );
    let summary = remote_desk_host::format_reachable_addresses("4444");
    assert!(!summary.is_empty());
}
