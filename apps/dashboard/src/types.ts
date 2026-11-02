export interface NetworkInterface {
  name: string;
  ip: string;
  is_loopback: boolean;
  is_ipv6: boolean;
}

export interface OverviewData {
  host_service: boolean;
  device_id: string;
  device_name: string;
  unattended: boolean;
  fingerprint: string;
  public_key: string;
  bind_addr: string;
  capture_backend: string;
  input_backend: string;
  system_state: 'ready' | 'in_session' | 'stopped';
  signaling: 'connected' | 'standalone' | 'disconnected';
  connectivity: 'ready' | 'connected' | 'reconnecting';
  session_active: boolean;
  active_peer: string | null;
  interfaces: NetworkInterface[];
}

export interface TrustedDevice {
  device_id: string;
  name: string;
  trusted_since: string;
  status: 'paired' | 'online' | 'offline' | 'revoked';
}

export interface PerformanceMetrics {
  capture_fps: number;
  fps_delivered: number;
  encoding_latency_ms: number;
  encode_latency_ms: number;
  network_rtt_ms: number;
  rtt_ms: number;
  dropped_frames: number;
  cpu_usage_pct: number;
  ram_usage_mb: number;
  direct_p2p: boolean;
  connection_mode: string;
}

export interface SecurityAuditEntry {
  id: string;
  timestamp: string;
  device_id: string;
  event: string;
  outcome: 'granted' | 'denied' | 'revoked' | 'terminated';
  sas_code?: string;
}

export interface ServiceLogMessage {
  id: number;
  timestamp: string;
  level: 'INFO' | 'WARN' | 'ERROR' | 'DEBUG';
  message: string;
}
