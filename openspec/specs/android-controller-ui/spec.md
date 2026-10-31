# android-controller-ui Specification

## Purpose
Provides the mobile Android controller interface with device management, video decoding canvas, and gesture input translation.

## Requirements

### Requirement: Android navigation and device list
The system SHALL display saved computers, their reachability status, and clear connection triggers without continuous background network polling.

#### Scenario: Displaying paired computers
- **WHEN** the user opens the Android Devices screen
- **THEN** saved computers are listed with names, platform badges, and current availability status

### Requirement: Dual touch interaction modes
The system SHALL provide seamless switching between Direct-Touch mode and Mouse-Pointer mode on the active remote canvas.

#### Scenario: Switching from direct touch to pointer mode
- **WHEN** the user taps the Mouse mode toggle in the floating session toolbar
- **THEN** a visible remote cursor appears and single-finger dragging moves the cursor relatively
