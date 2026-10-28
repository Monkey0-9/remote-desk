# Spec Delta

## Purpose
Orchestrates cryptographic session setup, device authentication, Short Authentication String verification, and immediate input revocation.

## ADDED Requirements

### Requirement: Mutual cryptographic authentication
The system SHALL require connecting controllers to prove ownership of their Ed25519 identity key using a cryptographic challenge-response handshake before video or input streaming begins.

#### Scenario: Trusted controller handshake
- **WHEN** a previously paired controller signs the host challenge nonce
- **THEN** the host accepts the session and initiates WebRTC ICE connection

#### Scenario: Unpaired controller rejection
- **WHEN** an unknown controller initiates a connection without prior pairing approval
- **THEN** the host prompts the local desktop user for interactive pairing or rejects the session

### Requirement: Immediate input revocation
The system SHALL immediately release all active synthetic keyboard keys and mouse buttons upon session disconnect or pause.

#### Scenario: Abrupt disconnection with held keys
- **WHEN** a controller abruptly loses network connectivity while Shift and Left Mouse are pressed
- **THEN** the host input engine injects release events for all active scancodes within 10ms
