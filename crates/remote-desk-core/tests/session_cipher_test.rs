use remote_desk_core::{CipherRole, CryptoError, SessionCrypto};

#[test]
fn test_session_cipher_bidirectional_roundtrip() {
    let shared_secret = [0x42u8; 32];
    let transcript_hash = [0x77u8; 32];

    let host_crypto = SessionCrypto::new(&shared_secret, &transcript_hash, CipherRole::Host);
    let client_crypto = SessionCrypto::new(&shared_secret, &transcript_hash, CipherRole::Client);

    // 1. Host encrypts video frame / control message -> Client decrypts
    let host_msg = b"VIDEO_FRAME_RGBA_PIXELS_SAMPLE_DATA_1234567890";
    let (seq, ciphertext) = host_crypto.encrypt(host_msg).expect("host encrypt");
    assert_eq!(seq, 1);
    assert_ne!(ciphertext, host_msg);

    let decrypted_by_client = client_crypto
        .decrypt(seq, &ciphertext)
        .expect("client decrypt");
    assert_eq!(decrypted_by_client, host_msg);

    // 2. Client encrypts input event -> Host decrypts
    let input_msg = b"INPUT_MOUSE_MOVE_X_0.5_Y_0.5";
    let (client_seq, input_ciphertext) = client_crypto.encrypt(input_msg).expect("client encrypt");
    assert_eq!(client_seq, 1);

    let decrypted_by_host = host_crypto
        .decrypt(client_seq, &input_ciphertext)
        .expect("host decrypt");
    assert_eq!(decrypted_by_host, input_msg);
}

#[test]
fn test_session_cipher_packet_packing_and_unpacking() {
    let shared_secret = [0x33u8; 32];
    let transcript_hash = [0x99u8; 32];

    let host_crypto = SessionCrypto::new(&shared_secret, &transcript_hash, CipherRole::Host);
    let client_crypto = SessionCrypto::new(&shared_secret, &transcript_hash, CipherRole::Client);

    let payload = b"Hello from authenticated host desktop!";
    let packet = host_crypto
        .pack_encrypted_packet(payload)
        .expect("pack packet");

    // Packet begins with 4-byte length
    assert!(packet.len() > 4 + 8 + payload.len());
    let declared_len = u32::from_be_bytes(packet[0..4].try_into().unwrap()) as usize;
    assert_eq!(declared_len, packet.len() - 4);

    let unpacked = client_crypto
        .unpack_encrypted_payload(&packet[4..])
        .expect("unpack packet");
    assert_eq!(unpacked, payload);
}

#[test]
fn test_session_cipher_detects_tampering_and_forgery() {
    let shared_secret = [0x11u8; 32];
    let transcript_hash = [0x22u8; 32];

    let host_crypto = SessionCrypto::new(&shared_secret, &transcript_hash, CipherRole::Host);
    let client_crypto = SessionCrypto::new(&shared_secret, &transcript_hash, CipherRole::Client);

    let plaintext = b"Sensitive Keystroke: Password123";
    let (seq, mut ciphertext) = host_crypto.encrypt(plaintext).expect("encrypt");

    // Flip 1 bit in ciphertext
    ciphertext[5] ^= 0x01;

    let result = client_crypto.decrypt(seq, &ciphertext);
    assert_eq!(result, Err(CryptoError::AuthenticationFailed));
}

#[test]
fn test_session_cipher_rejects_replays() {
    let shared_secret = [0x55u8; 32];
    let transcript_hash = [0x66u8; 32];

    let host_crypto = SessionCrypto::new(&shared_secret, &transcript_hash, CipherRole::Host);
    let client_crypto = SessionCrypto::new(&shared_secret, &transcript_hash, CipherRole::Client);

    let msg = b"Click Left Button";
    let (seq, ciphertext) = host_crypto.encrypt(msg).expect("encrypt");

    // First decryption succeeds
    let first = client_crypto.decrypt(seq, &ciphertext);
    assert!(first.is_ok());

    // Replay with identical sequence number is rejected
    let replay = client_crypto.decrypt(seq, &ciphertext);
    assert_eq!(
        replay,
        Err(CryptoError::ReplayDetected {
            expected: 2,
            actual: 1,
        })
    );
}
