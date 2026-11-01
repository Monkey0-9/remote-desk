use byteorder::{BigEndian, ByteOrder, LittleEndian};
use remote_desk_proto::codec::{pack_frame, unpack_frame, HEADER_SIZE};
use remote_desk_proto::messages::*;

#[test]
fn test_golden_vectors_mouse_move_abs() {
    let msg = RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload {
        monitor_id: 0,
        x: 0.5,
        y: 0.25,
    });
    let frame = pack_frame(1, &msg).expect("pack");

    // Header assertions:
    assert_eq!(&frame[0..2], b"RD"); // Magic
    assert_eq!(frame[2], 1); // Version
    assert_eq!(frame[3], 0x01); // MessageType::MouseMoveAbs
    assert_eq!(BigEndian::read_u64(&frame[4..12]), 1); // Sequence = 1
    let payload_len = BigEndian::read_u32(&frame[12..16]) as usize;
    assert_eq!(payload_len, 16); // 4-byte variant tag + 4 monitor + 4 x + 4 y
    assert_eq!(frame.len(), HEADER_SIZE + 16);

    // Payload assertions (bincode wire format):
    assert_eq!(LittleEndian::read_u32(&frame[16..20]), 0); // RemoteDeskMessage variant tag 0
    assert_eq!(LittleEndian::read_u32(&frame[20..24]), 0); // monitor_id
    assert_eq!(LittleEndian::read_f32(&frame[24..28]), 0.5); // x
    assert_eq!(LittleEndian::read_f32(&frame[28..32]), 0.25); // y

    // Roundtrip verification
    let (seq, unpacked) = unpack_frame(&frame).expect("unpack");
    assert_eq!(seq, 1);
    assert_eq!(unpacked, msg);
}

#[test]
fn test_golden_vectors_mouse_button() {
    let msg = RemoteDeskMessage::MouseButton(MouseButtonPayload {
        button: MouseButtonCode::Left,
        state: KeyState::Pressed,
    });
    let frame = pack_frame(2, &msg).expect("pack");

    assert_eq!(&frame[0..2], b"RD");
    assert_eq!(frame[2], 1);
    assert_eq!(frame[3], 0x03); // MessageType::MouseButton
    assert_eq!(BigEndian::read_u64(&frame[4..12]), 2);
    let payload_len = BigEndian::read_u32(&frame[12..16]) as usize;
    assert_eq!(payload_len, 12); // variant (4) + button (4) + state (4)

    assert_eq!(LittleEndian::read_u32(&frame[16..20]), 2); // Variant tag 2
    assert_eq!(LittleEndian::read_u32(&frame[20..24]), 0); // Left = 0
    assert_eq!(LittleEndian::read_u32(&frame[24..28]), 1); // Pressed = 1

    let (seq, unpacked) = unpack_frame(&frame).expect("unpack");
    assert_eq!(seq, 2);
    assert_eq!(unpacked, msg);
}

#[test]
fn test_golden_vectors_mouse_wheel() {
    let msg = RemoteDeskMessage::MouseWheel(MouseWheelPayload {
        delta_x: 10,
        delta_y: -120,
    });
    let frame = pack_frame(3, &msg).expect("pack");

    assert_eq!(&frame[0..2], b"RD");
    assert_eq!(frame[2], 1);
    assert_eq!(frame[3], 0x04); // MessageType::MouseWheel
    assert_eq!(BigEndian::read_u64(&frame[4..12]), 3);
    let payload_len = BigEndian::read_u32(&frame[12..16]) as usize;
    assert_eq!(payload_len, 8); // variant (4) + dx (2) + dy (2)

    assert_eq!(LittleEndian::read_u32(&frame[16..20]), 3); // Variant tag 3
    assert_eq!(LittleEndian::read_i16(&frame[20..22]), 10);
    assert_eq!(LittleEndian::read_i16(&frame[22..24]), -120);

    let (seq, unpacked) = unpack_frame(&frame).expect("unpack");
    assert_eq!(seq, 3);
    assert_eq!(unpacked, msg);
}

#[test]
fn test_golden_vectors_keyboard_key() {
    let msg = RemoteDeskMessage::KeyboardKey(KeyboardKeyPayload {
        scancode: 30, // KEY_A
        state: KeyState::Pressed,
        modifiers: 1, // Shift
    });
    let frame = pack_frame(4, &msg).expect("pack");

    assert_eq!(&frame[0..2], b"RD");
    assert_eq!(frame[2], 1);
    assert_eq!(frame[3], 0x05); // MessageType::KeyboardKey
    assert_eq!(BigEndian::read_u64(&frame[4..12]), 4);
    let payload_len = BigEndian::read_u32(&frame[12..16]) as usize;
    assert_eq!(payload_len, 16); // variant (4) + scancode (4) + state (4) + modifiers (4)

    assert_eq!(LittleEndian::read_u32(&frame[16..20]), 4); // Variant tag 4
    assert_eq!(LittleEndian::read_u32(&frame[20..24]), 30);
    assert_eq!(LittleEndian::read_u32(&frame[24..28]), 1); // KeyState::Pressed = 1
    assert_eq!(LittleEndian::read_u32(&frame[28..32]), 1); // modifiers bitmask

    let (seq, unpacked) = unpack_frame(&frame).expect("unpack");
    assert_eq!(seq, 4);
    assert_eq!(unpacked, msg);
}

#[test]
fn test_golden_vectors_ping_pong() {
    let ping = RemoteDeskMessage::Ping(PingPayload {
        timestamp_ms: 1700000000000,
    });
    let frame = pack_frame(5, &ping).expect("pack");

    assert_eq!(frame[3], 0x20); // MessageType::Ping
    assert_eq!(LittleEndian::read_u32(&frame[16..20]), 10); // Variant tag 10
    assert_eq!(LittleEndian::read_u64(&frame[20..28]), 1700000000000);

    let (seq, unpacked) = unpack_frame(&frame).expect("unpack");
    assert_eq!(seq, 5);
    assert_eq!(unpacked, ping);
}

#[test]
fn test_golden_vectors_disconnect() {
    let msg = RemoteDeskMessage::Disconnect(DisconnectPayload {
        reason_code: 0,
        reason_text: "User exit".to_string(),
    });
    let frame = pack_frame(6, &msg).expect("pack");

    assert_eq!(frame[3], 0xFF); // MessageType::Disconnect
    assert_eq!(LittleEndian::read_u32(&frame[16..20]), 12); // Variant tag 12
    let (seq, unpacked) = unpack_frame(&frame).expect("unpack");
    assert_eq!(seq, 6);
    assert_eq!(unpacked, msg);
}
