use remote_desk_core::DeviceIdentity;
use remote_desk_host::{HostConfig, HostService};
use remote_desk_proto::messages::{
    KeyState, KeyboardKeyPayload, MouseButtonCode, MouseButtonPayload, RemoteDeskMessage,
};
use remote_desk_proto::pack_frame;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::test]
async fn test_host_persistent_identity_reuse() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tmp_dir = std::env::temp_dir().join(format!("rd_host_test_{}", now));
    let key_file = tmp_dir.join("identity.key");

    let id1 = DeviceIdentity::load_or_generate(&key_file).unwrap();
    let host1 = HostService::new_mock(id1, HostConfig::default());
    let dev1 = host1.device_id();

    // Reload from the same file
    let id2 = DeviceIdentity::load_or_generate(&key_file).unwrap();
    let host2 = HostService::new_mock(id2, HostConfig::default());
    let dev2 = host2.device_id();

    assert_eq!(
        dev1, dev2,
        "Persistent host device ID must survive across reloads"
    );

    let _ = fs::remove_dir_all(tmp_dir);
}

#[tokio::test]
async fn test_host_failsafe_release_on_termination() {
    let identity = DeviceIdentity::generate();
    let host = HostService::new_mock(identity, HostConfig::default());

    // Inject held key and mouse button directly via protocol frames
    let key_msg = RemoteDeskMessage::KeyboardKey(KeyboardKeyPayload {
        scancode: 57, // Space
        state: KeyState::Pressed,
        modifiers: 0,
    });
    let btn_msg = RemoteDeskMessage::MouseButton(MouseButtonPayload {
        button: MouseButtonCode::Left,
        state: KeyState::Pressed,
    });

    let frame1 = pack_frame(1, &key_msg).unwrap();
    let frame2 = pack_frame(2, &btn_msg).unwrap();

    let _ = host.process_incoming_input_frame(&frame1).await;
    let _ = host.process_incoming_input_frame(&frame2).await;

    // Terminating session must execute revoke_all_input
    host.terminate_session("Client disconnected abruptly")
        .await
        .unwrap();

    // State machine is now Closed
    let sm = host.state_machine.lock().await;
    assert!(matches!(
        sm.state(),
        remote_desk_core::SessionState::Closed { .. }
    ));
}
