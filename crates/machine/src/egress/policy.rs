//! Numeric address policy for exact-name CONNECT requests.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;

use ipnet::IpNet;

use super::EgressError;

pub(crate) fn exclusions(source: &str) -> Result<Arc<[IpNet]>, EgressError> {
    if source.is_empty() {
        return Ok(Arc::default());
    }
    if source.len() > 8_192 {
        return Err(EgressError::InvalidExclusions);
    }
    source
        .split(',')
        .map(|prefix| {
            prefix
                .trim()
                .parse()
                .map_err(|_| EgressError::InvalidExclusions)
        })
        .collect::<Result<_, _>>()
}

pub(super) fn is_global_unicast(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => v4(address),
        IpAddr::V6(address) => v6(address),
    }
}

fn v4(address: Ipv4Addr) -> bool {
    let [first, ..] = address.octets();
    if !(1..=223).contains(&first) {
        return false;
    }
    let value = u32::from(address);
    const SPECIAL: &[(u32, u32)] = &[
        (0x0a00_0000, 8),  // Private
        (0x6440_0000, 10), // Shared address space
        (0x7f00_0000, 8),  // Loopback
        (0xa9fe_0000, 16), // Link local
        (0xac10_0000, 12), // Private
        (0xc000_0000, 24), // Protocol assignments
        (0xc000_0200, 24), // Documentation
        (0xc058_6300, 24), // 6to4 relay anycast
        (0xc0a8_0000, 16), // Private
        (0xc612_0000, 15), // Benchmarking
        (0xc633_6400, 24), // Documentation
        (0xcb00_7100, 24), // Documentation
    ];
    !SPECIAL.iter().any(|(network, prefix)| {
        let mask = u32::MAX << (32 - prefix);
        value & mask == *network
    })
}

fn v6(address: Ipv6Addr) -> bool {
    let [first, second, ..] = address.segments();
    // Require public unicast allocation space. This also excludes translation,
    // mapped and compatible IPv4, multicast, unique local and link-local space.
    (first & 0xe000 == 0x2000)
        && !(first == 0x2001 && (second <= 0x01ff || second == 0x0db8))
        && first != 0x2002
        && !(first == 0x3fff && second <= 0x0fff)
}
