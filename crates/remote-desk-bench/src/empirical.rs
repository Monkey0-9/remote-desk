use remote_desk_core::identity::DeviceIdentity;
use remote_desk_core::pairing::EphemeralKeyExchange;
use remote_desk_core::session::{SessionEvent, SessionStateMachine};
use remote_desk_net::coalescer::InputEventCoalescer;
use remote_desk_proto::codec::{pack_frame, unpack_frame};
use remote_desk_proto::messages::{
    KeyState, MouseButtonCode, MouseButtonPayload, MouseMoveAbsPayload, RemoteDeskMessage,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmpiricalBenchmarkMetrics {
    pub ed25519_sign_ops_per_sec: f64,
    pub ed25519_sign_latency_us: f64,
    pub ed25519_verify_ops_per_sec: f64,
    pub ed25519_verify_latency_us: f64,
    pub x25519_ecdh_ops_per_sec: f64,
    pub x25519_ecdh_latency_us: f64,
    pub frame_pack_unpack_throughput_mb_s: f64,
    pub frame_pack_latency_ns: f64,
    pub coordinate_sanitizer_ops_per_sec: f64,
    pub input_coalescing_reduction_ratio_pct: f64,
    pub session_transition_ops_per_sec: f64,
    pub session_transition_latency_ns: f64,
}

pub fn run_empirical_microbenchmarks() -> EmpiricalBenchmarkMetrics {
    // 1. Measure Ed25519 signing
    let identity = DeviceIdentity::generate();
    let payload = b"RemoteDesk-Microbenchmark-Payload-v1";
    let iterations = 500;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = identity.sign(payload);
    }
    let elapsed = start.elapsed();
    let ed25519_sign_latency_us = (elapsed.as_secs_f64() * 1_000_000.0) / (iterations as f64);
    let ed25519_sign_ops_per_sec = (iterations as f64) / elapsed.as_secs_f64();

    // 2. Measure Ed25519 verification
    let signature = identity.sign(payload);
    let vk = identity.verifying_key();
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = DeviceIdentity::verify(&vk, payload, &signature);
    }
    let elapsed = start.elapsed();
    let ed25519_verify_latency_us = (elapsed.as_secs_f64() * 1_000_000.0) / (iterations as f64);
    let ed25519_verify_ops_per_sec = (iterations as f64) / elapsed.as_secs_f64();

    // 3. Measure X25519 ECDH
    let peer_exchange = EphemeralKeyExchange::new();
    let peer_pub = peer_exchange.public_key_bytes();
    let ecdh_iters = 500;
    let start = Instant::now();
    for _ in 0..ecdh_iters {
        let ex = EphemeralKeyExchange::new();
        let _ = ex.compute_shared_secret(&peer_pub);
    }
    let elapsed = start.elapsed();
    let x25519_ecdh_latency_us = (elapsed.as_secs_f64() * 1_000_000.0) / (ecdh_iters as f64);
    let x25519_ecdh_ops_per_sec = (ecdh_iters as f64) / elapsed.as_secs_f64();

    // 4. Measure protocol pack/unpack throughput
    let sample_msg = RemoteDeskMessage::MouseButton(MouseButtonPayload {
        button: MouseButtonCode::Left,
        state: KeyState::Pressed,
    });
    let pack_iters = 2_000;
    let mut total_bytes = 0usize;
    let start = Instant::now();
    for i in 0..pack_iters {
        let bytes = pack_frame(i as u64, &sample_msg).unwrap();
        total_bytes += bytes.len();
        let _ = unpack_frame(&bytes).unwrap();
    }
    let elapsed = start.elapsed();
    let frame_pack_latency_ns = (elapsed.as_secs_f64() * 1_000_000_000.0) / (pack_iters as f64);
    let frame_pack_unpack_throughput_mb_s =
        (total_bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();

    // 5. Measure coordinate validation & sanitization
    let mut valid_count = 0usize;
    let coord_iters = 5_000;
    let start = Instant::now();
    for i in 0..coord_iters {
        let p = MouseMoveAbsPayload {
            monitor_id: 0,
            x: ((i % 100) as f32) / 100.0,
            y: (((i + 50) % 100) as f32) / 100.0,
        };
        if p.is_valid() {
            valid_count += 1;
        }
    }
    let elapsed = start.elapsed();
    assert_eq!(valid_count, coord_iters);
    let coordinate_sanitizer_ops_per_sec = (coord_iters as f64) / elapsed.as_secs_f64();

    // 6. Measure pointer event coalescer reduction
    let mut coalescer = InputEventCoalescer::new();
    for i in 0..100 {
        coalescer.push(RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload {
            monitor_id: 0,
            x: (i as f32) / 100.0,
            y: (i as f32) / 100.0,
        }));
    }
    coalescer.push(RemoteDeskMessage::MouseButton(MouseButtonPayload {
        button: MouseButtonCode::Left,
        state: KeyState::Pressed,
    }));
    let ratio = coalescer.reduction_ratio() * 100.0;

    // 7. Measure SessionStateMachine transition latency
    let sm_iters = 5_000;
    let start = Instant::now();
    for _ in 0..sm_iters {
        let mut sm = SessionStateMachine::new();
        let _ = sm.transition(SessionEvent::InitiateHandshake {
            peer_id: "test-peer".to_string(),
        });
        let _ = sm.transition(SessionEvent::SasGenerated {
            sas_code: "123-456".to_string(),
        });
        let _ = sm.transition(SessionEvent::ApprovalGranted);
        let _ = sm.transition(SessionEvent::Pause);
        let _ = sm.transition(SessionEvent::Resume);
        let _ = sm.transition(SessionEvent::Disconnect("done".to_string()));
    }
    let elapsed = start.elapsed();
    let total_transitions = (sm_iters * 6) as f64;
    let session_transition_latency_ns =
        (elapsed.as_secs_f64() * 1_000_000_000.0) / total_transitions;
    let session_transition_ops_per_sec = total_transitions / elapsed.as_secs_f64();

    EmpiricalBenchmarkMetrics {
        ed25519_sign_ops_per_sec,
        ed25519_sign_latency_us,
        ed25519_verify_ops_per_sec,
        ed25519_verify_latency_us,
        x25519_ecdh_ops_per_sec,
        x25519_ecdh_latency_us,
        frame_pack_unpack_throughput_mb_s,
        frame_pack_latency_ns,
        coordinate_sanitizer_ops_per_sec,
        input_coalescing_reduction_ratio_pct: ratio,
        session_transition_ops_per_sec,
        session_transition_latency_ns,
    }
}
