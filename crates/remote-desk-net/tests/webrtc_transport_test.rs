use remote_desk_net::coalescer::InputEventCoalescer;
use remote_desk_net::ice::IceServerConfig;
use remote_desk_net::webrtc_session::WebRtcPeerSession;
use remote_desk_proto::messages::{
    KeyState, KeyboardKeyPayload, MouseButtonCode, MouseButtonPayload, MouseMoveAbsPayload,
    RemoteDeskMessage,
};

#[tokio::test]
async fn test_webrtc_session_creation_and_sdp_offer() {
    use remote_desk_net::ice::{compute_ice_priority, IceCandidateType};

    let ice_configs = IceServerConfig::default_stun_servers();
    let mut session = WebRtcPeerSession::new(&ice_configs).await.unwrap();

    // Initialize data channels
    session.init_data_channels().await.unwrap();

    assert!(session.control_channel().is_some());
    assert!(session.input_channel().is_some());

    // Generate local SDP offer with RFC 8866 attributes
    let sdp = session.create_offer().await.unwrap();
    assert!(sdp.contains("v=0"));
    assert!(sdp.contains("m=application"));
    assert!(sdp.contains("a=ice-ufrag:"));
    assert!(sdp.contains("a=ice-pwd:"));
    assert!(sdp.contains("a=fingerprint:sha-256"));
    assert!(sdp.contains("a=candidate:"));

    // Verify candidate parsing from remote answer
    let remote_sdp = "v=0\r\n\
        o=- 12345 2 IN IP4 192.168.1.100\r\n\
        s=RemoteDesk-Client\r\n\
        t=0 0\r\n\
        m=application 9000 UDP/DTLS/SCTP webrtc-datachannel\r\n\
        c=IN IP4 192.168.1.100\r\n\
        a=candidate:1 1 UDP 2130706431 192.168.1.100 9000 typ host\r\n\
        a=candidate:2 1 UDP 16777215 203.0.113.1 9000 typ srflx\r\n";

    session.set_remote_answer(remote_sdp).await.unwrap();
    assert_eq!(session.remote_candidates().len(), 2);
    assert_eq!(
        session.remote_candidates()[0].candidate_type,
        IceCandidateType::Host
    );
    assert_eq!(
        session.remote_candidates()[1].candidate_type,
        IceCandidateType::ServerReflexive
    );

    // Verify priority computation
    let host_prio = compute_ice_priority(IceCandidateType::Host, 65535, 1);
    let srflx_prio = compute_ice_priority(IceCandidateType::ServerReflexive, 65535, 1);
    assert!(
        host_prio > srflx_prio,
        "Host priority ({}) must exceed srflx priority ({})",
        host_prio,
        srflx_prio
    );

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

#[test]
fn test_packet_chunking_and_reassembly() {
    use remote_desk_net::chunking::{chunk_payload, ChunkReassembler, PacketChunk};

    // Create a 5000-byte payload simulating large video frame or clipboard
    let original: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();
    let chunks = chunk_payload(42, &original);

    assert_eq!(chunks.len(), 5); // 5000 / 1200 = 4.16 -> 5 chunks

    let mut reassembler = ChunkReassembler::new();

    // Ingest first 4 chunks - should return None
    for chunk in &chunks[0..4] {
        let serialized = chunk.to_bytes();
        let parsed = PacketChunk::from_bytes(&serialized).expect("parse chunk");
        assert!(reassembler.ingest_chunk(parsed).is_none());
    }

    // Ingest final chunk - should return Some(original)
    let final_chunk = PacketChunk::from_bytes(&chunks[4].to_bytes()).expect("parse chunk");
    let reassembled = reassembler
        .ingest_chunk(final_chunk)
        .expect("complete payload");

    assert_eq!(reassembled, original);
}
