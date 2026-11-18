//! Sign-in settings.

use std::sync::Arc;
use std::time::Duration;
use url::Url;

/// The wall clock sign-in reads, in Unix milliseconds. Production uses the system
/// clock; tests inject one to make expiry deterministic. Cost: one call.
#[derive(Clone)]
pub struct Clock(Arc<dyn Fn() -> i64 + Send + Sync>);
impl Clock {
    /// The system clock ([`calternal_types::now_ms`]).
    pub fn system() -> Self {
        Self(Arc::new(calternal_types::now_ms))
    }
    /// A clock that reads `now`, for tests.
    pub fn from_fn(now: impl Fn() -> i64 + Send + Sync + 'static) -> Self {
        Self(Arc::new(now))
    }
    /// The current time in Unix milliseconds.
    pub fn now(&self) -> i64 {
        (self.0)()
    }
}
impl std::fmt::Debug for Clock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Clock")
    }
}

/// How sign-in behaves on this server.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// The web address people use, like `https://calternal.cloud`. Passkeys
    /// are bound to its host name, so a passkey made for one host never
    /// works on another.
    pub origin: Url,
    /// The host name passkeys are bound to (the origin's host).
    pub rp_id: String,
    /// Require a verified email before redeeming a signup token.
    pub email_required: bool,
    /// Enable the core OIDC transport on a host that is ready to offer it.
    /// The standard server starts with the transport disabled.
    pub oidc_enabled: bool,
    /// How long a setup token works. A day by default.
    pub setup_ttl: Duration,
    /// Idle session lifetime; source touch extends it at most once per hour.
    pub session_ttl: Duration,
    /// The clock idle-session expiry is judged with; the system clock by default.
    pub clock: Clock,
    /// How long a directory request receipt is kept (7 days by default, the same
    /// retention as space receipts); a retry after that runs as a new request.
    pub receipt_retention: Duration,
    /// How long to retain expired, revoked or redeemed invitation rows.
    pub invite_retention: Duration,
    /// How long a passkey prompt may stay open.
    pub ceremony_ttl: Duration,
    /// Maximum prompts in each of the independent Login, Setup and Add pools.
    pub max_ceremonies: usize,
    /// Maximum setup starts per source and per verified token per minute.
    pub setup_failures_per_minute: u32,
    /// Maximum anonymous login starts per transport client per minute.
    pub login_starts_per_minute: u32,
    /// Reserved for the later DAV port: maximum unknown app passwords per source
    /// per minute, and the same
    /// per credential. Past it, unknown secrets get 429 for the rest of the
    /// minute. Valid secrets bypass these failure limits; any cold lookup
    /// can still exceed the global queue's bounded wait.
    pub dav_failures_per_minute: u32,
    /// Reserved for the later DAV port: app-password mints per account per minute.
    pub dav_mints_per_minute: u32,
    /// Reserved for the later DAV port: maximum cold app-password lookups.
    /// Excess requests get 429 immediately; cached passwords bypass this cap.
    pub dav_max_pending_cold: usize,
    /// Reserved for the later DAV port: how long cached passwords are trusted
    /// before `auth.db` is read
    /// again. Bounds how late a revoke made by another process is seen.
    pub app_password_recheck: Duration,
    /// Freshness window for interactive account proof (legacy field name).
    /// Passkey management permits recent setup/recovery registration; callers
    /// using `require_fresh` with registration disabled require a sign-in assertion.
    pub app_password_fresh: Duration,
    /// Trusted reverse proxy networks. Empty by default: forwarded IPs are ignored.
    pub trusted_proxies: Vec<(std::net::IpAddr, u8)>,
}

impl AuthConfig {
    /// Settings for an origin, with sensible defaults for everything else.
    pub fn new(origin: &str) -> crate::Result<Self> {
        let url = Url::parse(origin).map_err(|_| crate::Error::invalid(origin))?;
        let host = url
            .host_str()
            .ok_or_else(|| crate::Error::invalid(origin))?
            .to_owned();
        let local = matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]");
        match url.scheme() {
            "https" => {}
            "http" if local => {}
            _ => return Err(crate::Error::invalid(origin)),
        }
        let bare = matches!(url.path(), "" | "/")
            && url.query().is_none()
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none();
        if !bare {
            return Err(crate::Error::invalid(origin));
        }
        Ok(Self {
            rp_id: host,
            email_required: false,
            oidc_enabled: false,
            origin: url,
            setup_ttl: Duration::from_secs(24 * 3600),
            session_ttl: Duration::from_secs(90 * 24 * 3600),
            clock: Clock::system(),
            receipt_retention: crate::spaces::DEFAULT_RECEIPT_RETENTION,
            invite_retention: Duration::from_secs(
                u64::from(crate::invites::DEFAULT_INVITE_RETENTION_DAYS) * 86_400,
            ),
            ceremony_ttl: Duration::from_secs(5 * 60),
            max_ceremonies: 10_000,
            setup_failures_per_minute: 20,
            login_starts_per_minute: 120,
            dav_failures_per_minute: 20,
            dav_mints_per_minute: 10,
            dav_max_pending_cold: 256,
            app_password_recheck: Duration::from_secs(30),
            app_password_fresh: Duration::from_secs(10 * 60),
            trusted_proxies: Vec::new(),
        })
    }

    /// Configures comma-separated trusted proxy CIDRs. Only these transport
    /// peers may supply forwarding information. Invalid input fails startup.
    pub fn set_trusted_proxies(&mut self, cidrs: &str) -> crate::Result<()> {
        let mut networks = Vec::new();
        for entry in cidrs.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (address, prefix) = entry
                .split_once('/')
                .ok_or_else(|| crate::Error::invalid(entry))?;
            let address = address
                .parse::<std::net::IpAddr>()
                .map_err(|_| crate::Error::invalid(entry))?;
            let prefix = prefix
                .parse::<u8>()
                .ok()
                .filter(|p| *p <= if address.is_ipv4() { 32 } else { 128 })
                .ok_or_else(|| crate::Error::invalid(entry))?;
            networks.push((address, prefix));
        }
        self.trusted_proxies = networks;
        Ok(())
    }

    /// Whether a transport peer may supply forwarding information.
    pub fn trusts_proxy(&self, address: std::net::IpAddr) -> bool {
        self.trusted_proxies
            .iter()
            .any(|(network, prefix)| match (*network, address) {
                (std::net::IpAddr::V4(a), std::net::IpAddr::V4(b)) if *prefix <= 32 => {
                    let mask = u32::MAX.checked_shl(32 - u32::from(*prefix)).unwrap_or(0);
                    (u32::from(a) & mask) == (u32::from(b) & mask)
                }
                (std::net::IpAddr::V6(a), std::net::IpAddr::V6(b)) if *prefix <= 128 => {
                    let mask = u128::MAX.checked_shl(128 - u32::from(*prefix)).unwrap_or(0);
                    (u128::from(a) & mask) == (u128::from(b) & mask)
                }
                _ => false,
            })
    }

    /// The origin as browsers send it in the `Origin` header.
    pub fn origin_header(&self) -> String {
        self.origin.origin().ascii_serialization()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins() {
        let c = AuthConfig::new("https://calternal.cloud").unwrap();
        assert_eq!(c.rp_id, "calternal.cloud");
        assert_eq!(c.origin_header(), "https://calternal.cloud");
        let c = AuthConfig::new("http://localhost:8080").unwrap();
        assert_eq!(c.origin_header(), "http://localhost:8080");
        assert!(AuthConfig::new("http://calternal.cloud").is_err());
        assert!(AuthConfig::new("https://calternal.cloud/app").is_err());
        assert!(AuthConfig::new("https://a:b@calternal.cloud").is_err());
        assert!(AuthConfig::new("calternal.cloud").is_err());
        assert!(AuthConfig::new("ftp://calternal.cloud").is_err());
    }
}

#[cfg(test)]
mod proxy_tests {
    use super::*;
    #[test]
    fn proxy_trust_is_explicit_and_validated_atomically() {
        let mut config = AuthConfig::new("http://localhost:8080").unwrap();
        assert!(!config.trusts_proxy("127.0.0.1".parse().unwrap()));
        config.set_trusted_proxies("192.0.2.7/24, ::1/128").unwrap();
        assert!(config.trusts_proxy("192.0.2.50".parse().unwrap()));
        assert!(config.trusts_proxy("::1".parse().unwrap()));
        assert!(!config.trusts_proxy("192.0.3.7".parse().unwrap()));
        for bad in [
            "127.0.0.1",
            "127.0.0.1/33",
            "::1/129",
            "localhost/8",
            "::1/-1",
        ] {
            assert!(config.set_trusted_proxies(bad).is_err());
            assert!(config.trusts_proxy("192.0.2.50".parse().unwrap()));
        }
        config.set_trusted_proxies("0.0.0.0/0, ::/0").unwrap();
        assert!(config.trusts_proxy("203.0.113.4".parse().unwrap()));
        assert!(config.trusts_proxy("2001:db8::1".parse().unwrap()));
    }
}
