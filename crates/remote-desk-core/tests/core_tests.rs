use remote_desk_core::{
    compute_sas_code, DeviceIdentity, EphemeralKeyExchange, SessionAction, SessionEvent,
    SessionState, SessionStateMachine,
};

#[test]
fn test_identity_generation_sign_verify() {
    let alice = DeviceIdentity::generate();
    let bob = DeviceIdentity::generate();

    let message = b"RemoteDesk authentication payload 2026";
    let signature = alice.sign(message);

    // Alice's public key should verify her signature
    assert!(DeviceIdentity::verify(&alice.verifying_key(), message, &signature).is_ok());

    // Bob's public key must fail to verify Alice's signature
    assert!(DeviceIdentity::verify(&bob.verifying_key(), message, &signature).is_err());

    // Corrupted message must fail
    assert!(DeviceIdentity::verify(&alice.verifying_key(), b"tampered", &signature).is_err());
}

#[test]
fn test_device_id_format() {
    let identity = DeviceIdentity::generate();
    let id = identity.device_id();

    assert!(id.starts_with("rd-"));
    assert_eq!(id.len(), 12); // "rd-XXXX-YYYY"
}

#[test]
fn test_ephemeral_diffie_hellman_and_sas_agreement() {
    let alice_identity = DeviceIdentity::generate();
    let bob_identity = DeviceIdentity::generate();

    let alice_exchange = EphemeralKeyExchange::new();
    let bob_exchange = EphemeralKeyExchange::new();

    let alice_pub = alice_exchange.public_key_bytes();
    let bob_pub = bob_exchange.public_key_bytes();

    // Alice computes shared secret using Bob's ephemeral pubkey
    let alice_shared = alice_exchange.compute_shared_secret(&bob_pub);

    // Bob computes shared secret using Alice's ephemeral pubkey
    let bob_shared = bob_exchange.compute_shared_secret(&alice_pub);

    // Shared secret must match perfectly
    assert_eq!(alice_shared, bob_shared);

    // Both parties compute Short Authentication String (SAS)
    let alice_sas = compute_sas_code(
        &alice_shared,
        &alice_identity.public_key_bytes(),
        &bob_identity.public_key_bytes(),
    );

    let bob_sas = compute_sas_code(
        &bob_shared,
        &alice_identity.public_key_bytes(),
        &bob_identity.public_key_bytes(),
    );

    assert_eq!(alice_sas, bob_sas);
    assert_eq!(alice_sas.len(), 7); // Format: "XXX-XXX"
}

#[test]
fn test_session_state_machine_flow() {
    let mut sm = SessionStateMachine::new();
    assert_eq!(sm.state(), &SessionState::Disconnected);
    assert!(!sm.is_active());
    assert!(!sm.can_inject_input());

    // 1. Handshake
    let act = sm
        .transition(SessionEvent::InitiateHandshake {
            peer_id: "rd-1234-5678".to_string(),
        })
        .unwrap();
    assert_eq!(act, SessionAction::None);

    // 2. SAS Generated -> Prompts approval
    let act = sm
        .transition(SessionEvent::SasGenerated {
            sas_code: "839-204".to_string(),
        })
        .unwrap();
    assert_eq!(
        act,
        SessionAction::PromptUserApproval {
            peer_id: "rd-1234-5678".to_string(),
            sas_code: "839-204".to_string()
        }
    );

    // 3. User grants approval -> Session active
    let act = sm.transition(SessionEvent::ApprovalGranted).unwrap();
    assert_eq!(
        act,
        SessionAction::StartStreaming {
            peer_id: "rd-1234-5678".to_string()
        }
    );
    assert!(sm.is_active());
    assert!(sm.can_inject_input());

    // 4. Disconnect -> Must trigger RevokeAllInput
    let act = sm
        .transition(SessionEvent::Disconnect("User quit".to_string()))
        .unwrap();
    assert_eq!(act, SessionAction::RevokeAllInput);
    assert!(!sm.is_active());
    assert!(!sm.can_inject_input());
}

#[test]
fn test_invalid_state_transition() {
    let mut sm = SessionStateMachine::new();
    // Cannot grant approval from Disconnected state
    let err = sm.transition(SessionEvent::ApprovalGranted);
    assert!(err.is_err());
}

#[test]
fn test_user_rejection_prevents_input_injection() {
    let mut sm = SessionStateMachine::new();
    sm.transition(SessionEvent::InitiateHandshake {
        peer_id: "rd-9999-8888".to_string(),
    })
    .unwrap();
    sm.transition(SessionEvent::SasGenerated {
        sas_code: "123-456".to_string(),
    })
    .unwrap();

    // User explicitly denies pairing request on host
    let action = sm.transition(SessionEvent::ApprovalDenied).unwrap();
    assert_eq!(
        action,
        SessionAction::CloseTransport {
            reason: "Permission denied".to_string()
        }
    );
    assert!(matches!(sm.state(), SessionState::Closed { .. }));
    assert!(!sm.is_active());
    assert!(!sm.can_inject_input());
}
