use remote_desk_core::identity::DeviceIdentity;
use remote_desk_core::pairing::EphemeralKeyExchange;
use remote_desk_core::session::{SessionEvent, SessionState, SessionStateMachine};
use std::fs;

#[test]
fn test_identity_persistence_and_reload() {
    let tmp_dir = std::env::temp_dir().join(format!("rd_test_{}", rand::random::<u64>()));
    let key_path = tmp_dir.join("identity.key");

    // First load: creates new key
    let id1 = DeviceIdentity::load_or_generate(&key_path).expect("generate initial key");
    assert!(key_path.exists());

    // Verify file permissions on Unix (0600)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(&key_path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "Key file must have 0600 permissions"
        );
    }

    // Verify 32-byte fingerprint
    let fp = id1.fingerprint_bytes();
    assert_eq!(fp.len(), 32);
    let fp_hex = id1.fingerprint_hex();
    assert_eq!(fp_hex.len(), 64);

    // Second load: reloads identical key
    let id2 = DeviceIdentity::load_or_generate(&key_path).expect("reload key");
    assert_eq!(id1.public_key_bytes(), id2.public_key_bytes());
    assert_eq!(id1.device_id(), id2.device_id());
    assert_eq!(id1.fingerprint_hex(), id2.fingerprint_hex());

    let _ = fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_pairing_low_order_point_rejection() {
    let host_exchange = EphemeralKeyExchange::new();
    // All-zero public key (known low-order point)
    let weak_pubkey = [0u8; 32];
    let res = host_exchange.compute_shared_secret(&weak_pubkey);
    assert!(res.is_err(), "Must reject weak/low-order public key");
}

#[test]
fn test_session_state_machine_12_states_and_monitor_retention() {
    let mut sm = SessionStateMachine::new();
    assert_eq!(sm.state(), &SessionState::Disconnected);

    // Transition to Authenticating
    sm.transition(SessionEvent::InitiateHandshake {
        peer_id: "peer-1".to_string(),
    })
    .unwrap();
    assert!(matches!(sm.state(), SessionState::Authenticating { .. }));

    // Transition to WaitingApproval
    sm.transition(SessionEvent::SasGenerated {
        sas_code: "123-456".to_string(),
    })
    .unwrap();
    assert!(matches!(sm.state(), SessionState::WaitingApproval { .. }));

    // Approval granted -> Active (Streaming)
    sm.transition(SessionEvent::ApprovalGranted).unwrap();
    assert!(sm.is_active());
    assert!(sm.can_inject_input());

    // Switch to monitor 2
    sm.transition(SessionEvent::SwitchMonitor(2)).unwrap();
    assert_eq!(
        sm.state(),
        &SessionState::Active {
            peer_id: "peer-1".to_string(),
            active_monitor: 2,
        }
    );

    // Pause -> Must retain active_monitor 2!
    sm.transition(SessionEvent::Pause).unwrap();
    assert_eq!(
        sm.state(),
        &SessionState::Paused {
            peer_id: "peer-1".to_string(),
            active_monitor: 2,
        }
    );
    assert!(!sm.can_inject_input());

    // Resume -> Must restore active_monitor 2!
    sm.transition(SessionEvent::Resume).unwrap();
    assert_eq!(
        sm.state(),
        &SessionState::Active {
            peer_id: "peer-1".to_string(),
            active_monitor: 2,
        }
    );
    assert!(sm.can_inject_input());
}

#[test]
fn test_identity_rotation_and_backup() {
    let tmp_dir = std::env::temp_dir().join(format!("rd_rot_{}", rand::random::<u64>()));
    let key_path = tmp_dir.join("identity.key");

    let initial = DeviceIdentity::load_or_generate(&key_path).expect("initial key");
    let (rotated, backup) = DeviceIdentity::rotate_identity(&key_path).expect("rotate key");

    assert_eq!(initial.public_key_bytes(), backup.public_key_bytes());
    assert_ne!(rotated.public_key_bytes(), initial.public_key_bytes());
    assert!(key_path.with_extension("key.bak").exists());

    // Test recovery from backup
    let recovered = DeviceIdentity::recover_from_backup(&key_path).expect("recover from backup");
    assert_eq!(recovered.public_key_bytes(), initial.public_key_bytes());

    let _ = fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_trusted_device_store_lifecycle() {
    use remote_desk_core::identity::TrustedDeviceStore;

    let tmp_dir = std::env::temp_dir().join(format!("rd_trust_{}", rand::random::<u64>()));
    let store_path = tmp_dir.join("trusted_devices.json");

    let mut store = TrustedDeviceStore::new();
    let dev_id = "rd-1234-5678".to_string();
    let pub_key = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90".to_string();

    store.trust_device(
        dev_id.clone(),
        pub_key.clone(),
        "Work Laptop".to_string(),
        1700000000,
    );
    assert!(store.is_trusted(&dev_id, &pub_key));
    assert_eq!(store.count(), 1);

    store.save_to_file(&store_path).expect("save trusted store");
    assert!(store_path.exists());

    let reloaded = TrustedDeviceStore::load_from_file(&store_path).expect("reload trusted store");
    assert!(reloaded.is_trusted(&dev_id, &pub_key));

    // Test revocation
    store.revoke_device(&dev_id);
    assert!(!store.is_trusted(&dev_id, &pub_key));
    assert_eq!(store.count(), 0);

    let _ = fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_handshake_transcript_signing_and_verification() {
    use remote_desk_core::pairing::{derive_session_keys, HandshakeTranscript};

    let host_id = DeviceIdentity::generate();
    let client_id = DeviceIdentity::generate();

    let transcript = HandshakeTranscript::new(
        1,
        host_id.public_key_bytes(),
        client_id.public_key_bytes(),
        [1u8; 32],
        [2u8; 32],
        "sess-xyz".to_string(),
        vec!["screen".to_string(), "input".to_string()],
    );

    let signature = transcript.sign(&host_id);
    let verify_res = transcript.verify(&host_id.verifying_key(), &signature);
    assert!(verify_res.is_ok(), "Transcript signature must verify");

    // Rejection on wrong verifying key
    let wrong_verify = transcript.verify(&client_id.verifying_key(), &signature);
    assert!(wrong_verify.is_err(), "Must reject wrong verifying key");

    // Key derivation check
    let shared_secret = [99u8; 32];
    let session_keys = derive_session_keys(&shared_secret, &transcript.compute_transcript_hash());
    assert_ne!(session_keys.media_key, session_keys.input_key);
    assert_ne!(session_keys.media_key, [0u8; 32]);

    // Test that length delimiters prevent transcript hash collision across variable-length fields
    let t_a = HandshakeTranscript::new(
        1,
        host_id.public_key_bytes(),
        client_id.public_key_bytes(),
        [1u8; 32],
        [2u8; 32],
        "sess".to_string(),
        vec!["screen".to_string()],
    );
    let t_b = HandshakeTranscript::new(
        1,
        host_id.public_key_bytes(),
        client_id.public_key_bytes(),
        [1u8; 32],
        [2u8; 32],
        "ses".to_string(),
        vec!["sscreen".to_string()],
    );
    assert_ne!(
        t_a.compute_transcript_hash(),
        t_b.compute_transcript_hash(),
        "Different variable-length field segmentations must yield distinct hashes"
    );
}
