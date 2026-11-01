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
}
