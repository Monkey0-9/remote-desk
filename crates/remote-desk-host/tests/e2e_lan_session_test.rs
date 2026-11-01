use remote_desk_core::identity::DeviceIdentity;
use remote_desk_core::EphemeralKeyExchange;
use remote_desk_host::{HostConfig, HostService};
use remote_desk_proto::{
    pack_frame, KeyState, MouseButtonCode, MouseButtonPayload, MouseMoveAbsPayload,
    RemoteDeskMessage,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast;

fn hex_encode(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX_CHARS[(b >> 4) as usize] as char);
        s.push(HEX_CHARS[(b & 0x0F) as usize] as char);
    }
    s
}

#[tokio::test]
async fn test_e2e_lan_session_handshake_streaming_input_and_cleanup() {
    let port = 19999;
    let bind_addr = format!("127.0.0.1:{}", port);

    let host_id = DeviceIdentity::generate();
    let config = HostConfig {
        bind_addr: bind_addr.clone(),
        device_name: "Test-Linux-Host".to_string(),
        unattended: true, // auto-approve for automated integration test
        signal_server_url: None,
        turn_servers: vec![],
    };

    let host = HostService::new_mock(host_id, config);
    let host_arc = std::sync::Arc::new(host);
    let host_daemon = host_arc.clone();

    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
    let daemon_task = tokio::spawn(async move {
        let _ = host_daemon.run_daemon(shutdown_rx).await;
    });

    // Wait for listener to bind
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect simulated client (Android controller on LAN)
    let mut client_stream = TcpStream::connect(&bind_addr).await.expect("connect");

    // 1. Perform Handshake
    let client_id = DeviceIdentity::generate();
    let client_exchange = EphemeralKeyExchange::new();

    let handshake_json = serde_json::json!({
        "peer_id": client_id.device_id(),
        "peer_pubkey_hex": client_id.public_key_hex(),
        "peer_ephemeral_pubkey_hex": hex_encode(&client_exchange.public_key_bytes()),
    });

    client_stream
        .write_all(handshake_json.to_string().as_bytes())
        .await
        .expect("write handshake");

    // 2. Read Handshake Response
    let mut resp_buf = vec![0u8; 1024];
    let n = client_stream.read(&mut resp_buf).await.expect("read resp");
    assert!(n > 0);
    let resp_str = String::from_utf8_lossy(&resp_buf[..n]);
    assert!(resp_str.contains("\"status\":\"approved\""));
    assert!(resp_str.contains("\"sas_code\":"));

    // 3. Receive Desktop Video Frame from Host
    let mut frame_len_buf = [0u8; 4];
    client_stream
        .read_exact(&mut frame_len_buf)
        .await
        .expect("read frame length");
    let frame_len = u32::from_be_bytes(frame_len_buf) as usize;
    assert!(frame_len > 0, "Host must stream non-empty video frames");

    let mut frame_data = vec![0u8; frame_len];
    client_stream
        .read_exact(&mut frame_data)
        .await
        .expect("read frame payload");
    assert_eq!(frame_data.len(), frame_len);

    // 4. Send Interactive Input Frames (MouseMove + MouseButton)
    let move_msg = RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload {
        monitor_id: 0,
        x: 0.5,
        y: 0.5,
    });
    let move_frame = pack_frame(1, &move_msg).expect("pack move");
    client_stream
        .write_all(&move_frame)
        .await
        .expect("send move");

    let click_msg = RemoteDeskMessage::MouseButton(MouseButtonPayload {
        button: MouseButtonCode::Left,
        state: KeyState::Pressed,
    });
    let click_frame = pack_frame(2, &click_msg).expect("pack click");
    client_stream
        .write_all(&click_frame)
        .await
        .expect("send click");

    // Allow host to process inputs
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 5. Client Disconnects
    drop(client_stream);

    // Give host moment to detect EOF and trigger failsafe release
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Stop daemon
    let _ = shutdown_tx.send(());
    let _ = daemon_task.await;
}
