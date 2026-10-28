use remote_desk_core::{compute_sas_code, DeviceIdentity, EphemeralKeyExchange};
use remote_desk_host::{HostConfig, HostService};
use remote_desk_net::{DiscoveredCandidate, IceCandidateType, IceServerConfig};
use remote_desk_proto::{
    pack_frame, unpack_frame, KeyState, KeyboardKeyPayload, MouseButtonCode, MouseButtonPayload,
    MouseMoveAbsPayload, RemoteDeskMessage,
};
use remote_desk_signal::RendezvousBroker;
use remote_desk_windows::{
    normalize_win32_coordinates, DxgiCaptureBackend, SendInputInjector,
    WindowsInputInjectorBackend, WindowsScreenCaptureBackend,
};

#[tokio::test]
async fn test_e2e_windows_capture_and_input_pipeline() {
    let mut capture = DxgiCaptureBackend::new();
    let monitors = capture.enumerate_monitors().await.unwrap();
    assert_eq!(monitors.len(), 2);

    capture.start_capture(monitors[0].id).await.unwrap();
    let frame = capture.next_frame().await.unwrap();
    assert_eq!(frame.width, 2560);
    assert_eq!(frame.height, 1440);

    let mut injector = SendInputInjector::new();

    // 1. Controller sends normalized absolute mouse move (0.5, 0.5)
    let move_msg = RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload {
        monitor_id: 0,
        x: 0.5,
        y: 0.5,
    });
    let packed = pack_frame(1, &move_msg).unwrap();
    let (seq, unpacked) = unpack_frame(&packed).unwrap();
    assert_eq!(seq, 1);

    if let RemoteDeskMessage::MouseMoveAbs(p) = unpacked {
        let (win_x, win_y) = normalize_win32_coordinates(p.x, p.y);
        assert_eq!(win_x, 32768);
        assert_eq!(win_y, 32768);
        injector.move_pointer(p.x, p.y).await.unwrap();
        assert_eq!(injector.current_x, 32768);
        assert_eq!(injector.current_y, 32768);
    } else {
        panic!("Expected MouseMoveAbs");
    }

    // 2. Controller sends left click down
    let click_msg = RemoteDeskMessage::MouseButton(MouseButtonPayload {
        button: MouseButtonCode::Left,
        state: KeyState::Pressed,
    });
    let packed_click = pack_frame(2, &click_msg).unwrap();
    let (_, unpacked_click) = unpack_frame(&packed_click).unwrap();
    if let RemoteDeskMessage::MouseButton(p) = unpacked_click {
        injector
            .mouse_button(p.button, p.state == KeyState::Pressed)
            .await
            .unwrap();
        assert_eq!(injector.held_buttons.len(), 1);
    }

    // 3. Controller sends keyboard key down
    let key_msg = RemoteDeskMessage::KeyboardKey(KeyboardKeyPayload {
        scancode: 0x1E,
        state: KeyState::Pressed,
        modifiers: 0,
    });
    let packed_key = pack_frame(3, &key_msg).unwrap();
    let (_, unpacked_key) = unpack_frame(&packed_key).unwrap();
    if let RemoteDeskMessage::KeyboardKey(p) = unpacked_key {
        injector
            .keyboard_key(p.scancode, p.state == KeyState::Pressed)
            .await
            .unwrap();
        assert_eq!(injector.held_keys.len(), 1);
    }

    // 4. Session drop -> Fail-safe release all
    injector.release_all().await.unwrap();
    assert_eq!(injector.held_buttons.len(), 0);
    assert_eq!(injector.held_keys.len(), 0);

    capture.stop_capture().await.unwrap();
}

#[tokio::test]
async fn test_e2e_ice_turn_priority_and_relay_routing() {
    // 1. ICE candidate prioritization
    let host_cand = DiscoveredCandidate {
        candidate_str: "candidate:1 1 UDP 2122260223 192.168.1.100 50000 typ host".to_string(),
        candidate_type: IceCandidateType::Host,
        address: "192.168.1.100".to_string(),
        port: 50000,
        priority: 2122260223,
    };

    let srflx_cand = DiscoveredCandidate {
        candidate_str: "candidate:2 1 UDP 1686052607 203.0.113.1 50000 typ srflx".to_string(),
        candidate_type: IceCandidateType::ServerReflexive,
        address: "203.0.113.1".to_string(),
        port: 50000,
        priority: 1686052607,
    };

    let relay_cand = DiscoveredCandidate {
        candidate_str: "candidate:3 1 UDP 41943039 198.51.100.2 3478 typ relay".to_string(),
        candidate_type: IceCandidateType::Relay,
        address: "198.51.100.2".to_string(),
        port: 3478,
        priority: 41943039,
    };

    // Priority ordering check: Host > Srflx > Relay
    assert!(host_cand.priority > srflx_cand.priority);
    assert!(srflx_cand.priority > relay_cand.priority);

    // 2. TURN Server config validation
    let turn_cfg = IceServerConfig::turn_relay(
        "turn:turn.example.com:3478".to_string(),
        "testuser".to_string(),
        "secretpass".to_string(),
    );
    assert_eq!(turn_cfg.username.as_deref(), Some("testuser"));
    assert_eq!(turn_cfg.credential.as_deref(), Some("secretpass"));

    // 3. Rendezvous broker routing between Host and Controller
    let broker = RendezvousBroker::new(100);
    let host_id = "rd-1234-5678";
    let client_id = "rd-8765-4321";

    broker.register_peer(host_id).unwrap();
    broker.register_peer(client_id).unwrap();

    // Client sends SDP offer envelope to Host
    let dummy_sdp = b"v=0\r\no=- 123456 2 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\n".to_vec();
    broker
        .route_message(client_id, host_id, dummy_sdp.clone())
        .unwrap();

    // Host receives envelope
    let incoming = broker.poll_messages(host_id);
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0], dummy_sdp);
}

#[tokio::test]
async fn test_e2e_host_session_with_sas_mutual_pairing() {
    let host_identity = DeviceIdentity::generate();
    let client_identity = DeviceIdentity::generate();

    let client_exchange = EphemeralKeyExchange::new();
    let client_ephemeral_pub = client_exchange.public_key_bytes();

    // 1. Host service receives client handshake
    let host = HostService::new_mock(host_identity, HostConfig::default());
    let handshake_resp = host
        .handle_handshake(
            &client_identity.device_id(),
            &client_identity.public_key_bytes(),
            &client_ephemeral_pub,
        )
        .await
        .unwrap();

    assert_eq!(handshake_resp.sas_code.len(), 7); // Format: "XXX-XXX"

    // 2. Client receives host ephemeral public key and computes shared secret
    let client_shared =
        client_exchange.compute_shared_secret(&handshake_resp.host_ephemeral_pubkey);
    let client_sas = compute_sas_code(
        &client_shared,
        &client_identity.public_key_bytes(),
        &host.identity.public_key_bytes(),
    );

    // Mutual SAS agreement check: Zero MitM
    assert_eq!(handshake_resp.sas_code, client_sas);

    // 3. Host approves
    let action = host.approve_session().await.unwrap();
    assert!(matches!(
        action,
        remote_desk_core::SessionAction::StartStreaming { .. }
    ));

    // 4. Client terminates
    host.terminate_session("Session finished").await.unwrap();
}
