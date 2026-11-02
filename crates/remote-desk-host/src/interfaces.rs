use std::net::{IpAddr, UdpSocket};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub ip: String,
    pub is_loopback: bool,
    pub is_ipv6: bool,
}

/// Discovers reachable non-loopback network interfaces on the local host.
/// Discloses actual reachable IP addresses rather than wildcard bind `0.0.0.0`.
pub fn discover_network_interfaces() -> Vec<NetworkInterfaceInfo> {
    let mut interfaces = Vec::new();

    // 1. Probe primary active IPv4 interface using kernel routing table query
    // Connecting an unconnected UDP socket does not transmit any packets over the wire,
    // but prompts the kernel to resolve the local egress IP assigned to the primary interface.
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(local) = socket.local_addr() {
                let ip = local.ip();
                if !ip.is_loopback() && !ip.is_unspecified() {
                    interfaces.push(NetworkInterfaceInfo {
                        name: "Primary LAN (Default Egress)".to_string(),
                        ip: ip.to_string(),
                        is_loopback: false,
                        is_ipv6: ip.is_ipv6(),
                    });
                }
            }
        }
    }

    // 2. Discover local interfaces from /proc/net/if_inet6 or /sys/class/net if on Linux
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
            for entry in entries.flatten() {
                let ifname = entry.file_name().to_string_lossy().to_string();
                if ifname == "lo" {
                    continue;
                }
                // Try reading operstate
                let operstate_path = entry.path().join("operstate");
                if let Ok(state) = std::fs::read_to_string(operstate_path) {
                    if state.trim() == "up" {
                        // Avoid duplicates if already discovered as primary
                        if !interfaces.iter().any(|i| i.name.contains(&ifname)) {
                            // If we don't have this interface yet, add it with name
                            interfaces.push(NetworkInterfaceInfo {
                                name: format!("Active Interface ({})", ifname),
                                ip: format!("Active ({})", ifname),
                                is_loopback: false,
                                is_ipv6: false,
                            });
                        }
                    }
                }
            }
        }
    }

    // Always ensure localhost is known as fallback reference
    interfaces.push(NetworkInterfaceInfo {
        name: "Loopback Interface (Localhost Only)".to_string(),
        ip: "127.0.0.1".to_string(),
        is_loopback: true,
        is_ipv6: false,
    });

    interfaces
}

/// Formats interface list into human-friendly strings for dashboard and startup logs
pub fn format_reachable_addresses(bind_addr: &str) -> Vec<String> {
    let port = bind_addr.split(':').nth(1).unwrap_or("4443");
    let ifaces = discover_network_interfaces();

    let mut addrs = Vec::new();
    for iface in ifaces {
        if !iface.is_loopback && iface.ip.parse::<IpAddr>().is_ok() {
            addrs.push(format!("http://{}:{}/", iface.ip, port));
        }
    }

    if addrs.is_empty() {
        addrs.push(format!("http://127.0.0.1:{}/", port));
    }
    addrs
}
