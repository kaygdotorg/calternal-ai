//! Admission and delivery for anonymous outbound image and font requests.

use super::{
    admission::Admission,
    audit, cache, check_source, db,
    eligibility::{self, Trigger},
    frames::Source,
    media,
    quota::Quota,
};
use crate::{api::Shared, auth::Directory, netguard};
use calternal_types::Owner;
use reqwest::Url;
use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{runtime::Handle, sync::Semaphore};

const CAP: usize = 10_000_000;
const TIMEOUT: Duration = Duration::from_secs(10);
const QUEUE_TIME: Duration = Duration::from_secs(15);
const CONCURRENT: usize = 16;
/// Process-local work limits, sharded owner cache and shared space budgets.
pub(crate) struct Service {
    cache: cache::Cache,
    admission: Arc<Admission>,
    quota: Arc<Quota>,
    running: Semaphore,
    own: netguard::OriginAddresses,
    ranges: std::ffi::OsString,
    nat64: Option<std::ffi::OsString>,
    #[cfg(all(feature = "test-plugins", debug_assertions))]
    fixture: Option<Fixture>,
}

impl Default for Service {
    fn default() -> Self {
        Self {
            cache: cache::Cache::default(),
            admission: Arc::new(Admission::default()),
            quota: Arc::new(Quota::default()),
            running: Semaphore::new(CONCURRENT),
            own: Arc::new(arc_swap::ArcSwapOption::from_pointee(Vec::new())),
            ranges: std::env::var_os("CALTERNAL_CONTENT_FETCH_DENY_CIDRS").unwrap_or_default(),
            nat64: std::env::var_os("CALTERNAL_CONTENT_FETCH_NAT64_PREFIX"),
            #[cfg(all(feature = "test-plugins", debug_assertions))]
            fixture: Fixture::configured(),
        }
    }
}

impl Service {
    /// Reserve a Mail dial under the same per-space budget as media requests.
    pub(in crate::plugins) fn reserve(
        &self,
        space: calternal_types::SpaceId,
        bytes: u64,
    ) -> Result<super::admission::Reservation, calternal_types::Error> {
        self.admission.try_start(space, bytes)
    }

    /// Check the Mail destination with the live origin and interface deny rules.
    /// Enumerates interfaces off the async runtime, then resolves an exact FQDN.
    pub(in crate::plugins) async fn mail_addresses(
        &self,
        host: &str,
        port: u16,
    ) -> io::Result<Vec<std::net::SocketAddr>> {
        build_policy(
            self.ranges.clone(),
            self.nat64.clone(),
            false,
            self.own.clone(),
        )
        .await?
        .checked_addresses(host, port)
        .await
    }
}

/// Resolve the public origin at startup, then refresh once a minute. Keep the
/// last confirmed set on lookup failure. Initial failure holds Class B closed.
pub(crate) async fn configure<D: Directory>(state: &Shared<D>, origin: &str) {
    let service = &state.store.plugin_fetch;
    if service.ranges.is_empty() {
        tracing::warn!(
            "CALTERNAL_CONTENT_FETCH_DENY_CIDRS is empty; configure any additional NAT or hairpin server ranges"
        );
    }
    let Ok(origin) = Url::parse(origin) else {
        return;
    };
    let Some(host) = origin.host_str() else {
        return;
    };
    let host = host.to_owned();
    service.own.store(None);
    async fn refresh(host: &str, own: &netguard::OriginAddresses) {
        let raw = host.trim_matches(['[', ']']);
        let values = if let Ok(ip) = raw.parse::<std::net::IpAddr>() {
            vec![ip]
        } else {
            let name = if host.contains('.') && !host.ends_with('.') {
                format!("{host}.")
            } else {
                host.to_owned()
            };
            match tokio::time::timeout(
                Duration::from_secs(3),
                tokio::net::lookup_host((name.as_str(), 0)),
            )
            .await
            {
                Ok(Ok(addresses)) => addresses.map(|address| address.ip()).collect(),
                _ => return,
            }
        };
        if !values.is_empty() {
            own.store(Some(Arc::new(values)));
        }
    }
    refresh(&host, &service.own).await;
    let state = Arc::downgrade(state);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let Some(state) = state.upgrade() else {
                break;
            };
            refresh(&host, &state.store.plugin_fetch.own).await;
        }
    });
}

impl Service {
    /// Evict media for an erased owner. At most 4096 entries are examined.
    pub(crate) fn erase(&self, owner: Owner) {
        self.cache.erase(owner);
    }

    pub(super) fn get(&self, source: &Source, url: &Url) -> Option<Arc<cache::Entry>> {
        self.cache.get(key(source, url))
    }
}

fn key(source: &Source, url: &Url) -> cache::Key {
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    hash.update(source.plugin.as_bytes());
    hash.update(b"\0");
    hash.update(url.as_str().as_bytes());
    cache::Key {
        account: source.account,
        space: source.space,
        target: hash.finish().as_ref().try_into().expect("SHA-256 length"),
    }
}

fn refused() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "content fetch refused")
}

/// Queue at most the available work slots; no DNS or sockets on the caller.
pub(super) fn enqueue<D: Directory>(
    runtime: &Handle,
    state: &Shared<D>,
    source: Arc<Source>,
    urls: &[Url],
    trigger: Trigger,
) {
    let service = &state.store.plugin_fetch;
    for url in urls {
        let key = key(&source, url);
        let Some(entry) = service.cache.start(key) else {
            continue;
        };
        let Some(slot) = service.quota.enqueue(&source.plugin, source.space) else {
            service
                .cache
                .finish(key, &entry, Err(cache::Failure::Capacity));
            continue;
        };
        let (st, source, url) = (state.clone(), source.clone(), url.clone());
        runtime.spawn(async move {
            let _slot = slot;
            let service = &st.store.plugin_fetch;
            let asset = match tokio::time::timeout(QUEUE_TIME, service.running.acquire()).await {
                Ok(Ok(_permit)) => retrieve(&st, &source, url, trigger).await,
                _ => Err(cache::Failure::Capacity),
            };
            service.cache.finish(key, &entry, asset);
        });
    }
}

async fn authorized<D: Directory>(
    state: &Shared<D>,
    source: &Arc<Source>,
    trigger: Trigger,
) -> io::Result<()> {
    if !source.session.is_live_in(state).await {
        return Err(refused());
    }
    let (st, source) = (state.clone(), source.clone());
    crate::blocking::spawn(move || {
        let lifecycle = st.store.content().lifecycle();
        let _space = lifecycle
            .step(source.space.as_uuid())
            .map_err(|_| refused())?;
        let _account = lifecycle
            .step(source.account.as_uuid())
            .map_err(|_| refused())?;
        check_source(&st, &source).map_err(|_| refused())?;
        let preference = db::preference(st.store.appdata(), source.space, source.account)
            .map_err(|_| refused())?;
        eligibility::may_enqueue(preference, source.age, trigger)
            .then_some(())
            .ok_or_else(refused)
    })
    .await
    .map_err(io::Error::other)?
}

async fn begin_audit<D: Directory>(
    state: &Shared<D>,
    source: &Arc<Source>,
    host: &str,
) -> io::Result<i64> {
    let (st, source, host) = (state.clone(), source.clone(), host.to_owned());
    crate::blocking::spawn(move || {
        let lifecycle = st.store.content().lifecycle();
        let _space = lifecycle
            .step(source.space.as_uuid())
            .map_err(|_| refused())?;
        let _account = lifecycle
            .step(source.account.as_uuid())
            .map_err(|_| refused())?;
        audit::begin(
            st.store.appdata(),
            source.space,
            audit::Attempt {
                account: source.account,
                package: &source.plugin,
                revision: source.package_revision,
                approval: source.activation_generation,
                host: &host,
                port: 443,
            },
        )
        .map_err(|_| refused())
    })
    .await
    .map_err(io::Error::other)?
}

async fn finish_audit<D: Directory>(
    state: &Shared<D>,
    source: &Source,
    hop: (i64, Instant),
    outcome: audit::Outcome,
    bytes: u64,
) {
    let st = state.clone();
    let space = source.space;
    let _ = crate::blocking::spawn(move || {
        let lifecycle = st.store.content().lifecycle();
        let Ok(_space) = lifecycle.step(space.as_uuid()) else {
            return;
        };
        let _ = audit::finish(
            st.store.appdata(),
            space,
            hop.0,
            outcome,
            hop.1.elapsed(),
            bytes,
        );
    })
    .await;
}

/// Read local interfaces on a blocking worker once per source fetch.
/// Each dial builds its pinned TLS client on a blocking worker as well.
async fn build_policy(
    ranges: std::ffi::OsString,
    nat64: Option<std::ffi::OsString>,
    fixture: bool,
    own: netguard::OriginAddresses,
) -> io::Result<netguard::ContentPolicy> {
    crate::blocking::spawn(move || {
        let ranges = ranges.to_str().ok_or_else(refused)?;
        let nat64 = nat64
            .as_ref()
            .map(|value| value.to_str().ok_or_else(refused))
            .transpose()?;
        netguard::ContentPolicy::load(ranges, nat64, fixture, own)
    })
    .await
    .map_err(io::Error::other)?
}

async fn retrieve<D: Directory>(
    state: &Shared<D>,
    source: &Arc<Source>,
    url: Url,
    trigger: Trigger,
) -> Result<cache::Asset, cache::Failure> {
    let service = &state.store.plugin_fetch;
    #[cfg(all(feature = "test-plugins", debug_assertions))]
    let fixture = service.fixture.is_some();
    #[cfg(not(all(feature = "test-plugins", debug_assertions)))]
    let fixture = false;
    let policy = build_policy(
        service.ranges.clone(),
        service.nat64.clone(),
        fixture,
        service.own.clone(),
    )
    .await
    .map_err(|_| cache::Failure::Unavailable)?;
    let own_host = state
        .directory
        .http_auth()
        .and_then(|http| Url::parse(&http.config.origin_header()).ok());
    let mut hop = None;
    let mut host_slot = None;
    let mut failure = cache::Failure::Unavailable;
    let mut media_bytes = 0;
    let mut outcome = audit::Outcome::UpstreamFailure;
    let mut reservation = None;
    let result = tokio::time::timeout(TIMEOUT, async {
        let mut url = url;
        let mut remaining = 3;
        let response = loop {
            netguard::check_url(&url)?;
            let host = url.domain().ok_or_else(refused)?;
            if own_host.as_ref().and_then(Url::domain) == Some(host) {
                return Err(refused());
            }
            authorized(state, source, trigger).await?;
            if !service.quota.visit(&source.plugin, source.space, host) {
                failure = cache::Failure::Capacity;
                return Err(refused());
            }
            reservation = Some(
                service
                    .admission
                    .try_start(source.space, CAP as u64)
                    .map_err(|_| {
                        failure = cache::Failure::Capacity;
                        refused()
                    })?,
            );
            hop = Some((begin_audit(state, source, host).await?, Instant::now()));
            let checked = policy.resolve(host).await?;
            let Some(dial) = service.quota.dial(checked.ip()) else {
                if let Some(reservation) = reservation.take() {
                    reservation.finish(0);
                }
                failure = cache::Failure::Capacity;
                return Err(refused());
            };
            host_slot = Some(dial);
            let response = request(service, url.clone(), checked)
                .await?
                .send()
                .await
                .map_err(io::Error::other)?;
            match netguard::redirect_target(&url, &response, &mut remaining) {
                Ok(None) => break response,
                Ok(Some(next)) => {
                    drop(response);
                    host_slot.take();
                    if let Some(reservation) = reservation.take() {
                        reservation.finish(0);
                    }
                    if let Some(previous) = hop.take() {
                        finish_audit(state, source, previous, audit::Outcome::Redirect, 0).await;
                    }
                    url = next;
                }
                Err(error) => {
                    outcome = audit::Outcome::Refused;
                    return Err(error);
                }
            }
        };
        if response.status() != reqwest::StatusCode::OK {
            return Err(refused());
        }
        let headers = response.headers().clone();
        let bytes = netguard::read_bounded(response, CAP).await?;
        media_bytes = bytes.len() as u64;
        let Some(kind) = media::checked_type(&headers, &bytes) else {
            outcome = audit::Outcome::Refused;
            return Err(refused());
        };
        authorized(state, source, trigger).await?;
        outcome = audit::Outcome::Ready;
        Ok::<_, io::Error>(cache::Asset {
            kind,
            bytes: bytes.into(),
        })
    })
    .await;
    drop(host_slot);
    if let Some(last) = hop {
        finish_audit(state, source, last, outcome, media_bytes).await;
    }
    // Redirect bodies are discarded. Charge failed/unknown transfers their full reserve.
    if result.is_ok() && (media_bytes != 0 || matches!(outcome, audit::Outcome::Ready)) {
        if let Some(last) = reservation.take() {
            last.finish(media_bytes);
        }
    }
    result.ok().and_then(Result::ok).ok_or(failure)
}

async fn request(
    _service: &Service,
    url: Url,
    checked: std::net::SocketAddr,
) -> io::Result<reqwest::RequestBuilder> {
    #[cfg(all(feature = "test-plugins", debug_assertions))]
    if let Some(fixture) = &_service.fixture
        && url.domain() == Some("content-fetch.test")
    {
        let mut target = fixture.origin.clone();
        target.set_path(url.path());
        target.set_query(url.query());
        return Ok(fixture.client.get(target));
    }
    let host = url.domain().ok_or_else(refused)?.to_owned();
    let client = crate::blocking::spawn(move || netguard::content_client(&host, checked, TIMEOUT))
        .await
        .map_err(io::Error::other)?
        .map_err(io::Error::other)?;
    Ok(client.get(url))
}

#[cfg(all(feature = "test-plugins", debug_assertions))]
struct Fixture {
    origin: Url,
    client: reqwest::Client,
}

#[cfg(all(feature = "test-plugins", debug_assertions))]
impl Fixture {
    fn configured() -> Option<Self> {
        let origin =
            Url::parse(&std::env::var("CALTERNAL_TEST_CONTENT_FETCH_ORIGIN").ok()?).ok()?;
        if origin.scheme() != "http"
            || origin.host_str() != Some("127.0.0.1")
            || origin.port().is_none()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
            || !origin.username().is_empty()
            || origin.password().is_some()
        {
            return None;
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .http1_only()
            .pool_max_idle_per_host(0)
            .connect_timeout(Duration::from_secs(5))
            .timeout(TIMEOUT)
            .user_agent("calternal")
            .build()
            .ok()?;
        Some(Self { origin, client })
    }
}

#[cfg(test)]
mod client_tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn background_client_reports_configuration_errors() {
        assert!(
            build_policy("not-a-cidr".into(), None, false, Default::default())
                .await
                .is_err()
        );
        assert!(
            build_policy(Default::default(), None, false, Default::default())
                .await
                .is_ok()
        );
    }
}
