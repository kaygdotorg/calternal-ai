//! Admission and delivery for anonymous outbound image and font requests.

use super::{
    admission::Admission,
    audit, cache, check_source, db,
    eligibility::{self, Trigger},
    frames::Source,
    media,
};
use crate::{api::Shared, auth::Directory, netguard};
use calternal_types::Owner;
use reqwest::{Client, Url};
use std::{
    collections::{HashMap, VecDeque},
    io,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{runtime::Handle, sync::Semaphore};

const CAP: usize = 10_000_000;
const TIMEOUT: Duration = Duration::from_secs(10);
const QUEUE_TIME: Duration = Duration::from_secs(15);
const CONCURRENT: usize = 16;
const QUEUE: usize = 128;
const PER_PLUGIN: usize = 60;
const TRACKED_PLUGINS: usize = 512;
const PER_HOST: usize = 2;

struct Limits {
    plugins: HashMap<String, VecDeque<Instant>>,
    hosts: HashMap<String, usize>,
}

/// Process-local work limits, sharded owner cache and shared space budgets.
pub(crate) struct Service {
    cache: cache::Cache,
    admission: Arc<Admission>,
    queue: Arc<Semaphore>,
    running: Semaphore,
    limits: Mutex<Limits>,
    ranges: String,
    #[cfg(all(feature = "test-plugins", debug_assertions))]
    fixture: Option<Fixture>,
}

impl Default for Service {
    fn default() -> Self {
        Self {
            cache: cache::Cache::default(),
            admission: Arc::new(Admission::default()),
            queue: Arc::new(Semaphore::new(QUEUE)),
            running: Semaphore::new(CONCURRENT),
            limits: Mutex::new(Limits {
                plugins: HashMap::new(),
                hosts: HashMap::new(),
            }),
            ranges: std::env::var("CALTERNAL_CONTENT_FETCH_DENY_CIDRS").unwrap_or_default(),
            #[cfg(all(feature = "test-plugins", debug_assertions))]
            fixture: Fixture::configured(),
        }
    }
}

impl Service {
    /// Evict media for an erased owner. At most 4096 entries are examined.
    pub(crate) fn erase(&self, owner: Owner) {
        self.cache.erase(owner);
    }

    pub(super) fn get(&self, source: &Source, url: &Url) -> Option<Arc<cache::Entry>> {
        self.cache.get(key(source, url))
    }

    fn plugin_slot(&self, plugin: &str) -> io::Result<()> {
        let mut limits = self
            .limits
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let now = Instant::now();
        limits.plugins.retain(|_, starts| {
            while starts
                .front()
                .is_some_and(|at| now.duration_since(*at) >= Duration::from_secs(60))
            {
                starts.pop_front();
            }
            !starts.is_empty()
        });
        if limits.plugins.len() >= TRACKED_PLUGINS && !limits.plugins.contains_key(plugin) {
            return Err(refused());
        }
        let starts = limits.plugins.entry(plugin.into()).or_default();
        if starts.len() >= PER_PLUGIN {
            return Err(refused());
        }
        starts.push_back(now);
        Ok(())
    }

    fn host_slot(&self, host: &str) -> io::Result<HostSlot<'_>> {
        let mut limits = self
            .limits
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let active = limits.hosts.entry(host.into()).or_default();
        if *active >= PER_HOST {
            return Err(refused());
        }
        *active += 1;
        Ok(HostSlot {
            service: self,
            host: host.into(),
        })
    }
}

struct HostSlot<'a> {
    service: &'a Service,
    host: String,
}

impl Drop for HostSlot<'_> {
    fn drop(&mut self) {
        let mut limits = self
            .service
            .limits
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(active) = limits.hosts.get_mut(&self.host) {
            *active -= 1;
            if *active == 0 {
                limits.hosts.remove(&self.host);
            }
        }
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
        let Ok(slot) = service.queue.clone().try_acquire_owned() else {
            break;
        };
        let key = key(&source, url);
        let Some(entry) = service.cache.start(key) else {
            continue;
        };
        let (st, source, url) = (state.clone(), source.clone(), url.clone());
        runtime.spawn(async move {
            let _slot = slot;
            let service = &st.store.plugin_fetch;
            let asset = match tokio::time::timeout(QUEUE_TIME, service.running.acquire()).await {
                Ok(Ok(_permit)) => retrieve(&st, &source, url, trigger).await,
                _ => None,
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

async fn retrieve<D: Directory>(
    state: &Shared<D>,
    source: &Arc<Source>,
    url: Url,
    trigger: Trigger,
) -> Option<cache::Asset> {
    let service = &state.store.plugin_fetch;
    #[cfg(all(feature = "test-plugins", debug_assertions))]
    let fixture = service.fixture.is_some();
    #[cfg(not(all(feature = "test-plugins", debug_assertions)))]
    let fixture = false;
    let client = netguard::content_client(&service.ranges, TIMEOUT, fixture).ok()?;
    let own_host = state
        .directory
        .http_auth()
        .and_then(|http| Url::parse(&http.config.origin_header()).ok());
    let mut hop = None;
    let mut host_slot = None;
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
            service.plugin_slot(&source.plugin)?;
            reservation = Some(
                service
                    .admission
                    .try_start(source.space, CAP as u64)
                    .map_err(|_| refused())?,
            );
            host_slot = Some(service.host_slot(host)?);
            hop = Some((begin_audit(state, source, host).await?, Instant::now()));
            let response = request(service, &client, url.clone())
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
    result.ok()?.ok()
}

fn request(_service: &Service, client: &Client, url: Url) -> reqwest::RequestBuilder {
    #[cfg(all(feature = "test-plugins", debug_assertions))]
    if let Some(fixture) = &_service.fixture
        && url.domain() == Some("content-fetch.test")
    {
        let mut target = fixture.origin.clone();
        target.set_path(url.path());
        target.set_query(url.query());
        return fixture.client.get(target);
    }
    client.get(url)
}

#[cfg(all(feature = "test-plugins", debug_assertions))]
struct Fixture {
    origin: Url,
    client: Client,
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
        let client = Client::builder()
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
