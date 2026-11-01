use remote_desk_net::coalescer::InputEventCoalescer;
use remote_desk_net::ice::IceServerConfig;
use remote_desk_net::webrtc_session::WebRtcPeerSession;
use remote_desk_proto::messages::{
    KeyState, KeyboardKeyPayload, MouseButtonCode, MouseButtonPayload, MouseMoveAbsPayload,
    RemoteDeskMessage,
};

#[tokio::test]
async fn test_webrtc_session_creation_and_sdp_offer() {
    let ice_configs = IceServerConfig::default_stun_servers();
    let mut session = WebRtcPeerSession::new(&ice_configs).await.unwrap();

    // Initialize data channels
    session.init_data_channels().await.unwrap();

    assert!(session.control_channel().is_some());
    assert!(session.input_channel().is_some());

    // Generate local SDP offer
    let sdp = session.create_offer().await.unwrap();
    assert!(sdp.contains("v=0"));
    assert!(sdp.contains("m=application"));

    session.close().await.unwrap();
}

#[test]
fn test_input_event_coalescing_and_preservation() {
    let mut coalescer = InputEventCoalescer::new();

    // Push series of rapid mouse moves on monitor 0
    for i in 0..10 {
        coalescer.push(RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload {
            monitor_id: 0,
            x: 0.1 * (i as f32),
            y: 0.1 * (i as f32),
        }));
    }

    // Push mouse button down
    coalescer.push(RemoteDeskMessage::MouseButton(MouseButtonPayload {
        button: MouseButtonCode::Left,
        state: KeyState::Pressed,
    }));

    // Push more moves
    for i in 0..5 {
        coalescer.push(RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload {
            monitor_id: 0,
            x: 0.5 + 0.05 * (i as f32),
            y: 0.5 + 0.05 * (i as f32),
        }));
    }

    // Push key press
    coalescer.push(RemoteDeskMessage::KeyboardKey(KeyboardKeyPayload {
        scancode: 30, // 'A'
        state: KeyState::Pressed,
        modifiers: 0,
    }));

    let drained = coalescer.drain();

    // Must have preserved:
    // 1. One latest mouse move before the click
    // 2. The click
    // 3. One latest mouse move before the key
    // 4. The key press
    assert_eq!(drained.len(), 4);

    // Verify ordering
    assert!(matches!(drained[0], RemoteDeskMessage::MouseMoveAbs(_)));
    assert!(matches!(drained[1], RemoteDeskMessage::MouseButton(_)));
    assert!(matches!(drained[2], RemoteDeskMessage::MouseMoveAbs(_)));
    assert!(matches!(drained[3], RemoteDeskMessage::KeyboardKey(_)));

    // Verify coalescer reported a substantial reduction
    assert!(coalescer.reduction_ratio() > 0.5);
}
