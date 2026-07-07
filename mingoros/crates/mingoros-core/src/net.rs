//! Host network-interface enumeration — so the operator can *pick* which local
//! interface DDS binds to (the direct-link Ethernet to the DV PC) instead of
//! typing its IP. See `docs/CONNECT.md`.

use serde::Serialize;

/// A local network interface + one of its IPv4 addresses. The `ip` is exactly
/// what you pass as the DDS interface bind (`mingoROS --iface <ip>` / the app's
/// interface picker).
#[derive(Debug, Clone, Serialize)]
pub struct NetInterface {
    /// OS interface name, e.g. `en7` (macOS) / `eth0` (Linux) / `Ethernet` (Win).
    pub name: String,
    /// The interface's IPv4 address, e.g. `10.42.0.2`.
    pub ip: String,
    /// True for loopback (`lo`/`lo0`, 127.0.0.0/8) — never the direct link.
    pub loopback: bool,
}

/// List the host's IPv4 interfaces, **non-loopback first** then by name. IPv6 is
/// skipped (the direct-cable link is IPv4). Returns empty if enumeration fails.
pub fn list_interfaces() -> Vec<NetInterface> {
    let mut out: Vec<NetInterface> = match if_addrs::get_if_addrs() {
        Ok(ifs) => ifs
            .into_iter()
            .filter_map(|i| {
                let loopback = i.is_loopback();
                let name = i.name.clone();
                match i.addr {
                    if_addrs::IfAddr::V4(v4) => Some(NetInterface {
                        name,
                        ip: v4.ip.to_string(),
                        loopback,
                    }),
                    if_addrs::IfAddr::V6(_) => None,
                }
            })
            .collect(),
        Err(e) => {
            tracing::warn!("network interface enumeration failed: {e}");
            Vec::new()
        }
    };
    out.sort_by(|a, b| {
        a.loopback
            .cmp(&b.loopback)
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}

/// Is `ip` still one of the host's interface IPs? False means the NIC that DDS
/// was bound to has gone (cable/adapter unplugged, DHCP lease dropped) — the
/// signal for a silent link loss on a direct-cable DV PC link.
pub fn ip_present(ip: std::net::IpAddr) -> bool {
    let ip = ip.to_string();
    list_interfaces().iter().any(|nif| nif.ip == ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_at_least_loopback() {
        // Every host has a loopback; enumeration should not panic and should
        // return it (sorted last).
        let ifs = list_interfaces();
        assert!(
            ifs.iter().any(|i| i.loopback),
            "expected a loopback interface"
        );
        // Non-loopback entries sort before loopback ones: for every adjacent
        // pair, a loopback is never followed by a non-loopback.
        assert!(
            ifs.windows(2).all(|w| !w[0].loopback || w[1].loopback),
            "loopback interfaces must sort last"
        );
    }

    #[test]
    fn ip_present_finds_loopback_and_rejects_bogus() {
        // Loopback is always up; a TEST-NET-3 address (RFC 5737) never is.
        assert!(ip_present("127.0.0.1".parse().unwrap()));
        assert!(!ip_present("203.0.113.255".parse().unwrap()));
    }
}
