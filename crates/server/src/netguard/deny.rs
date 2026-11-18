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

/// IPv6 ranges as `(address, prefix length)`.
const V6: &[(Ipv6Addr, u8)] = &[
    (Ipv6Addr::UNSPECIFIED, 128),
    (Ipv6Addr::LOCALHOST, 128),
    // IPv4-mapped.
    (Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0, 0), 96),
    // NAT64, both well-known prefixes.
    (Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, 0, 0), 96),
    (Ipv6Addr::new(0x64, 0xff9b, 1, 0, 0, 0, 0, 0), 48),
    // Discard-only.
    (Ipv6Addr::new(0x100, 0, 0, 0, 0, 0, 0, 0), 64),
    // Teredo.
    (Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0), 32),
    // Documentation.
    (Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32),
    // 6to4.
    (Ipv6Addr::new(0x2002, 0, 0, 0, 0, 0, 0, 0), 16),
    // Unique local.
    (Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0), 7),
    // Link-local and the old site-local range.
    (Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0), 10),
    (Ipv6Addr::new(0xfec0, 0, 0, 0, 0, 0, 0, 0), 10),
    // Multicast.
    (Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0), 8),
];

fn in_v4(a: Ipv4Addr, net: Ipv4Addr, bits: u8) -> bool {
    let mask = u32::MAX.checked_shl(32 - u32::from(bits)).unwrap_or(0);
    u32::from(a) & mask == u32::from(net) & mask
}

fn in_v6(a: Ipv6Addr, net: Ipv6Addr, bits: u8) -> bool {
    let mask = u128::MAX.checked_shl(128 - u32::from(bits)).unwrap_or(0);
    u128::from(a) & mask == u128::from(net) & mask
}

/// The IPv4 address hidden inside an IPv6 one, in the forms that carry
/// one: mapped (`::ffff:a.b.c.d`), NAT64 (`64:ff9b::a.b.c.d`), 6to4
/// (`2002:aabb:ccdd::`) and the old IPv4-compatible form (`::a.b.c.d`).
fn embedded_v4(a: Ipv6Addr) -> Option<Ipv4Addr> {
    let o = a.octets();
    let low = Ipv4Addr::new(o[12], o[13], o[14], o[15]);
    if in_v6(a, Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0, 0), 96)
        || in_v6(a, Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, 0, 0), 96)
        || in_v6(a, Ipv6Addr::UNSPECIFIED, 96)
    {
        return Some(low);
    }
    if o[0] == 0x20 && o[1] == 0x02 {
        return Some(Ipv4Addr::new(o[2], o[3], o[4], o[5]));
    }
    None
}

/// True for an address that cannot be used for public HTTPS requests.
pub(super) fn is_denied(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => V4.iter().any(|&(n, b)| in_v4(a, n, b)),
        IpAddr::V6(a) => {
            V6.iter().any(|&(n, b)| in_v6(a, n, b))
                || embedded_v4(a).is_some_and(|v4| is_denied(IpAddr::V4(v4)))
        }
    }
}
