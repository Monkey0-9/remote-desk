# windows-host-engine Specification

## Purpose
Captures desktop video frames and injects input on Windows operating systems via DXGI Desktop Duplication, Windows Graphics Capture, and Win32 SendInput.

## Requirements

### Requirement: Windows Desktop Duplication & Graphics Capture
Under Windows 10/11 sessions, the host engine SHALL interface with DXGI Desktop Duplication API and Windows Graphics Capture (WGC) to extract desktop frame buffers.

#### Scenario: Primary monitor capture
- **WHEN** start_capture(0) is invoked on DxgiCaptureBackend
- **THEN** raw RGBA frame buffers are produced at the target display refresh rate without crashing or leaking Direct3D resources.

### Requirement: Win32 SendInput Injection & Stuck-Key Fail-Safe
The Windows host backend SHALL translate normalized floating-point coordinates (0.0..1.0) to Win32 absolute mouse space (0..65535) and inject input via SendInput. Disconnection SHALL release all held keys and mouse buttons immediately.

#### Scenario: Input coordinate mapping and fail-safe key release
- **WHEN** the controller sends normalized coordinates
- **THEN** the cursor moves in Win32 desktop coordinates
- **AND** when the session abruptly terminates, release_all() immediately clears all held keys and buttons.
