//! Reserved and non-public address ranges ported from the existing app.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// IPv4 ranges as `(address, prefix length)`.
const V4: &[(Ipv4Addr, u8)] = &[
    (Ipv4Addr::new(0, 0, 0, 0), 8),
    (Ipv4Addr::new(10, 0, 0, 0), 8),
    // Carrier-grade NAT, which also covers NetBird.
    (Ipv4Addr::new(100, 64, 0, 0), 10),
    (Ipv4Addr::new(127, 0, 0, 0), 8),
    // Link-local, including the cloud metadata address.
    (Ipv4Addr::new(169, 254, 0, 0), 16),
    (Ipv4Addr::new(172, 16, 0, 0), 12),
    (Ipv4Addr::new(192, 0, 0, 0), 24),
    (Ipv4Addr::new(192, 0, 2, 0), 24),
    (Ipv4Addr::new(192, 88, 99, 0), 24),
    (Ipv4Addr::new(192, 168, 0, 0), 16),
    (Ipv4Addr::new(198, 18, 0, 0), 15),
    (Ipv4Addr::new(198, 51, 100, 0), 24),
    (Ipv4Addr::new(203, 0, 113, 0), 24),
    (Ipv4Addr::new(224, 0, 0, 0), 4),
    (Ipv4Addr::new(240, 0, 0, 0), 4),
    (Ipv4Addr::new(255, 255, 255, 255), 32),
];

// Within global unicast, refuse special-purpose protocol, documentation and
// transition ranges. Mapped IPv4, NAT64 and other non-global ranges are
// outside the allowed prefix.
const V6: &[(Ipv6Addr, u8)] = &[
    (Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0), 23),
    (Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2002, 0, 0, 0, 0, 0, 0, 0), 16),
    (Ipv6Addr::new(0x3fff, 0, 0, 0, 0, 0, 0, 0), 20),
];

fn in_v4(a: Ipv4Addr, net: Ipv4Addr, bits: u8) -> bool {
    let mask = u32::MAX.checked_shl(32 - u32::from(bits)).unwrap_or(0);
    u32::from(a) & mask == u32::from(net) & mask
}

fn in_v6(a: Ipv6Addr, net: Ipv6Addr, bits: u8) -> bool {
    let mask = u128::MAX.checked_shl(128 - u32::from(bits)).unwrap_or(0);
    u128::from(a) & mask == u128::from(net) & mask
}

/// True for an address that cannot be used for public HTTPS requests.
pub(super) fn is_denied(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => V4.iter().any(|&(n, b)| in_v4(a, n, b)),
        IpAddr::V6(a) => {
            !in_v6(a, Ipv6Addr::new(0x2000, 0, 0, 0, 0, 0, 0, 0), 3)
                || V6.iter().any(|&(n, b)| in_v6(a, n, b))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_destination_ranges_pass() {
        for denied in [
            "10.1.2.3", "127.0.0.1", "169.254.169.254", "100.64.0.1",
            "::1", "::ffff:127.0.0.1", "64:ff9b::a01:203", "2002:7f00:1::",
            "2001:2::1", "3fff::1", "5f00::1", "fc00::1", "fe80::1",
        ] {
            assert!(is_denied(denied.parse().unwrap()), "{denied}");
        }
        for public in ["8.8.8.8", "2606:4700:4700::1111"] {
            assert!(!is_denied(public.parse().unwrap()), "{public}");
        }
    }
}
