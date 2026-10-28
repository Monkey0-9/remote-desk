# Spec Delta

## Purpose
Provides standardized deployment definitions, systemd service units, and security governance policies.

## ADDED Requirements

### Requirement: Linux & Windows Deployment Artifacts
The repository SHALL maintain standard production deployment definitions including systemd service units, Freedesktop desktop application entries, and Docker Compose configurations for self-hosting.

#### Scenario: Service installation via systemd unit
- **WHEN** the systemd unit remote-desk.service is loaded and started
- **THEN** the host streaming daemon starts in user-session scope with standard security sandbox constraints.

### Requirement: Vulnerability Policy & Release Runbook
The project SHALL provide a documented security vulnerability disclosure policy and release runbook specifying pre-release quality gates and disaster recovery rollback procedures.

#### Scenario: Security reporting following NIST SSDF
- **WHEN** a vulnerability is submitted according to SECURITY.md
- **THEN** initial acknowledgement occurs within 24 hours, triage within 72 hours, and patch release adheres to release runbook procedures.
