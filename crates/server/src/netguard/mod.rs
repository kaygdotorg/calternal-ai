//! Fixed-destination HTTPS for trusted server integrations. The address rules
//! come from the existing app's netguard (ADR 0071).

mod deny;
mod fetch;
mod local;
pub(crate) use fetch::{check_url, checked_url, get_with_redirects, read_bounded, redirect_target};

use reqwest::dns::{Name, Resolve, Resolving};
use std::{io, net::IpAddr, sync::Arc, time::Duration};

/// Atomically replaced set of the configured origin’s resolved addresses.
pub(crate) type OriginAddresses = Arc<arc_swap::ArcSwapOption<Vec<IpAddr>>>;

/// Creates an HTTPS-only client with no proxy, redirects or automatic retries.
/// DNS answers are rejected as a set if any address is disallowed. Reqwest
/// connects to the checked addresses and retains the host name for TLS.
pub(crate) fn client(hosts: &'static [&'static str]) -> Result<reqwest::Client, reqwest::Error> {
    client_for(Some(hosts), Duration::from_secs(12))
}

/// Build a DNS-checked direct HTTPS transport. With no fixed host list, use
/// the public-host policy at dial time and pair it with per-hop authorization
/// in `get_with_redirects`. Anonymous public fetches open a fresh checked
/// HTTP/1 connection for each hop. Cost: one process-local transport build.
/// Intended for the plugin network broker in Forgejo #58 and fixed-host adapters.
pub(crate) fn client_for(
    hosts: Option<&'static [&'static str]>,
    timeout: Duration,
) -> Result<reqwest::Client, reqwest::Error> {
    build(
        hosts,
        timeout,
        Arc::new(local::Local::default()),
        false,
        None,
    )
}

/// Checked Class B dialing policy. Build with an interface enumeration on a
/// blocking worker, then resolve once per hop and check every DNS answer.
pub(crate) struct ContentPolicy {
    local: local::Local,
    own: OriginAddresses,
    fixture: bool,
}

impl ContentPolicy {
    /// Read attached interfaces and parse operator ranges. One blocking call.
    pub(crate) fn load(
        ranges: &str,
        nat64: Option<&str>,
        fixture: bool,
        own: OriginAddresses,
    ) -> io::Result<Self> {
        Ok(Self {
            local: local::Local::load(ranges, nat64)?,
            own,
            fixture,
        })
    }

    /// Resolve an admitted hostname and select its first checked address.
    /// The Class B caller must use this address for both quota and connection.
    pub(crate) async fn resolve(&self, host: &str) -> io::Result<std::net::SocketAddr> {
        #[cfg(all(feature = "test-plugins", debug_assertions))]
        if self.fixture && host == "content-fetch.test" {
            return Ok(([127, 0, 0, 1], 443).into());
        }
        Ok(self.checked_addresses(host, 443).await?[0])
    }

    /// Resolve an exact hostname and apply the configured deny policy to all answers.
    /// Dial one of these addresses directly. The port does not affect admission.
    pub(crate) async fn checked_addresses(
        &self,
        host: &str,
        port: u16,
    ) -> io::Result<Vec<std::net::SocketAddr>> {
        let answers = lookup(host, self.fixture).await?;
        if answers.is_empty()
            || answers.iter().any(|a| {
                deny::is_denied(a.ip())
                    || self.local.denies(a.ip())
                    || own_denies(Some(&self.own), a.ip())
            })
        {
            return Err(refused());
        }
        Ok(answers
            .into_iter()
            .map(|address| std::net::SocketAddr::new(address.ip(), port))
            .collect())
    }
}

/// Build one TLS client on a blocking worker. Only the checked host override
/// can resolve, so the connector uses the quota's IP while preserving SNI.
pub(crate) fn content_client(
    host: &str,
    checked: std::net::SocketAddr,
    timeout: Duration,
) -> Result<reqwest::Client, reqwest::Error> {
    struct NoDns;
    impl Resolve for NoDns {
        fn resolve(&self, _: Name) -> Resolving {
            Box::pin(async { Err(refused().into()) })
        }
    }
    builder(timeout)
        .dns_resolver(Arc::new(NoDns))
        .resolve(host, checked)
        .http1_only()
        .pool_max_idle_per_host(0)
        .build()
}

async fn lookup(host: &str, fixture: bool) -> io::Result<Vec<std::net::SocketAddr>> {
    if let Some(answers) = fixture_answers(fixture, host) {
        return Ok(answers);
    }
    // Exact FQDN, with no local search suffixes.
    let name = format!("{host}.");
    Ok(tokio::net::lookup_host((name.as_str(), 0)).await?.collect())
}

fn builder(timeout: Duration) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(5))
        .timeout(timeout)
        .user_agent("calternal")
}

fn build(
    hosts: Option<&'static [&'static str]>,
    timeout: Duration,
    local: Arc<local::Local>,
    fixture: bool,
    own: Option<OriginAddresses>,
) -> Result<reqwest::Client, reqwest::Error> {
    let mut builder = builder(timeout).dns_resolver(Arc::new(PublicDns {
        hosts,
        local,
        fixture,
        own,
    }));
    if hosts.is_none() {
        builder = builder.http1_only().pool_max_idle_per_host(0);
    }
    builder.build()
}

struct PublicDns {
    own: Option<OriginAddresses>,
    fixture: bool,
    local: Arc<local::Local>,
    hosts: Option<&'static [&'static str]>,
}

impl Resolve for PublicDns {
    fn resolve(&self, name: Name) -> Resolving {
        let hosts = self.hosts;
        let local = self.local.clone();
        let fixture = self.fixture;
        let own = self.own.clone();
        Box::pin(async move {
            if hosts.is_some_and(|hosts| !hosts.contains(&name.as_str())) {
                return Err(refused().into());
            }
            let addresses = lookup(name.as_str(), fixture).await?;
            if addresses.is_empty()
                || addresses.iter().any(|a| {
                    deny::is_denied(a.ip())
                        || local.denies(a.ip())
                        || own_denies(own.as_ref(), a.ip())
                })
            {
                return Err(refused().into());
            }
            Ok(Box::new(addresses.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

fn own_denies(own: Option<&OriginAddresses>, ip: IpAddr) -> bool {
    own.is_some_and(|own| {
        own.load()
            .as_ref()
            .is_none_or(|addresses| addresses.contains(&ip))
    })
}

#[cfg(test)]
mod origin_tests {
    use super::*;

    #[test]
    fn origin_refresh_changes_the_ip_policy_even_for_aliases() {
        let own = OriginAddresses::default();
        let old = "93.184.215.17".parse().unwrap();
        let new = "93.184.215.18".parse().unwrap();
        assert!(own_denies(Some(&own), old));
        own.store(Some(Arc::new(vec![old])));
        assert!(own_denies(Some(&own), old));
        assert!(!own_denies(Some(&own), new));
        own.store(Some(Arc::new(vec![new])));
        assert!(own_denies(Some(&own), new));
        assert!(!own_denies(Some(&own), old));
    }
}

fn refused() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "outbound destination refused",
    )
}

fn fixture_answers(_fixture: bool, _name: &str) -> Option<Vec<std::net::SocketAddr>> {
    #[cfg(all(feature = "test-plugins", debug_assertions))]
    if _fixture {
        let answers: &[&str] = match _name {
            "private.content-fetch.test" => &["10.2.3.4:0"],
            "loopback.content-fetch.test" => &["127.0.0.1:0"],
            "link-local.content-fetch.test" => &["169.254.1.2:0"],
            "local-v6.content-fetch.test" => &["[fe80::1]:0"],
            "mixed.content-fetch.test" => &["93.184.215.17:0", "127.0.0.1:0"],
            "server-range.content-fetch.test" => &["93.184.215.18:0"],
            _ => return None,
        };
        return Some(
            answers
                .iter()
                .map(|value| value.parse().expect("fixture address"))
                .collect(),
        );
    }
    None
}
