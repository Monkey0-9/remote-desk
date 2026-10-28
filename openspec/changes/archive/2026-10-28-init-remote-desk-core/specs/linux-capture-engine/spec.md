# Spec Delta

## Purpose
Captures desktop video frames on Linux operating systems via Wayland XDG Desktop Portal and X11 extensions.

## ADDED Requirements

### Requirement: Wayland screen capture via Portal
The system SHALL request screen capture via org.freedesktop.portal.ScreenCast and receive video frame buffers through PipeWire.

#### Scenario: User grants portal permission
- **WHEN** the host requests a ScreenCast session and the user selects a display
- **THEN** the system receives PipeWire stream node parameters and streams video frames

#### Scenario: User denies portal permission
- **WHEN** the user dismisses or rejects the portal capture dialog
- **THEN** the capture backend returns PermissionDenied error without crashing

### Requirement: X11 capture fallback
The system SHALL support capturing desktop frames via MIT-SHM shared memory under native X11 sessions.

#### Scenario: X11 screen capture
- **WHEN** running under an X11 display server
- **THEN** the system captures full or damaged desktop rects into shared memory buffers
