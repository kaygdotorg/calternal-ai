//! Extra local ranges for anonymous Class B fetches.
use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

/// A snapshot of attached networks and operator-provided public ranges.
#[derive(Default)]
pub(super) struct Local(Vec<(IpAddr, IpAddr)>);

impl Local {
    pub(super) fn load(extra: &str) -> io::Result<Self> {
        if extra.len() > 4096 {
            return Err(super::refused());
        }
        let mut result = Self(interfaces()?);
        for range in extra
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let (address, bits) = range.split_once('/').ok_or_else(super::refused)?;
            let address: IpAddr = address.parse().map_err(|_| super::refused())?;
            let bits: u32 = bits.parse().map_err(|_| super::refused())?;
            let mask = match address {
                IpAddr::V4(_) if bits <= 32 => {
                    IpAddr::V4(u32::MAX.checked_shl(32 - bits).unwrap_or(0).into())
                }
                IpAddr::V6(_) if bits <= 128 => {
                    IpAddr::V6(u128::MAX.checked_shl(128 - bits).unwrap_or(0).into())
                }
                _ => return Err(super::refused()),
            };
            result.0.push((address, mask));
        }
        Ok(result)
    }

    pub(super) fn denies(&self, ip: IpAddr) -> bool {
        self.0
            .iter()
            .any(|(address, mask)| match (ip, address, mask) {
                (IpAddr::V4(ip), IpAddr::V4(address), IpAddr::V4(mask)) => {
                    u32::from(ip) & u32::from(*mask) == u32::from(*address) & u32::from(*mask)
                }
                (IpAddr::V6(ip), IpAddr::V6(address), IpAddr::V6(mask)) => {
                    u128::from(ip) & u128::from(*mask) == u128::from(*address) & u128::from(*mask)
                }
                _ => false,
            })
    }
}

#[cfg(unix)]
fn interfaces() -> io::Result<Vec<(IpAddr, IpAddr)>> {
    let mut head = std::ptr::null_mut();
    // getifaddrs initializes the list on success. The guard owns its lifetime.
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return Err(io::Error::last_os_error());
    }
    struct Interfaces(*mut libc::ifaddrs);
    impl Drop for Interfaces {
        fn drop(&mut self) {
            // This is the head returned by the successful call above.
            unsafe { libc::freeifaddrs(self.0) };
        }
    }
    let list = Interfaces(head);
    let mut addresses = Vec::new();
    let mut next = list.0;
    // Entries and their sockaddr pointers remain live until the guard drops.
    while let Some(entry) = unsafe { next.as_ref() } {
        next = entry.ifa_next;
        let address = entry.ifa_addr;
        let mask = entry.ifa_netmask;
        if address.is_null() || mask.is_null() {
            continue;
        }
        // getifaddrs supplies address and netmask of the recorded family.
        match unsafe { (*address).sa_family as libc::c_int } {
            libc::AF_INET => {
                let (address, mask) = unsafe {
                    (
                        *address.cast::<libc::sockaddr_in>(),
                        *mask.cast::<libc::sockaddr_in>(),
                    )
                };
                addresses.push((
                    Ipv4Addr::from(address.sin_addr.s_addr.to_ne_bytes()).into(),
                    Ipv4Addr::from(mask.sin_addr.s_addr.to_ne_bytes()).into(),
                ));
            }
            libc::AF_INET6 => {
                let (address, mask) = unsafe {
                    (
                        *address.cast::<libc::sockaddr_in6>(),
                        *mask.cast::<libc::sockaddr_in6>(),
                    )
                };
                addresses.push((
                    Ipv6Addr::from(address.sin6_addr.s6_addr).into(),
                    Ipv6Addr::from(mask.sin6_addr.s6_addr).into(),
                ));
            }
            _ => {}
        }
    }
    Ok(addresses)
}

#[cfg(not(unix))]
fn interfaces() -> io::Result<Vec<(IpAddr, IpAddr)>> {
    Err(super::refused())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn configured_ranges_refuse_matching_addresses_and_bad_config() {
        let denied = Local::load("93.184.215.16/29,2606:4700:4700::/48").unwrap();
        for ip in ["93.184.215.17", "2606:4700:4700::1111"] {
            assert!(denied.denies(ip.parse().unwrap()));
        }
        assert!(!denied.denies("93.184.215.24".parse().unwrap()));
        assert!(Local::load("93.184.215.16/33").is_err());
    }
}
