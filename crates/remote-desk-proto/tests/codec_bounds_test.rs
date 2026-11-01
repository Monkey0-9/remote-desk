use remote_desk_proto::codec::{pack_frame, unpack_frame, SequenceWindowTracker};
use remote_desk_proto::messages::{
    ClipboardDataPayload, MouseMoveAbsPayload, RemoteDeskMessage, TouchGesturePayload,
};

#[test]
fn test_sequence_window_tracker_replay_rejection() {
    let mut tracker = SequenceWindowTracker::new(64);

    // Initial packets in order
    assert!(tracker.validate_and_update(1));
    assert!(tracker.validate_and_update(2));
    assert!(tracker.validate_and_update(3));

    // Replay of sequence 2 must fail!
    assert!(
        !tracker.validate_and_update(2),
        "Replay of packet 2 must be rejected"
    );

    // Out of order within window
    assert!(tracker.validate_and_update(5));
    assert!(tracker.validate_and_update(4)); // late packet within window accepted once
    assert!(
        !tracker.validate_and_update(4),
        "Duplicate late packet must be rejected"
    );

    // Packet far in the past beyond window
    assert!(tracker.validate_and_update(100));
    assert!(
        !tracker.validate_and_update(1),
        "Packet older than window must be rejected"
    );
}

#[test]
fn test_mouse_move_abs_coordinate_sanitization() {
    // Valid coordinates
    let valid = MouseMoveAbsPayload {
        monitor_id: 0,
        x: 0.5,
        y: 0.8,
    };
    assert!(valid.is_valid());

    // NaN coordinates must be rejected
    let nan_x = MouseMoveAbsPayload {
        monitor_id: 0,
        x: f32::NAN,
        y: 0.5,
    };
    assert!(!nan_x.is_valid());

    // Infinity must be rejected
    let inf_y = MouseMoveAbsPayload {
        monitor_id: 0,
        x: 0.5,
        y: f32::INFINITY,
    };
    assert!(!inf_y.is_valid());

    // Out of range coordinates
    let out_of_bounds = MouseMoveAbsPayload {
        monitor_id: 0,
        x: 1.05,
        y: -0.01,
    };
    assert!(!out_of_bounds.is_valid());
}

#[test]
fn test_touch_gesture_and_clipboard_payloads() {
    let touch = RemoteDeskMessage::TouchGesture(TouchGesturePayload {
        touch_id: 1,
        phase: 1, // Move
        x: 0.25,
        y: 0.75,
        pressure: 0.9,
    });
    let bytes = pack_frame(1, &touch).expect("pack touch");
    let (seq, unpacked) = unpack_frame(&bytes).expect("unpack touch");
    assert_eq!(seq, 1);
    assert_eq!(unpacked, touch);

    let clip = RemoteDeskMessage::ClipboardData(ClipboardDataPayload {
        mime_type: "text/plain".to_string(),
        data: b"hello remote desktop".to_vec(),
    });
    let bytes2 = pack_frame(2, &clip).expect("pack clip");
    let (seq2, unpacked2) = unpack_frame(&bytes2).expect("unpack clip");
    assert_eq!(seq2, 2);
    assert_eq!(unpacked2, clip);
}
