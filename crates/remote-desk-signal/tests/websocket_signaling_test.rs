use futures_util::{SinkExt, StreamExt};
use remote_desk_signal::server::{ClientSignalRequest, ServerSignalResponse};
use remote_desk_signal::SignalingServer;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn test_signaling_server_http_health_probe() {
    let server = SignalingServer::bind("127.0.0.1:0", 100).await.unwrap();
    let addr = server.local_addr().to_string();

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let server_task = tokio::spawn(async move {
        let _ = server.run(shutdown_rx).await;
    });

    // Send HTTP GET /health
    let mut stream = TcpStream::connect(&addr).await.unwrap();
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();

    let mut buf = vec![0u8; 1024];
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);

    assert!(response.contains("200 OK"));
    assert!(response.contains(r#""status":"ok""#));
    assert!(response.contains(r#""service":"remote-desk-signal""#));

    let _ = shutdown_tx.send(());
    let _ = server_task.await;
}

#[tokio::test]
async fn test_websocket_registration_and_realtime_routing() {
    let server = SignalingServer::bind("127.0.0.1:0", 100).await.unwrap();
    let addr = server.local_addr().to_string();

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let server_task = tokio::spawn(async move {
        let _ = server.run(shutdown_rx).await;
    });

    let ws_url = format!("ws://{}/", addr);

    use remote_desk_core::identity::DeviceIdentity;
    use std::time::{SystemTime, UNIX_EPOCH};

    let id_a = DeviceIdentity::generate();
    let id_b = DeviceIdentity::generate();

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Helper for hex encoding
    let to_hex = |bytes: &[u8]| -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            use std::fmt::Write;
            write!(&mut s, "{:02x}", b).unwrap();
        }
        s
    };

    // 1. Connect Client A (Host)
    let (mut client_a, _) = connect_async(&ws_url).await.unwrap();
    let challenge_a = format!(
        "RemoteDesk-Signaling-Register:{}:{}",
        now_ms,
        id_a.device_id()
    );
    let sig_a = id_a.sign(challenge_a.as_bytes());
    let reg_a = ClientSignalRequest::Register {
        device_id: id_a.device_id(),
        timestamp_ms: now_ms,
        public_key_hex: id_a.public_key_hex(),
        signature_hex: Some(to_hex(&sig_a)),
    };
    client_a
        .send(Message::Text(serde_json::to_string(&reg_a).unwrap()))
        .await
        .unwrap();

    let msg = client_a.next().await.unwrap().unwrap();
    let text = msg.to_text().unwrap();
    let resp: ServerSignalResponse = serde_json::from_str(text).unwrap();
    assert!(matches!(resp, ServerSignalResponse::Registered { .. }));

    // 2. Connect Client B (Controller)
    let (mut client_b, _) = connect_async(&ws_url).await.unwrap();
    let challenge_b = format!(
        "RemoteDesk-Signaling-Register:{}:{}",
        now_ms,
        id_b.device_id()
    );
    let sig_b = id_b.sign(challenge_b.as_bytes());
    let reg_b = ClientSignalRequest::Register {
        device_id: id_b.device_id(),
        timestamp_ms: now_ms,
        public_key_hex: id_b.public_key_hex(),
        signature_hex: Some(to_hex(&sig_b)),
    };
    client_b
        .send(Message::Text(serde_json::to_string(&reg_b).unwrap()))
        .await
        .unwrap();

    let msg = client_b.next().await.unwrap().unwrap();
    assert!(matches!(
        serde_json::from_str(msg.to_text().unwrap()).unwrap(),
        ServerSignalResponse::Registered { .. }
    ));

    // 3. Client B sends signaling message to Client A
    let route_req = ClientSignalRequest::Route {
        target_id: id_a.device_id(),
        envelope_json: format!(
            r#"{{"sender_device_id":"{}","type":"SdpOffer","sdp":"v=0..."}}"#,
            id_b.device_id()
        ),
    };
    client_b
        .send(Message::Text(serde_json::to_string(&route_req).unwrap()))
        .await
        .unwrap();

    // Client B gets confirmation
    let ack = client_b.next().await.unwrap().unwrap();
    assert!(matches!(
        serde_json::from_str(ack.to_text().unwrap()).unwrap(),
        ServerSignalResponse::Routed { .. }
    ));

    // Client A receives routed message live over WebSocket!
    let incoming = client_a.next().await.unwrap().unwrap();
    let incoming_resp: ServerSignalResponse =
        serde_json::from_str(incoming.to_text().unwrap()).unwrap();
    match incoming_resp {
        ServerSignalResponse::Incoming { message } => {
            assert!(message.contains("SdpOffer"));
        }
        other => panic!("Unexpected response on client A: {:?}", other),
    }

    let _ = shutdown_tx.send(());
    let _ = server_task.await;
}

#[tokio::test]
async fn test_websocket_registration_proof_of_possession_and_security() {
    use remote_desk_core::identity::DeviceIdentity;
    use std::time::{SystemTime, UNIX_EPOCH};

    let server = SignalingServer::bind("127.0.0.1:0", 100).await.unwrap();
    let addr = server.local_addr().to_string();

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let server_task = tokio::spawn(async move {
        let _ = server.run(shutdown_rx).await;
    });

    let ws_url = format!("ws://{}/", addr);

    // 1. Unauthenticated client cannot route
    let (mut client, _) = connect_async(&ws_url).await.unwrap();
    let unauth_route = ClientSignalRequest::Route {
        target_id: "rd-victim".to_string(),
        envelope_json: "{}".to_string(),
    };
    client
        .send(Message::Text(serde_json::to_string(&unauth_route).unwrap()))
        .await
        .unwrap();

    let err_msg = client.next().await.unwrap().unwrap();
    let resp: ServerSignalResponse = serde_json::from_str(err_msg.to_text().unwrap()).unwrap();
    assert!(
        matches!(resp, ServerSignalResponse::Error { message } if message.contains("Unauthenticated"))
    );

    // 1b. Registration without signature is rejected
    let identity = DeviceIdentity::generate();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let nosig_reg = ClientSignalRequest::Register {
        device_id: identity.device_id(),
        timestamp_ms: now_ms,
        public_key_hex: identity.public_key_hex(),
        signature_hex: None,
    };
    client
        .send(Message::Text(serde_json::to_string(&nosig_reg).unwrap()))
        .await
        .unwrap();

    let err_msg_nosig = client.next().await.unwrap().unwrap();
    let resp_nosig: ServerSignalResponse =
        serde_json::from_str(err_msg_nosig.to_text().unwrap()).unwrap();
    assert!(
        matches!(resp_nosig, ServerSignalResponse::Error { message } if message.contains("missing mandatory Ed25519 signature"))
    );

    // 1c. Registration with mismatched device_id is rejected
    let challenge_mismatch = format!("RemoteDesk-Signaling-Register:{}:rd-9999-9999", now_ms);
    let sig_mismatch = identity.sign(challenge_mismatch.as_bytes());
    let mut sig_mismatch_hex = String::with_capacity(128);
    for b in sig_mismatch {
        use std::fmt::Write;
        write!(&mut sig_mismatch_hex, "{:02x}", b).unwrap();
    }
    let mismatch_reg = ClientSignalRequest::Register {
        device_id: "rd-9999-9999".to_string(), // Mismatched ID
        timestamp_ms: now_ms,
        public_key_hex: identity.public_key_hex(),
        signature_hex: Some(sig_mismatch_hex),
    };
    client
        .send(Message::Text(serde_json::to_string(&mismatch_reg).unwrap()))
        .await
        .unwrap();

    let err_msg_mismatch = client.next().await.unwrap().unwrap();
    let resp_mismatch: ServerSignalResponse =
        serde_json::from_str(err_msg_mismatch.to_text().unwrap()).unwrap();
    assert!(
        matches!(resp_mismatch, ServerSignalResponse::Error { message } if message.contains("does not match public key derived ID"))
    );

    // 2. Client with forged signature is rejected
    let identity = DeviceIdentity::generate();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let forged_sig_hex = "00".repeat(64); // 64 zero bytes = invalid signature
    let forged_reg = ClientSignalRequest::Register {
        device_id: identity.device_id(),
        timestamp_ms: now_ms,
        public_key_hex: identity.public_key_hex(),
        signature_hex: Some(forged_sig_hex),
    };
    client
        .send(Message::Text(serde_json::to_string(&forged_reg).unwrap()))
        .await
        .unwrap();

    let err_msg2 = client.next().await.unwrap().unwrap();
    let resp2: ServerSignalResponse = serde_json::from_str(err_msg2.to_text().unwrap()).unwrap();
    assert!(
        matches!(resp2, ServerSignalResponse::Error { message } if message.contains("invalid cryptographic signature"))
    );

    // 3. Client with valid proof-of-possession Ed25519 signature succeeds
    let challenge = format!(
        "RemoteDesk-Signaling-Register:{}:{}",
        now_ms,
        identity.device_id()
    );
    let valid_sig = identity.sign(challenge.as_bytes());
    let mut valid_sig_hex = String::with_capacity(128);
    for b in valid_sig {
        use std::fmt::Write;
        write!(&mut valid_sig_hex, "{:02x}", b).unwrap();
    }

    let valid_reg = ClientSignalRequest::Register {
        device_id: identity.device_id(),
        timestamp_ms: now_ms,
        public_key_hex: identity.public_key_hex(),
        signature_hex: Some(valid_sig_hex),
    };
    client
        .send(Message::Text(serde_json::to_string(&valid_reg).unwrap()))
        .await
        .unwrap();

    let success_msg = client.next().await.unwrap().unwrap();
    let success_resp: ServerSignalResponse =
        serde_json::from_str(success_msg.to_text().unwrap()).unwrap();
    assert!(matches!(
        success_resp,
        ServerSignalResponse::Registered { .. }
    ));

    let _ = shutdown_tx.send(());
    let _ = server_task.await;
}

#[tokio::test]
async fn test_signaling_server_http_dashboard_and_browser_endpoints() {
    let server = SignalingServer::bind("127.0.0.1:0", 100).await.unwrap();
    let addr = server.local_addr().to_string();

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let server_task = tokio::spawn(async move {
        let _ = server.run(shutdown_rx).await;
    });

    // 1. GET / returns HTML dashboard
    {
        let mut stream = TcpStream::connect(&addr).await.unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        let response = String::from_utf8_lossy(&buf);
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Type: text/html"));
        assert!(response.contains("Remote-Desk Rendezvous Broker"));
    }

    // 2. GET /signal returns HTML dashboard
    {
        let mut stream = TcpStream::connect(&addr).await.unwrap();
        stream
            .write_all(b"GET /signal HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        let response = String::from_utf8_lossy(&buf);
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Type: text/html"));
        assert!(response.contains("RENDEZVOUS BROKER ONLINE"));
    }

    // 3. GET /api/info returns JSON info
    {
        let mut stream = TcpStream::connect(&addr).await.unwrap();
        stream
            .write_all(b"GET /api/info HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        let response = String::from_utf8_lossy(&buf);
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("application/json"));
        assert!(response.contains(r#""status":"online""#));
        assert!(response.contains(r#""capacity":100"#));
    }

    // 4. GET /favicon.ico returns 204 No Content
    {
        let mut stream = TcpStream::connect(&addr).await.unwrap();
        stream
            .write_all(b"GET /favicon.ico HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        let response = String::from_utf8_lossy(&buf);
        assert!(response.starts_with("HTTP/1.1 204 No Content"));
    }

    let _ = shutdown_tx.send(());
    let _ = server_task.await;
}
