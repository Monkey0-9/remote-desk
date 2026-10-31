# remote-core-proto Specification

## Purpose
Provides standardized, cross-platform binary wire protocol definitions, message framing, and serialization for real-time remote control.

## Requirements

### Requirement: Binary framing format
The system SHALL frame all data channel messages with magic bytes 0x5244, an 8-bit version identifier, an 8-bit message type, a 64-bit sequence counter, and a 32-bit payload length.

#### Scenario: Valid frame reception
- **WHEN** a binary frame with magic bytes 0x5244 and known message type arrives
- **THEN** the parser successfully extracts the sequence counter and decodes the message payload

#### Scenario: Corrupted frame rejection
- **WHEN** a binary frame with invalid magic bytes arrives
- **THEN** the parser rejects the packet with FramingError and does not crash

### Requirement: Normalized input event encoding
The system SHALL encode pointer movements, clicks, and wheel deltas into compact binary representations using normalized float coordinates between 0.0 and 1.0.

#### Scenario: Absolute mouse move encoding
- **WHEN** a controller dispatches an absolute mouse move at coordinates (0.5, 0.25)
- **THEN** the serialized packet contains monitor ID and 32-bit IEEE 754 float values preserving precision
