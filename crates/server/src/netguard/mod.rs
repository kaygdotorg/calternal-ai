//! Fixed-destination HTTPS for trusted server integrations. The address rules
//! come from the existing app's netguard (ADR 0071).

mod deny;
mod fetch;
mod local;
pub(crate) use fetch::{check_url, checked_url, get_with_redirects, read_bounded, redirect_target};

use reqwest::dns::{Name, Resolve, Resolving};
use std::{io, sync::Arc, time::Duration};

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
    build(hosts, timeout, Arc::new(local::Local::default()), false)
}

/// Direct anonymous HTTPS with attached and operator-configured networks
/// denied by the resolver. Refreshes attached addresses for each client build.
pub(crate) fn content_client(
    ranges: &str,
    timeout: Duration,
    fixture: bool,
) -> io::Result<reqwest::Client> {
    build(
        None,
        timeout,
        Arc::new(local::Local::load(ranges)?),
        fixture,
    )
    .map_err(io::Error::other)
}

fn build(
    hosts: Option<&'static [&'static str]>,
    timeout: Duration,
    local: Arc<local::Local>,
    fixture: bool,
) -> Result<reqwest::Client, reqwest::Error> {
    let mut builder = reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .dns_resolver(Arc::new(PublicDns {
            hosts,
            local,
            fixture,
        }))
        .connect_timeout(Duration::from_secs(5))
        .timeout(timeout)
        .user_agent("calternal");
    if hosts.is_none() {
        builder = builder.http1_only().pool_max_idle_per_host(0);
    }
    builder.build()
}

struct PublicDns {
    fixture: bool,
    local: Arc<local::Local>,
    hosts: Option<&'static [&'static str]>,
}

impl Resolve for PublicDns {
    fn resolve(&self, name: Name) -> Resolving {
        let hosts = self.hosts;
        let local = self.local.clone();
        let fixture = self.fixture;
        Box::pin(async move {
            if hosts.is_some_and(|hosts| !hosts.contains(&name.as_str())) {
                return Err(refused().into());
            }
            // A trailing dot makes the OS query the exact DNS name without
            // trying local search suffixes. Only the DNS query uses that dot.
            let fqdn = format!("{}.", name.as_str());
            let addresses = if let Some(answers) = fixture_answers(fixture, name.as_str()) {
                answers
            } else {
                tokio::net::lookup_host((fqdn.as_str(), 0)).await?.collect()
            };
            if addresses.is_empty()
                || addresses
                    .iter()
                    .any(|a| deny::is_denied(a.ip()) || local.denies(a.ip()))
            {
                return Err(refused().into());
            }
            Ok(Box::new(addresses.into_iter()) as reqwest::dns::Addrs)
        })
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
