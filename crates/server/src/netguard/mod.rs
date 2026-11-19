//! Fixed-destination HTTPS for trusted server integrations. The address rules
//! come from the existing app's netguard (ADR 0071).

mod deny;
mod fetch;
pub(crate) use fetch::{checked_url, get_with_redirects, read_bounded};

use reqwest::dns::{Name, Resolve, Resolving};
use std::{io, net::SocketAddr, sync::Arc, time::Duration};

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
    let mut builder = reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .dns_resolver(Arc::new(PublicDns { hosts }))
        .connect_timeout(Duration::from_secs(5))
        .timeout(timeout)
        .user_agent("calternal");
    if hosts.is_none() {
        builder = builder.http1_only().pool_max_idle_per_host(0);
    }
    builder.build()
}

struct PublicDns {
    hosts: Option<&'static [&'static str]>,
}

impl Resolve for PublicDns {
    fn resolve(&self, name: Name) -> Resolving {
        let hosts = self.hosts;
        Box::pin(async move {
            if hosts.is_some_and(|hosts| !hosts.contains(&name.as_str())) {
                return Err(refused().into());
            }
            let addresses = public_addresses(name.as_str(), 0).await?;
            Ok(Box::new(addresses.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

/// Resolve an exact DNS name, refusing the entire set if any answer is private.
/// Dial the returned addresses without resolving the hostname again.
pub(crate) async fn public_addresses(host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
    let fqdn = format!("{host}.");
    let addresses: Vec<_> = tokio::net::lookup_host((fqdn.as_str(), port)).await?.collect();
    if addresses.is_empty() || addresses.iter().any(|address| deny::is_denied(address.ip())) {
        return Err(refused());
    }
    Ok(addresses)
}

fn refused() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "outbound destination refused",
    )
}
