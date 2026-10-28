# worldwide-turn-signaling Specification

## Purpose
Provides global WebRTC connectivity across symmetric NATs via STUN/TURN relays and ephemeral signaling brokering.

## Requirements

### Requirement: ICE STUN & TURN Relay Fallback
The transport subsystem SHALL support standard ICE negotiation with STUN discovery and TURN relay fallback for traversing symmetric NATs and restrictive firewalls.

#### Scenario: Fallback to TURN relay when direct P2P is blocked
- **WHEN** direct P2P ICE candidate checks fail
- **THEN** the session routes media and DataChannels via configured TURN relay
- **AND** all traffic across the relay remains end-to-end encrypted via DTLS 1.3 and SRTP.

### Requirement: In-Memory Rendezvous Signaling Broker
The signaling subsystem SHALL provide a standalone rendezvous server routing signaling envelopes by 9-digit Device ID (rd-XXXX-XXXX) with token-bucket rate limiting and zero persistent database storage.

#### Scenario: Ephemeral exchange of SDP and ICE candidates
- **WHEN** a controller deposits an authenticated SDP offer for a registered host
- **THEN** the message is queued in the host's ephemeral mailbox and rate-limited to 5 requests per second per sender
- **AND** no cleartext session payloads or IP addresses are written to permanent storage.
