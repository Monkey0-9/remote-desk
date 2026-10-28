use remote_desk_core::identity::DeviceIdentity;
use remote_desk_net::{
    DataChannelRole, DiscoveredPeer, MockTransportChannel, PeerRegistry, RemoteDeskPeerSession,
    SignalingEnvelope, SignalingError, SignalingPayload,
};
use remote_desk_proto::{unpack_frame, MouseMoveAbsPayload, PingPayload, RemoteDeskMessage};
use std::sync::Arc;

#[test]
fn test_signaling_envelope_sign_and_verify() {
    let identity = DeviceIdentity::generate();
    let now = 1_700_000_000_000u64;

    let payload = SignalingPayload::HandshakeInit {
        client_name: "Pixel 8 Pro".to_string(),
        ephemeral_x25519_pub: "abcdef1234567890".to_string(),
        requested_capabilities: vec!["video:h264".into(), "input:touch".into()],
    };

    let envelope = SignalingEnvelope::sign_and_create(&identity, payload, now)
        .expect("Signing envelope should succeed");

    assert_eq!(envelope.sender_device_id, identity.device_id());
    assert!(envelope.verify_signature(now + 500).is_ok());
}

#[test]
fn test_signaling_envelope_tamper_detection() {
    let identity = DeviceIdentity::generate();
    let now = 1_700_000_000_000u64;

    let payload = SignalingPayload::SdpOffer {
        sdp: "v=0\r\no=- 12345 2 IN IP4 127.0.0.1".into(),
    };

    let mut envelope = SignalingEnvelope::sign_and_create(&identity, payload, now).unwrap();

    // Tamper with payload
    envelope.payload = SignalingPayload::SdpOffer {
        sdp: "v=0\r\no=- 99999 2 IN IP4 10.0.0.1".into(),
    };

    let result = envelope.verify_signature(now);
    assert_eq!(result, Err(SignalingError::InvalidSignature));
}

#[test]
fn test_signaling_envelope_timestamp_expiration() {
    let identity = DeviceIdentity::generate();
    let now = 1_700_000_000_000u64;

    let payload = SignalingPayload::SessionClose {
        reason: "User closed".into(),
    };

    let envelope = SignalingEnvelope::sign_and_create(&identity, payload, now).unwrap();

    // Verify 61 seconds later -> should be rejected for drift
    let result = envelope.verify_signature(now + 61_000);
    assert_eq!(result, Err(SignalingError::TimestampExpired));
}

#[tokio::test]
async fn test_dual_datachannels_transmission() {
    let (control_tx, mut control_rx) = MockTransportChannel::new_pair(DataChannelRole::Control, 10);
    let (input_tx, mut input_rx) = MockTransportChannel::new_pair(DataChannelRole::Input, 10);

    let session = RemoteDeskPeerSession::new(Arc::new(control_tx), Arc::new(input_tx));

    // Send control ping
    session
        .send_control(RemoteDeskMessage::Ping(PingPayload { timestamp_ms: 1234 }))
        .await
        .unwrap();

    // Send input pointer move
    session
        .send_input(RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload {
            monitor_id: 0,
            x: 0.75,
            y: 0.25,
        }))
        .await
        .unwrap();

    // Verify control packet received
    let control_bytes = control_rx.recv().await.expect("Control packet received");
    let (seq, decoded_control) = unpack_frame(&control_bytes).expect("Valid frame");
    assert_eq!(seq, 1);
    match decoded_control {
        RemoteDeskMessage::Ping(PingPayload { timestamp_ms }) => assert_eq!(timestamp_ms, 1234),
        _ => panic!("Expected Ping message"),
    }

    // Verify input packet received
    let input_bytes = input_rx.recv().await.expect("Input packet received");
    let (seq, decoded_input) = unpack_frame(&input_bytes).expect("Valid frame");
    assert_eq!(seq, 2);
    match decoded_input {
        RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload { x, y, .. }) => {
            assert!((x - 0.75).abs() < 1e-4);
            assert!((y - 0.25).abs() < 1e-4);
        }
        _ => panic!("Expected MouseMoveAbs message"),
    }
}

#[test]
fn test_peer_registry_ttl() {
    let registry = PeerRegistry::new(10_000); // 10s TTL
    let now = 1_000_000;

    registry.upsert_peer(DiscoveredPeer {
        device_id: "rd-1111-2222".into(),
        name: "Host Alpha".into(),
        os_type: "Linux".into(),
        address: "192.168.1.50".into(),
        port: 4443,
        pubkey_fingerprint: "abc".into(),
        last_seen_ms: now,
    });

    registry.upsert_peer(DiscoveredPeer {
        device_id: "rd-3333-4444".into(),
        name: "Host Beta (Stale)".into(),
        os_type: "Windows".into(),
        address: "192.168.1.60".into(),
        port: 4443,
        pubkey_fingerprint: "def".into(),
        last_seen_ms: now - 15_000, // 15s ago, stale!
    });

    let active = registry.list_active_peers(now);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].device_id, "rd-1111-2222");
}
