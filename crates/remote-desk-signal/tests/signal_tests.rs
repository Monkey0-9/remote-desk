use remote_desk_signal::{RendezvousBroker, SignalServerError};

#[test]
fn test_peer_registration_and_message_routing() {
    let broker = RendezvousBroker::new(100);

    let host_id = "rd-1234-5678";
    let controller_id = "rd-8765-4321";

    broker.register_peer(host_id).unwrap();
    broker.register_peer(controller_id).unwrap();

    let payload = b"encrypted_sdp_offer_bytes".to_vec();
    broker
        .route_message(controller_id, host_id, payload.clone())
        .expect("Routing should succeed");

    let received = broker.poll_messages(host_id);
    assert_eq!(received.len(), 1);
    assert_eq!(received[0], payload);

    // Queue is drained after polling
    assert_eq!(broker.poll_messages(host_id).len(), 0);
}

#[test]
fn test_route_to_unregistered_peer_fails() {
    let broker = RendezvousBroker::new(100);
    let sender = "rd-0000-1111";
    let target = "rd-9999-8888";

    broker.register_peer(sender).unwrap();

    let err = broker
        .route_message(sender, target, vec![1, 2, 3])
        .unwrap_err();
    assert_eq!(err, SignalServerError::PeerNotFound(target.to_string()));
}

#[test]
fn test_rate_limiter_throttles_bursts() {
    let broker = RendezvousBroker::new(100);
    let sender = "rd-spammer-01";
    let target = "rd-target-01";

    broker.register_peer(sender).unwrap();
    broker.register_peer(target).unwrap();

    // First 5 requests within burst limit should succeed
    for _ in 0..5 {
        assert!(broker.route_message(sender, target, vec![0]).is_ok());
    }

    // 6th immediate request should be throttled
    let throttled = broker.route_message(sender, target, vec![0]).unwrap_err();
    assert_eq!(throttled, SignalServerError::RateLimitExceeded);
}
