//! Fixed-destination HTTPS for trusted server integrations. The address rules
//! come from the existing app's netguard (ADR 0071).

mod deny;

use reqwest::dns::{Name, Resolve, Resolving};
use std::{io, sync::Arc, time::Duration};

/// Creates an HTTPS-only client with no proxy, redirects or automatic retries.
/// DNS answers are rejected as a set if any address is disallowed. Reqwest
/// connects to the checked addresses and retains the host name for TLS.
pub(crate) fn client(hosts: &'static [&'static str]) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .dns_resolver(Arc::new(PublicDns { hosts }))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(12))
        .user_agent("calternal")
        .build()
}

struct PublicDns {
    hosts: &'static [&'static str],
}

impl Resolve for PublicDns {
    fn resolve(&self, name: Name) -> Resolving {
        let hosts = self.hosts;
        Box::pin(async move {
            if !hosts.contains(&name.as_str()) {
                return Err(refused().into());
            }
            // A trailing dot makes the OS query the exact DNS name without
            // trying local search suffixes. Only the DNS query uses that dot.
            let fqdn = format!("{}.", name.as_str());
            let addresses: Vec<_> = tokio::net::lookup_host((fqdn.as_str(), 0)).await?.collect();
            if addresses.is_empty() || addresses.iter().any(|a| deny::is_denied(a.ip())) {
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
