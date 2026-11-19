//! Opt-in browser push for committed thread events, with bounded delivery.

pub(crate) mod db;
mod transport;

use calternal_core::{
    AccountInputList, JsonObject, PushDevice, PushDevices, PushPayload, PushQueued, PushRemoved,
    PushStatus, PushSubscribeInput, PushUnsubscribeInput,
};
use calternal_types::{AccountId, Capability, Error, InstallationId, SpaceId};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{api::Shared, auth::Directory, plugins::Session};

const PENDING: usize = 128;
const MAX_AGE: Duration = Duration::from_secs(60);
const CONCURRENT: usize = 4;

pub(crate) struct Service {
    queue: mpsc::Sender<Notice>,
    public_key: String,
}

#[derive(Clone)]
enum Target {
    Thread {
        space: SpaceId,
        thread: Uuid,
        pending: Option<String>,
    },
    Test {
        device: InstallationId,
        generation: String,
    },
}

struct Notice {
    account: AccountId,
    target: Target,
    at: Instant,
}

impl Service {
    pub(crate) fn thread(
        &self,
        account: AccountId,
        space: &str,
        thread: &str,
        pending: Option<String>,
    ) {
        let (Ok(space), Ok(thread)) = (SpaceId::parse(space), Uuid::parse_str(thread)) else {
            return;
        };
        let _ = self.queue.try_send(Notice {
            account,
            target: Target::Thread {
                space,
                thread,
                pending,
            },
            at: Instant::now(),
        });
    }
}

/// Start the bounded web push worker for a protected installation. Opens
/// server-owned key material on a blocking worker; no outbound startup calls.
pub async fn configure<D: Directory>(state: &Shared<D>) -> Result<(), Error> {
    let Some(auth) = state.directory.http_auth() else {
        return Ok(());
    };
    let origin = auth.config.origin_header();
    let fixture = cfg!(all(feature = "test-plugins", debug_assertions))
        && std::env::var_os("CALTERNAL_TEST_WEB_PUSH_LOCAL").is_some();
    if !fixture && !origin.starts_with("https://") {
        return Ok(());
    }
    let root = state.store.appdata().to_owned();
    let transport = tokio::task::spawn_blocking(move || transport::Transport::open(&root, origin))
        .await
        .map_err(|_| Error::unavailable("Web push startup failed."))??;
    let (queue, incoming) = mpsc::channel(PENDING);
    state
        .jobs
        .push
        .set(Service {
            queue,
            public_key: transport.public.clone(),
        })
        .map_err(|_| Error::internal("Web push was already configured."))?;
    let weak = Arc::downgrade(state);
    tokio::spawn(async move {
        use futures_util::StreamExt as _;
        use futures_util::stream;
        let stream = stream::unfold(incoming, |mut receiver| async {
            receiver.recv().await.map(|notice| (notice, receiver))
        });
        let transport = Arc::new(transport);
        stream
            .for_each_concurrent(CONCURRENT, |notice| {
                let transport = transport.clone();
                let state = weak.upgrade();
                async move {
                    if let Some(state) = state {
                        deliver(&state, transport, notice, fixture).await;
                    }
                }
            })
            .await;
    });
    let weak = Arc::downgrade(state);
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(24 * 60 * 60));
        loop {
            timer.tick().await;
            let Some(state) = weak.upgrade() else { break };
            let _ = tokio::task::spawn_blocking(move || {
                db::prune_expired(state.store.appdata(), &state.store.content().lifecycle());
            })
            .await;
        }
    });
    Ok(())
}

pub(crate) async fn operation<D: Directory>(
    state: Shared<D>,
    account: AccountId,
    session: Session,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let Session::Installation {
        account: owner,
        installation,
    } = session
    else {
        return Err(Error::forbidden(
            "Sign in with a browser to manage web push.",
        ));
    };
    if owner != account {
        return Err(Error::forbidden(
            "Sign in with a browser to manage web push.",
        ));
    }
    let auth = state
        .directory
        .http_auth()
        .ok_or_else(|| Error::unavailable("Web push is unavailable."))?;
    let installations = auth
        .auth
        .installations(account)
        .await
        .map_err(|_| Error::unavailable("Device inventory is unavailable."))?;
    if !installations.iter().any(|d| {
        d.id == installation && matches!(d.kind, calternal_auth::InstallationKind::Browser)
    }) {
        return Err(Error::forbidden(
            "Sign in with a browser to manage web push.",
        ));
    }
    let op = op.to_owned();
    crate::blocking::spawn(move || {
        let push = state
            .jobs
            .push
            .get()
            .ok_or_else(|| Error::conflict("Web push requires an HTTPS server."))?;
        let _owner = state.store.content().lifecycle().step(account.as_uuid())?;
        let root = state.store.appdata();
        let input = json!(input);
        match op.as_str() {
            "push.subscribe" | "push.devices" => {
                if op == "push.subscribe" {
                    let input: PushSubscribeInput = decode(input)?;
                    transport::validate(&input.endpoint, &input.p256dh, &input.auth)?;
                    db::register(root, account, installation, &input)?;
                } else {
                    let _: AccountInputList = decode(input)?;
                }
                let devices = db::devices(root, account)?
                    .into_iter()
                    .filter_map(|(id, expires_at_ms)| {
                        let device = installations.iter().find(|d| d.id.to_string() == id)?;
                        Some(PushDevice {
                            device_id: id,
                            name: device.name.clone(),
                            current: device.id == installation,
                            expires_at_ms,
                        })
                    })
                    .collect();
                Ok(json!(PushDevices {
                    application_server_key: push.public_key.clone(),
                    devices
                }))
            }
            "push.unsubscribe" => {
                let input: PushUnsubscribeInput = decode(input)?;
                let id = InstallationId::parse(&input.device_id)
                    .map_err(|_| Error::invalid_input("Invalid device ID."))?;
                Ok(json!(PushRemoved {
                    removed: db::remove(root, account, id, None)?
                }))
            }
            "push.test" => {
                let _: AccountInputList = decode(input)?;
                let slot = push.queue.try_reserve().map_err(|error| match error {
                    tokio::sync::mpsc::error::TrySendError::Full(()) => {
                        Error::rate_limited("Web push queue is busy.")
                            .with_retry_after(Duration::from_secs(1))
                    }
                    tokio::sync::mpsc::error::TrySendError::Closed(()) => {
                        Error::unavailable("Web push is unavailable.")
                    }
                })?;
                let generation = db::admit_test(root, account, installation)?;
                slot.send(Notice {
                    account,
                    target: Target::Test {
                        device: installation,
                        generation,
                    },
                    at: Instant::now(),
                });
                Ok(json!(PushQueued { queued: true }))
            }
            _ => Err(Error::not_found("unknown web push operation")),
        }
    })
    .await
    .map_err(|_| Error::unavailable("Web push storage is unavailable."))?
}

fn decode<T: serde::de::DeserializeOwned>(input: Value) -> Result<T, Error> {
    serde_json::from_value(input).map_err(|_| Error::invalid_input("Invalid web push input."))
}

fn can_deliver<D: Directory>(state: &Shared<D>, account: AccountId, target: &Target) -> bool {
    let Target::Thread { space, pending, .. } = target else {
        return true;
    };
    state
        .directory
        .capabilities(account, *space)
        .is_some_and(|caps| {
            caps.contains(Capability::SpaceRead) && caps.contains(Capability::PluginsUse)
        })
        && pending.as_ref().is_none_or(|request| {
            state
                .jobs
                .push_request_pending(account, request)
                .unwrap_or(false)
        })
}

fn payload<D: Directory>(state: &Shared<D>, notice: &Notice) -> Option<PushPayload> {
    match &notice.target {
        Target::Thread {
            space,
            thread,
            pending,
        } => {
            if !can_deliver(state, notice.account, &notice.target) {
                return None;
            }
            let metadata = state
                .store
                .content()
                .live_metadata(*space, "threads", *thread)
                .ok()?;
            let title = metadata["title"].as_str()?;
            if title.is_empty() || title.len() > 256 || title.chars().any(char::is_control) {
                return None;
            }
            Some(PushPayload {
                title: title.into(),
                status: if pending.is_some() {
                    PushStatus::NeedsYou
                } else {
                    PushStatus::Finished
                },
                space_id: Some(space.to_string()),
                thread_id: Some(thread.to_string()),
            })
        }
        Target::Test { .. } => Some(PushPayload {
            title: "Calternal".into(),
            status: PushStatus::Test,
            space_id: None,
            thread_id: None,
        }),
    }
}

async fn deliver<D: Directory>(
    state: &Shared<D>,
    transport: Arc<transport::Transport>,
    notice: Notice,
    fixture: bool,
) {
    if notice.at.elapsed() > MAX_AGE {
        return;
    }
    let account = notice.account;
    let at = notice.at;
    let target = notice.target.clone();
    let st = state.clone();
    let Ok(Some(prepared)) = tokio::task::spawn_blocking(move || {
        let _owner = st
            .store
            .content()
            .lifecycle()
            .step(account.as_uuid())
            .ok()?;
        let payload = payload(&st, &notice)?;
        let root = st.store.appdata();
        let subscriptions = db::subscriptions(root, account).ok()?;
        let targets = subscriptions
            .into_iter()
            .filter(|sub| match &notice.target {
                Target::Thread { .. } => true,
                Target::Test { device, generation } => {
                    sub.device == *device && sub.generation == *generation
                }
            });
        if fixture {
            if targets.count() > 0
                && let Ok(bytes) = serde_json::to_vec(&payload)
            {
                let _ = calternal_fsio::write_atomic_mode(
                    &root.join("web-push-fixture.json"),
                    &bytes,
                    0o600,
                );
            }
            return None;
        }
        Some(
            targets
                .filter_map(|sub| {
                    let request = transport.message(&sub, &payload).ok()?;
                    Some((sub.device, sub.generation, request))
                })
                .collect::<Vec<_>>(),
        )
    })
    .await
    else {
        return;
    };
    for (device, generation, request) in prepared {
        if at.elapsed() > MAX_AGE {
            return;
        }
        if !Session::installation(account, device)
            .is_live(&state.directory)
            .await
        {
            continue;
        }
        // Fence queued work against account erasure and against a subscription
        // being removed or replaced while the notification was prepared.
        let st = state.clone();
        let expected = generation.clone();
        let target = target.clone();
        let current = tokio::task::spawn_blocking(move || {
            let Ok(_owner) = st.store.content().lifecycle().step(account.as_uuid()) else {
                return false;
            };
            can_deliver(&st, account, &target)
                && db::is_current(st.store.appdata(), account, device, &expected).unwrap_or(false)
        })
        .await
        .unwrap_or(false);
        if current
            && let Ok(response) = request.send().await
            && transport::gone(response.status())
        {
            remove(state, account, device, generation).await;
        }
    }
}

async fn remove<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    device: InstallationId,
    generation: String,
) {
    let st = state.clone();
    let _ = tokio::task::spawn_blocking(move || {
        let _owner = st.store.content().lifecycle().step(account.as_uuid())?;
        db::remove(st.store.appdata(), account, device, Some(&generation))
    })
    .await;
}
