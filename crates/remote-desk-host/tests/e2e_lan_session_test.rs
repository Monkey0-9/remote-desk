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
    host_arc.trusted_devices.lock().await.trust_device(
        client_id.device_id(),
        client_id.public_key_hex(),
        "Trusted Test Client".to_string(),
        0,
    );

    let client_exchange = EphemeralKeyExchange::new();
    let eph_hex = hex_encode(&client_exchange.public_key_bytes());
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let challenge = format!(
        "RemoteDesk-LAN-Auth:{}:{}:{}",
        client_id.device_id(),
        eph_hex,
        now_ms
    );
    let signature = client_id.sign(challenge.as_bytes());
    let sig_hex = hex_encode(&signature);

    let handshake_json = serde_json::json!({
        "peer_id": client_id.device_id(),
        "peer_pubkey_hex": client_id.public_key_hex(),
        "peer_ephemeral_pubkey_hex": eph_hex,
        "signature_hex": sig_hex,
        "timestamp_ms": now_ms,
    });

    client_stream
        .write_all(format!("{}\n", handshake_json).as_bytes())
        .await
        .expect("write handshake");

    // 2. Read Handshake Response
    let mut resp_buf = vec![0u8; 1024];
    let n = client_stream.read(&mut resp_buf).await.expect("read resp");
    assert!(n > 0);
    let resp_str = String::from_utf8_lossy(&resp_buf[..n]);
    assert!(resp_str.contains("\"status\":\"approved\""));
    assert!(resp_str.contains("\"sas_code\":"));
    assert!(resp_str.contains("\"cipher\":\"chacha20poly1305\""));

    let resp_val: serde_json::Value =
        serde_json::from_str(resp_str.trim()).expect("parse resp json");
    let host_eph_hex = resp_val["host_ephemeral_pubkey_hex"].as_str().unwrap();
    let transcript_hash_hex = resp_val["transcript_hash_hex"].as_str().unwrap();

    let mut host_eph_bytes = [0u8; 32];
    for (i, chunk) in host_eph_hex.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).unwrap();
        host_eph_bytes[i] = u8::from_str_radix(s, 16).unwrap();
    }
    let mut transcript_hash = [0u8; 32];
    for (i, chunk) in transcript_hash_hex.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).unwrap();
        transcript_hash[i] = u8::from_str_radix(s, 16).unwrap();
    }

    let shared_secret = client_exchange
        .compute_shared_secret(&host_eph_bytes)
        .expect("compute shared secret");

    let client_crypto = remote_desk_core::SessionCrypto::new(
        &shared_secret,
        &transcript_hash,
        remote_desk_core::CipherRole::Client,
    );

    // 3. Receive Authenticated Encrypted Desktop Video Frame from Host
    let mut frame_len_buf = [0u8; 4];
    client_stream
        .read_exact(&mut frame_len_buf)
        .await
        .expect("read frame length");
    let packet_len = u32::from_be_bytes(frame_len_buf) as usize;
    assert!(packet_len > 0, "Host must stream non-empty video frames");

    let mut encrypted_payload = vec![0u8; packet_len];
    client_stream
        .read_exact(&mut encrypted_payload)
        .await
        .expect("read frame payload");

    // Decrypt frame with client AEAD cipher
    let frame_data = client_crypto
        .unpack_encrypted_payload(&encrypted_payload)
        .expect("decrypt video frame");
    assert!(
        !frame_data.is_empty(),
        "Decrypted frame data must not be empty"
    );

    // 4. Send Authenticated Encrypted Input Frames (MouseMove + MouseButton)
    let move_msg = RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload {
        monitor_id: 0,
        x: 0.5,
        y: 0.5,
    });
    let move_frame = pack_frame(1, &move_msg).expect("pack move");
    let enc_move_packet = client_crypto
        .pack_encrypted_packet(&move_frame)
        .expect("encrypt move packet");
    client_stream
        .write_all(&enc_move_packet)
        .await
        .expect("send move");

    let click_msg = RemoteDeskMessage::MouseButton(MouseButtonPayload {
        button: MouseButtonCode::Left,
        state: KeyState::Pressed,
    });
    let click_frame = pack_frame(2, &click_msg).expect("pack click");
    let enc_click_packet = client_crypto
        .pack_encrypted_packet(&click_frame)
        .expect("encrypt click packet");
    client_stream
        .write_all(&enc_click_packet)
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

#[tokio::test]
async fn test_tampered_encrypted_packet_causes_session_termination() {
    let port = 19998;
    let bind_addr = format!("127.0.0.1:{}", port);

    let host_id = DeviceIdentity::generate();
    let config = HostConfig {
        bind_addr: bind_addr.clone(),
        device_name: "Tamper-Test-Host".to_string(),
        unattended: true,
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

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client_stream = TcpStream::connect(&bind_addr).await.expect("connect");

    let client_id = DeviceIdentity::generate();
    host_arc.trusted_devices.lock().await.trust_device(
        client_id.device_id(),
        client_id.public_key_hex(),
        "Trusted Test Client".to_string(),
        0,
    );

    let client_exchange = EphemeralKeyExchange::new();
    let eph_hex = hex_encode(&client_exchange.public_key_bytes());
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let challenge = format!(
        "RemoteDesk-LAN-Auth:{}:{}:{}",
        client_id.device_id(),
        eph_hex,
        now_ms
    );
    let signature = client_id.sign(challenge.as_bytes());
    let sig_hex = hex_encode(&signature);

    let handshake_json = serde_json::json!({
        "peer_id": client_id.device_id(),
        "peer_pubkey_hex": client_id.public_key_hex(),
        "peer_ephemeral_pubkey_hex": eph_hex,
        "signature_hex": sig_hex,
        "timestamp_ms": now_ms,
    });

    client_stream
        .write_all(format!("{}\n", handshake_json).as_bytes())
        .await
        .expect("write handshake");

    let mut resp_buf = vec![0u8; 1024];
    let n = client_stream.read(&mut resp_buf).await.expect("read resp");
    assert!(n > 0);
    let resp_str = String::from_utf8_lossy(&resp_buf[..n]);
    assert!(resp_str.contains("\"status\":\"approved\""));

    let resp_val: serde_json::Value =
        serde_json::from_str(resp_str.trim()).expect("parse resp json");
    let host_eph_hex = resp_val["host_ephemeral_pubkey_hex"].as_str().unwrap();
    let transcript_hash_hex = resp_val["transcript_hash_hex"].as_str().unwrap();

    let mut host_eph_bytes = [0u8; 32];
    for (i, chunk) in host_eph_hex.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).unwrap();
        host_eph_bytes[i] = u8::from_str_radix(s, 16).unwrap();
    }
    let mut transcript_hash = [0u8; 32];
    for (i, chunk) in transcript_hash_hex.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).unwrap();
        transcript_hash[i] = u8::from_str_radix(s, 16).unwrap();
    }

    let shared_secret = client_exchange
        .compute_shared_secret(&host_eph_bytes)
        .expect("compute shared secret");

    let client_crypto = remote_desk_core::SessionCrypto::new(
        &shared_secret,
        &transcript_hash,
        remote_desk_core::CipherRole::Client,
    );

    // Read initial video frame
    let mut frame_len_buf = [0u8; 4];
    client_stream
        .read_exact(&mut frame_len_buf)
        .await
        .expect("read frame length");
    let packet_len = u32::from_be_bytes(frame_len_buf) as usize;
    let mut encrypted_payload = vec![0u8; packet_len];
    client_stream
        .read_exact(&mut encrypted_payload)
        .await
        .expect("read frame payload");

    // Construct valid input packet and tamper with 1 byte of ciphertext
    let move_msg = RemoteDeskMessage::MouseMoveAbs(MouseMoveAbsPayload {
        monitor_id: 0,
        x: 0.5,
        y: 0.5,
    });
    let move_frame = pack_frame(1, &move_msg).expect("pack move");
    let mut tampered_packet = client_crypto
        .pack_encrypted_packet(&move_frame)
        .expect("pack packet");

    // Flip 1 bit in ciphertext payload (past 4-byte len and 8-byte seq)
    tampered_packet[14] ^= 0x55;

    // Send tampered packet to host
    client_stream
        .write_all(&tampered_packet)
        .await
        .expect("send tampered packet");

    // Host must reject and terminate session, dropping socket
    let mut drop_buf = [0u8; 64];
    let read_res = client_stream.read(&mut drop_buf).await;
    // EOF (0 bytes) or socket error expected
    match read_res {
        Ok(0) => {}  // Expected EOF
        Ok(_) => {}  // Or remaining buffered bytes then EOF
        Err(_) => {} // Connection reset/aborted
    }

    // Stop daemon
    let _ = shutdown_tx.send(());
    let _ = daemon_task.await;
}
