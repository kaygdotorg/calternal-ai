//! Installed v2 package operations on the enrolled space carrier.
//!
//! A person invokes a declared operation of a package that is active in the
//! space. The host wakes or reuses that package's placement, mints a ticket
//! for the exact actor, operation capability and grant revision, stamps the
//! author context, and answers each item callback through the item engine on
//! a blocking worker. Writes are fenced on the package's durable activation
//! stamp inside the content writer; reads and deliveries recheck it.
use super::{
    awake::{Inbox, Wake},
    *,
};
use crate::plugins::item_engine::{
    CapturedInput, ItemAccess, ItemAuthority, ItemCheck, ItemEngine, ItemStages,
};
use calternal_plugin_host::ManifestV2;
use calternal_types::ItemRef;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

/// Reclaim period for idle staged saves (catalogue: within 30 seconds).
pub const RECLAIM: Duration = Duration::from_secs(30);

/// Awake placements and staged-save pools for activated v2 packages, keyed
/// by space and exact package pin. Lookups are lock-free snapshot reads on
/// the request path. Only a first wake, a first staged save in a space or a
/// disable copies the map. Entries are bounded by spaces times active
/// packages (128 per installation) and leave on disable.
#[derive(Default)]
pub struct PackagePlacements {
    wakes: arc_swap::ArcSwap<HashMap<(SpaceId, PackagePin), Arc<Wake>>>,
    stages: arc_swap::ArcSwap<HashMap<SpaceId, Arc<ItemStages>>>,
}

impl std::fmt::Debug for PackagePlacements {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PackagePlacements")
            .field("placements", &self.wakes.load().len())
            .field("staging_spaces", &self.stages.load().len())
            .finish()
    }
}

impl PackagePlacements {
    fn wake(&self, space: SpaceId, pin: &PackagePin) -> Arc<Wake> {
        let key = (space, pin.clone());
        if let Some(wake) = self.wakes.load().get(&key) {
            return wake.clone();
        }
        let mut found = None;
        self.wakes.rcu(|current| {
            let mut next = HashMap::clone(current);
            found = Some(next.entry(key.clone()).or_default().clone());
            next
        });
        found.expect("rcu runs at least once")
    }

    /// The space's shared staged-save pool. One pool per space keeps the
    /// space-wide staging bounds across packages and invocations. The first
    /// use starts that pool's reclaim timer, which drops idle provisional
    /// bytes within [`RECLAIM`] and ends when the pool is dropped. Call
    /// inside the Tokio runtime.
    pub(crate) fn stages(&self, space: SpaceId) -> Arc<ItemStages> {
        if let Some(pool) = self.stages.load().get(&space) {
            return pool.clone();
        }
        let mut found = None;
        let mut created = false;
        self.stages.rcu(|current| {
            let mut next = HashMap::clone(current);
            created = !next.contains_key(&space);
            found = Some(
                next.entry(space)
                    .or_insert_with(|| Arc::new(ItemStages::new(space)))
                    .clone(),
            );
            next
        });
        let pool = found.expect("rcu runs at least once");
        if created {
            let owner = Arc::downgrade(&pool);
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(RECLAIM);
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    tick.tick().await;
                    let Some(pool) = owner.upgrade() else { break };
                    pool.reclaim();
                }
            });
        }
        pool
    }

    /// Fence every awake placement of `package` in `space` after its
    /// disable committed. In-flight writes already fail at the durable
    /// activation fence; this also ends their tickets and the carrier.
    /// Cost: one map copy and one short async lock per placement.
    pub(crate) async fn retire(&self, space: SpaceId, package: &str) {
        let mut removed = Vec::new();
        self.wakes.rcu(|current| {
            removed.clear();
            let mut next = HashMap::clone(current);
            next.retain(|(owner, pin), wake| {
                let keep = *owner != space || pin.package != package;
                if !keep {
                    removed.push(wake.clone());
                }
                keep
            });
            next
        });
        for wake in removed {
            if let Some(awake) = wake.awake.lock().await.take() {
                awake.fence.close();
            }
        }
    }
}

/// Dispatch one declared v2 package operation for a person. `input` is the
/// operation's author input plus the envelope's `space_id`, which only
/// selects the space whose live authority is then checked. `captured` is the
/// exact text a checked composer or editor route bound to the operation's
/// declared `write_origins.host_input` field; it must equal that input field
/// and is scoped to the operation's target item. The generic operation route
/// passes `None`, so its attested `input` sources are refused. Cost on a warm
/// placement: one capability read, one activation read, one ticket mint and
/// the operation's callbacks. A cold wake also reads the pinned worker once.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn invoke<D: Directory>(
    state: Shared<D>,
    actor: Identity,
    session: Session,
    request_id: &str,
    operation: &str,
    mut input: calternal_core::JsonObject,
    machines: Option<Arc<MachineService>>,
    captured: Option<String>,
) -> Result<Value, Error> {
    let catalog = state.registry.snapshot();
    let (package, required) = {
        let (declared, operation) = catalog.v2_operation(operation).ok_or_else(missing)?;
        let required = serde_json::from_value::<calternal_core::ManifestCapability>(
            operation["required_capability"].clone(),
        )
        .ok()
        .and_then(crate::plugins::host_capability)
        .ok_or_else(missing)?;
        (
            catalog.v2_package(declared.id()).ok_or_else(missing)?,
            required,
        )
    };
    let space = input
        .remove("space_id")
        .and_then(|value| value.as_str().and_then(|id| SpaceId::parse(id).ok()))
        .ok_or_else(|| Error::invalid_input("space_id is required"))?;
    let input = Value::Object(input.into_iter().collect());
    package
        .check_operation_input(operation, &input)
        .map_err(|_| Error::invalid_input("operation input does not match its schema"))?;
    let captured = captured
        .map(|text| capture(&package, operation, &input, text))
        .transpose()?;
    let installation = match &session {
        Session::Static => None,
        Session::Installation {
            account,
            installation,
        } if *account == actor.id => Some(*installation),
        _ => {
            return Err(Error::forbidden(
                "plugin operations require a person session",
            ));
        }
    };
    if !session.is_live(&state.directory).await
        || authorization::capabilities(&state, actor.id, space)
            .await?
            .is_none_or(|rights| {
                !rights.contains(Capability::PluginsUse)
                    || !rights.contains(Capability::SpaceRead)
                    || !rights.contains(required)
            })
    {
        return Err(missing());
    }
    let worker = package.declaration()["worker"].clone();
    let pin = PackagePin {
        package: package.id().to_owned(),
        digest: worker["sha256"]
            .as_str()
            .ok_or_else(|| Error::unavailable("the package declares no worker"))?
            .to_owned(),
    };
    let content = state.store.shared_content();
    let key = crate::store::plugin_key(package.id());
    let activation = {
        let (content, key) = (content.clone(), key.clone());
        crate::blocking::spawn(move || content.control(space, &key))
            .await
            .map_err(|_| unavailable())??
            .filter(|value| value["enabled"] == true)
            .ok_or_else(|| Error::not_found("plugin is not enabled in this space"))?
    };
    let wake = state.registry.packages().wake(space, &pin);
    let awake = loop {
        let mut current = wake.awake.lock().await;
        if let Some(awake) = current.as_ref() {
            if let Some(mut drained) = awake.drain_waiter() {
                drop(current);
                tokio::time::timeout(
                    crate::plugins::worker::WORKER_START_TIMEOUT,
                    drained.wait_for(|done| *done),
                )
                .await
                .map_err(|_| unavailable())?
                .map_err(|_| unavailable())?;
                continue;
            }
            if awake.sender.is_live() {
                break awake.clone();
            }
        }
        let service = machines
            .clone()
            .or_else(|| state.store.machines())
            .ok_or_else(unavailable)?;
        let appdata = state.store.appdata().to_owned();
        let id = package.id().to_owned();
        let source = crate::blocking::spawn(move || {
            crate::plugins::package_store::installed_worker(&appdata, &id, &worker)
        })
        .await
        .map_err(|_| unavailable())?
        .map_err(|_| unavailable())?;
        let awake = attach_worker(
            &state,
            service,
            actor.id,
            space,
            source,
            &pin,
            None,
            Arc::downgrade(&wake),
        )
        .await?;
        *current = Some(awake.clone());
        break awake;
    };
    let scope = Scope {
        actor: actor.id,
        installation,
        package: pin,
        epoch: awake.epoch,
        operation: operation.to_owned(),
        target: None,
        required,
        author: AuthorContext {
            request_id: request_id.to_owned(),
            actor: actor.id,
            authored_at: crate::store::now_rfc3339(),
        },
    };
    let stages = state.registry.packages().stages(space);
    awake
        .invoke_with(scope, move |sender, inbox, ticket, cancel| {
            let engine = engine(Bound {
                captured,
                content,
                space,
                package: package.clone(),
                key,
                activation,
                stages,
                ticket: ticket.clone(),
                cancel: cancel.clone(),
            });
            run(sender, inbox, ticket, cancel, engine, input, package)
        })
        .await
}

/// Bind a checked capture to the declared field and the operation's target.
fn capture(
    package: &ManifestV2,
    operation: &str,
    input: &Value,
    text: String,
) -> Result<CapturedInput, Error> {
    let declared = package.declaration()["operations"]
        .as_array()
        .and_then(|all| all.iter().find(|entry| entry["id"] == operation))
        .ok_or_else(missing)?;
    let field = declared["write_origins"]["host_input"]
        .as_str()
        .ok_or_else(|| Error::forbidden("the operation declares no captured input"))?;
    if input[field].as_str() != Some(text.as_str()) {
        return Err(Error::forbidden(
            "the capture does not match the operation input",
        ));
    }
    let target = declared["target"]["field"]
        .as_str()
        .map(|field| {
            input[field]
                .as_str()
                .map(|item| ItemRef(item.to_owned()))
                .ok_or_else(|| Error::invalid_input("the operation target is missing"))
        })
        .transpose()?;
    Ok(CapturedInput { text, target })
}

struct Bound {
    captured: Option<CapturedInput>,
    content: Arc<calternal_content::Content>,
    space: SpaceId,
    package: Arc<ManifestV2>,
    key: String,
    activation: Value,
    stages: Arc<ItemStages>,
    ticket: Arc<Ticket>,
    cancel: Arc<AtomicBool>,
}

/// Bind the operation's own kind: its target, its created kind, or the
/// package's only owned kind. Without one, item callbacks are refused.
fn engine(bound: Bound) -> Option<ItemEngine> {
    let scope = &bound.ticket.scope;
    let declaration = bound.package.declaration();
    let operation = declaration["operations"]
        .as_array()?
        .iter()
        .find(|entry| entry["id"] == scope.operation.as_str())?;
    let owned = declaration["kinds"].as_array()?;
    let kind = operation["target"]["kind"]
        .as_str()
        .or_else(|| operation["creates"].as_str())
        .or_else(|| match owned.as_slice() {
            [only] => only["id"].as_str(),
            _ => None,
        })?
        .to_owned();
    let authority: ItemAuthority = {
        let (ticket, cancel) = (bound.ticket.clone(), bound.cancel.clone());
        let (content, space, key) = (bound.content.clone(), bound.space, bound.key.clone());
        let activation = bound.activation.clone();
        Arc::new(move |access: ItemAccess| {
            if cancel.load(Ordering::Acquire) {
                return Err(missing());
            }
            ticket.require_live(Instant::now())?;
            match access.check {
                // The content writer checks the activation stamp at commit,
                // under the lock that serializes disable. Never reenter here.
                ItemCheck::Create | ItemCheck::Write | ItemCheck::Metadata => Ok(()),
                ItemCheck::Head | ItemCheck::Range | ItemCheck::Deliver => {
                    if content.control(space, &key)?.as_ref() == Some(&activation) {
                        Ok(())
                    } else {
                        Err(missing())
                    }
                }
            }
        })
    };
    let engine = ItemEngine::new(
        bound.content,
        bound.space,
        scope.actor,
        bound.package,
        &scope.operation,
        &kind,
        calternal_types::RequestId(scope.author.request_id.clone()),
        authority,
    )
    .ok()?
    .with_activation(bound.key, bound.activation);
    let engine = match bound.captured {
        Some(captured) => engine.with_captured_input(captured),
        None => engine,
    };
    engine.with_stages(bound.stages).ok()
}

async fn run(
    channel: SpaceSender,
    mut inbox: Inbox,
    ticket: Arc<Ticket>,
    cancel: Arc<AtomicBool>,
    mut engine: Option<ItemEngine>,
    input: Value,
    package: Arc<ManifestV2>,
) -> Result<Value, Error> {
    let pin = ticket.scope.package.clone();
    let live = || {
        let (ticket, cancel) = (ticket.clone(), cancel.clone());
        move || !cancel.load(Ordering::Acquire) && ticket.require_live(Instant::now()).is_ok()
    };
    let send_error = |_| {
        ticket
            .require_live(Instant::now())
            .err()
            .unwrap_or_else(unavailable)
    };
    channel
        .send_checked_wait(
            &pin,
            ticket.id(),
            SpaceMessage::Invoke {
                operation: ticket.scope.operation.clone(),
                input,
                deadline_ms: ticket.deadline_ms(),
                author_context: ticket.scope.author.clone(),
            },
            live(),
        )
        .await
        .map_err(send_error)?;
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let delivery = tokio::select! {
            _ = tick.tick() => {
                ticket.require_live(Instant::now())?;
                continue;
            }
            item = inbox.recv() => item.ok_or_else(unavailable)?,
        };
        ticket.authorize_at(delivery.frame(), Instant::now())?;
        let reply = match &delivery.frame().message {
            SpaceMessage::Call {
                callback_id,
                method,
                input,
            } => (*callback_id, *method, input.clone()),
            SpaceMessage::Complete { result: Ok(data) } => {
                let data = data.clone();
                drop(delivery);
                package
                    .check_operation_output(&ticket.scope.operation, &data)
                    .map_err(|_| outside())?;
                if let Some(engine) = engine.take() {
                    let checked = data.clone();
                    crate::blocking::spawn(move || engine.created(&checked))
                        .await
                        .map_err(|_| unavailable())??;
                }
                ticket.require_live(Instant::now())?;
                if !channel.is_live() {
                    return Err(unavailable());
                }
                return Ok(data);
            }
            SpaceMessage::Complete { result: Err(_) } => return Err(unavailable()),
            _ => return Err(outside()),
        };
        drop(delivery);
        let (callback_id, method, input) = reply;
        let result = match engine.take() {
            None => Err(ErrorCode::NotFound),
            Some(mut bound) => {
                let (bound, result) = crate::blocking::spawn(move || {
                    let result = bound.call(method.as_str(), &input);
                    (bound, result)
                })
                .await
                .map_err(|_| unavailable())?;
                engine = Some(bound);
                result.map_err(|error| error.code)
            }
        };
        ticket.require_live(Instant::now())?;
        channel
            .send_checked_wait(
                &pin,
                ticket.id(),
                SpaceMessage::Reply {
                    callback_id,
                    result,
                },
                live(),
            )
            .await
            .map_err(send_error)?;
    }
}

fn outside() -> Error {
    Error::new(
        ErrorCode::Forbidden,
        "plugin response is outside the admitted operation",
    )
}
