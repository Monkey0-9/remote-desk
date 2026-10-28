use remote_desk_proto::{
    pack_frame, unpack_frame, DisconnectPayload, DisplayTopologyPayload, KeyState,
    KeyboardKeyPayload, MessageType, MonitorDescriptor, MouseButtonCode, MouseButtonPayload,
    MouseMoveAbsPayload, MouseMoveRelPayload, MouseWheelPayload, PingPayload, PongPayload,
    ProtoError, RemoteDeskMessage, HEADER_SIZE,
};

#[test]
fn test_roundtrip_mouse_move_abs() {
    let msg = RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload {
        monitor_id: 1,
        x: 0.75,
        y: 0.25,
    });

    let packed = pack_frame(42, &msg).expect("packing should succeed");
    assert_eq!(&packed[0..2], &[0x52, 0x44]); // Magic RD
    assert_eq!(packed[2], 1); // Version 1
    assert_eq!(packed[3], MessageType::MouseMoveAbs as u8);

    let (seq, unpacked) = unpack_frame(&packed).expect("unpacking should succeed");
    assert_eq!(seq, 42);
    assert_eq!(unpacked, msg);
}

#[test]
fn test_roundtrip_mouse_move_rel() {
    let msg = RemoteDeskMessage::MouseMoveRel(MouseMoveRelPayload { dx: -12, dy: 45 });

    let packed = pack_frame(1001, &msg).unwrap();
    let (seq, unpacked) = unpack_frame(&packed).unwrap();
    assert_eq!(seq, 1001);
    assert_eq!(unpacked, msg);
}

#[test]
fn test_roundtrip_mouse_button() {
    let msg = RemoteDeskMessage::MouseButton(MouseButtonPayload {
        button: MouseButtonCode::Right,
        state: KeyState::Pressed,
    });

    let packed = pack_frame(7, &msg).unwrap();
    let (seq, unpacked) = unpack_frame(&packed).unwrap();
    assert_eq!(seq, 7);
    assert_eq!(unpacked, msg);
}

#[test]
fn test_roundtrip_mouse_wheel() {
    let msg = RemoteDeskMessage::MouseWheel(MouseWheelPayload {
        delta_x: 0,
        delta_y: -120,
    });

    let packed = pack_frame(88, &msg).unwrap();
    let (seq, unpacked) = unpack_frame(&packed).unwrap();
    assert_eq!(seq, 88);
    assert_eq!(unpacked, msg);
}

#[test]
fn test_roundtrip_keyboard_key() {
    let msg = RemoteDeskMessage::KeyboardKey(KeyboardKeyPayload {
        scancode: 42,
        state: KeyState::Pressed,
        modifiers: 0x01, // Shift
    });

    let packed = pack_frame(500, &msg).unwrap();
    let (seq, unpacked) = unpack_frame(&packed).unwrap();
    assert_eq!(seq, 500);
    assert_eq!(unpacked, msg);
}

#[test]
fn test_roundtrip_display_topology() {
    let msg = RemoteDeskMessage::DisplayTopology(DisplayTopologyPayload {
        monitors: vec![
            MonitorDescriptor {
                id: 0,
                name: "eDP-1".to_string(),
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                refresh_rate: 60,
                is_primary: true,
            },
            MonitorDescriptor {
                id: 1,
                name: "HDMI-A-1".to_string(),
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
                refresh_rate: 144,
                is_primary: false,
            },
        ],
        active_monitor_id: 0,
    });

    let packed = pack_frame(1, &msg).unwrap();
    let (seq, unpacked) = unpack_frame(&packed).unwrap();
    assert_eq!(seq, 1);
    assert_eq!(unpacked, msg);
}

#[test]
fn test_roundtrip_ping_pong_disconnect() {
    let ping = RemoteDeskMessage::Ping(PingPayload {
        timestamp_ms: 12345678,
    });
    let (s1, u1) = unpack_frame(&pack_frame(10, &ping).unwrap()).unwrap();
    assert_eq!(s1, 10);
    assert_eq!(u1, ping);

    let pong = RemoteDeskMessage::Pong(PongPayload {
        client_timestamp_ms: 12345678,
        host_timestamp_ms: 12345700,
    });
    let (s2, u2) = unpack_frame(&pack_frame(11, &pong).unwrap()).unwrap();
    assert_eq!(s2, 11);
    assert_eq!(u2, pong);

    let disc = RemoteDeskMessage::Disconnect(DisconnectPayload {
        reason_code: 1,
        reason_text: "User ended session".to_string(),
    });
    let (s3, u3) = unpack_frame(&pack_frame(12, &disc).unwrap()).unwrap();
    assert_eq!(s3, 12);
    assert_eq!(u3, disc);
}

#[test]
fn test_invalid_magic_rejected() {
    let mut packed =
        pack_frame(1, &RemoteDeskMessage::Ping(PingPayload { timestamp_ms: 0 })).unwrap();
    packed[0] = 0xAA; // Corrupt magic

    let err = unpack_frame(&packed).unwrap_err();
    assert_eq!(
        err,
        ProtoError::InvalidMagic {
            expected: [0x52, 0x44],
            actual: [0xAA, 0x44]
        }
    );
}

#[test]
fn test_unsupported_version_rejected() {
    let mut packed =
        pack_frame(1, &RemoteDeskMessage::Ping(PingPayload { timestamp_ms: 0 })).unwrap();
    packed[2] = 99; // Corrupt version

    let err = unpack_frame(&packed).unwrap_err();
    assert_eq!(
        err,
        ProtoError::UnsupportedVersion {
            expected: 1,
            actual: 99
        }
    );
}

#[test]
fn test_truncated_buffer_rejected() {
    let packed = pack_frame(1, &RemoteDeskMessage::Ping(PingPayload { timestamp_ms: 0 })).unwrap();
    let truncated = &packed[..HEADER_SIZE - 2];

    let err = unpack_frame(truncated).unwrap_err();
    assert!(matches!(err, ProtoError::BufferTooShort { .. }));
}

#[test]
fn test_payload_length_mismatch_rejected() {
    let packed = pack_frame(1, &RemoteDeskMessage::Ping(PingPayload { timestamp_ms: 0 })).unwrap();
    // Drop last 2 bytes of payload
    let corrupted = &packed[..packed.len() - 2];

    let err = unpack_frame(corrupted).unwrap_err();
    assert!(matches!(err, ProtoError::PayloadLengthMismatch { .. }));
}

#[test]
fn test_fuzz_arbitrary_garbage_bytes_never_panics() {
    // Generate pseudo-random permutations of corrupted byte streams
    let mut state: u32 = 0x12345678;
    for len in 0..128 {
        let mut buf = vec![0u8; len];
        for b in buf.iter_mut() {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            *b = (state >> 24) as u8;
        }
        // Fuzz deserializer: must cleanly return Err, never panic
        let _ = unpack_frame(&buf);
    }
}
