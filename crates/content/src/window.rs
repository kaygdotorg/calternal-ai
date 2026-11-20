//! Ordered windows over committed, audience-visible rows.

mod kinds;
mod read;
mod replay;
pub use kinds::{WindowField, WindowKindOrder, WindowOrder, WindowScope};

use crate::{
    Content, Error, Result, SpaceId,
    types::{internal, missing},
};
use rusqlite::{Connection, types::Value as Key};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::sync::OnceLock;
use uuid::Uuid;

#[cfg(test)]
mod tests;

const CATALOGUE: &str = include_str!("../../../contract/host-catalogue-v2.json");

#[derive(Deserialize)]
struct Limits {
    materialized_rows_per_window: u64,
    index_jump_skips_per_window: u64,
}

#[derive(Deserialize)]
struct WindowLimits {
    max_offset: u64,
    max_request_span: u64,
    max_encoded_bytes: usize,
}

#[derive(Deserialize)]
struct QueryLimits {
    window: WindowLimits,
    live_limits: Limits,
    scope_limit_refusal: ScopeRefusal,
}

#[derive(Deserialize)]
struct ScopeRefusal {
    code: String,
    message: String,
}

fn limits() -> &'static QueryLimits {
    #[derive(Deserialize)]
    struct Catalogue {
        query: QueryLimits,
    }
    static LIMITS: OnceLock<QueryLimits> = OnceLock::new();
    LIMITS.get_or_init(|| {
        serde_json::from_str::<Catalogue>(CATALOGUE)
            .expect("sealed window limits")
            .query
    })
}

pub(crate) fn projection_error(error: rusqlite::Error) -> Error {
    if let rusqlite::Error::SqliteFailure(_, Some(message)) = &error
        && message == &limits().scope_limit_refusal.code
    {
        let refusal = &limits().scope_limit_refusal;
        return Error::new(
            calternal_types::ErrorCode::EffectiveScopeLimitExceeded,
            &refusal.message,
        );
    }
    internal(error)
}

/// The catalogue's window envelope. Resolve the Bind, audience and parent
/// edge on the host before using this request.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WindowRequest {
    /// A half-open range from the selected end of the Bind.
    Window {
        /// Bind identifier, anchor and requested bounds.
        request: WindowRange,
    },
}

/// Bounds and identity of one request within the host's active view.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowRange {
    /// Bind ID in the active description.
    pub bind: String,
    /// Inclusive offset from the requested anchor.
    pub from: u64,
    /// Exclusive offset from the requested anchor.
    pub to: u64,
    /// Host-issued parent ref, when targeting a nested Bind.
    #[serde(default, deserialize_with = "present")]
    pub parent: Option<String>,
    /// Omitted for an ordinary absolute start request.
    #[serde(default, deserialize_with = "present")]
    pub anchor: Option<WindowAnchor>,
}

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// Endpoints used for absolute and end-relative windows.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowAnchor {
    /// Count distance from the first authorized row.
    #[default]
    Start,
    /// Count distance from the last authorized row. Exact count policy required.
    End,
}

/// The current viewer classification, assigned by the host from live authority.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowAudience {
    /// Member in their own space.
    OwnSpaceMember,
    /// Member in another space view.
    Member,
    /// Recipient of an explicit item share.
    Guest,
    /// Anonymous visitor to an explicit public share.
    Public,
}

impl WindowAudience {
    fn guest(self) -> bool {
        matches!(self, Self::Guest | Self::Public)
    }
}

/// Count policy obtained from the checked Bind and scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowCountMode {
    /// Omit the count. Required for guest views without explicit disclosure.
    None,
    /// Disclose the exact total. For guests the host must verify explicit permission.
    Exact,
    /// Use an audience-certified member estimate if it is still current.
    Estimate {
        /// Certified estimate for this scope.
        value: u64,
        /// Opaque result version for which the estimate was certified by the host.
        result_version: String,
    },
}

/// Trusted host resolution of a Bind. Recompute this on each authorization check,
/// including the session, live grants, permitted order, projection and count policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowAccess {
    /// Scope authorized for this Bind and viewer.
    pub scope: WindowScope,
    /// Audience authorized for the active view.
    pub audience: WindowAudience,
    /// Effective authorized count mode.
    pub count: WindowCountMode,
    /// Host-generated random identity for this active view and authority revision.
    /// Rotate it whenever the checked grant or view changes.
    pub view: Uuid,
}

/// A plan refusal conforming to the catalogue's typed unsupported-query response.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowPlanReason {
    /// The covering jump would walk beyond the skip budget.
    SkipLimit,
    /// The requested scope, count, bounds or continuation has no qualified plan.
    UnsupportedPlan,
    /// The plan exceeds the materialization or encoded-window budget.
    MaterializationLimit,
    /// This audience and plan do not have exact count authority.
    EndUnavailable,
}

/// Typed catalogue 422 reply with a machine-readable plan reason.
#[derive(Debug, Serialize)]
pub struct WindowPlanError {
    status: u16,
    code: &'static str,
    /// Reason the host could not produce a window.
    pub reason: WindowPlanReason,
}

/// The catalogue reply after the host checks current authority.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WindowReply {
    /// An authorized result or empty window.
    Result(WindowResult),
    /// The checked projection is catching up. Carries no rows or count.
    Loading(LoadingWindow),
    /// A request that could not be served by a bounded plan.
    PlanError(WindowPlanError),
}

impl From<WindowPlanReason> for WindowReply {
    fn from(reason: WindowPlanReason) -> Self {
        Self::PlanError(WindowPlanError {
            status: 422,
            code: "unsupported_query_plan",
            reason,
        })
    }
}

/// A caller-safe loading reply for a currently authorized window.
#[derive(Debug, Serialize)]
pub struct LoadingWindow {
    bind: String,
    state: &'static str,
    result_version: Option<()>,
    window_ref: Option<()>,
    offset: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    rows: [(); 0],
    audience: WindowAudience,
    anchor: WindowAnchor,
}

/// A sealed WindowResult plus the private seek boundary owned by the host.
/// Serialize the result for the active view only. Keep it server-side to authorize
/// a matching contiguous continuation.
#[derive(Debug, Serialize)]
pub struct WindowResult {
    bind: String,
    state: &'static str,
    result_version: String,
    window_ref: Uuid,
    offset: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    rows: Vec<WindowRow>,
    audience: WindowAudience,
    anchor: WindowAnchor,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<WindowCount>,
    #[serde(skip)]
    boundary: Option<(Key, String)>,
}

impl WindowResult {
    /// Resolve the opaque selector in this server-owned result. The caller
    /// must still reauthorize the active view and item before using the ID.
    /// Scans at most one page of selectors and performs no storage I/O.
    pub fn item_for_row(&self, selector: &str) -> Option<Uuid> {
        self.rows
            .iter()
            .find(|row| row.id == selector)
            .map(|row| row.item)
    }
}

#[derive(Debug, Serialize)]
struct WindowRow {
    id: String,
    fields: Value,
    #[serde(skip)]
    item: Uuid,
}

#[derive(Debug, Serialize)]
struct WindowCount {
    mode: &'static str,
    value: u64,
}

impl Content {
    /// Read one authorized window from the published journal generation.
    /// The callback runs before storage access and before delivery. An
    /// unpublished generation returns loading without rows or a count. A
    /// ready read uses one sidecar/source snapshot, the journal and source
    /// freshness fences, and one head probe per returned row. No flushes.
    pub fn window(
        &self,
        space: SpaceId,
        request: &WindowRequest,
        previous: Option<&WindowResult>,
        authorize: impl Fn() -> Result<WindowAccess>,
    ) -> Result<WindowReply> {
        let access = authorize()?;
        let WindowRequest::Window { request } = request;
        let reauthorize = || {
            if authorize()? == access {
                Ok(())
            } else {
                Err(missing())
            }
        };
        if let Some(reason) = bad_plan(request, &access, previous) {
            reauthorize()?;
            return Ok(reason.into());
        }
        let _step = self.lifecycle.step(space.as_uuid())?;
        let loading = || {
            reauthorize()?;
            self.wake_window(space, true);
            Ok(WindowReply::Loading(LoadingWindow {
                bind: request.bind.clone(),
                state: "loading",
                result_version: None,
                window_ref: None,
                offset: request.from,
                parent: request.parent.clone(),
                rows: [],
                audience: access.audience,
                anchor: request.anchor.unwrap_or_default(),
            }))
        };
        let source = crate::db::space_path(&self.roots.appdata, space);
        if !source.try_exists().map_err(internal)? {
            let reply = evaluate(
                None,
                &access.scope.key(),
                &read::Scope::default(),
                request,
                &access,
                previous,
            )?;
            reauthorize()?;
            return Ok(reply);
        }
        if !replay::path(&source).try_exists().map_err(internal)? {
            return loading();
        }
        let conn = replay::reader(&source)?;
        let Some(journal) = calternal_store::ProjectionJournal::open(&self.roots.appdata, space)
            .map_err(internal)?
        else {
            return loading();
        };
        let tx = conn.unchecked_transaction().map_err(internal)?;
        let version: i64 = tx
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(internal)?;
        if version == 0 {
            return loading();
        }
        let Some(publication) = replay::Publication::read(&tx)? else {
            return loading();
        };
        if journal.head().map_err(internal)? != publication.head
            || !publication.matches_source(&tx)?
        {
            return loading();
        }
        let bound: String = tx
            .query_row(
                "SELECT value FROM source.binding WHERE id='space'",
                [],
                |row| row.get(0),
            )
            .map_err(internal)?;
        if bound != space.to_string() {
            return Err(missing());
        }
        if let WindowScope::Children { parent, .. } = access.scope
            && !kinds::parent_live(&tx, parent)?
        {
            return Err(missing());
        }
        let scope = access.scope.key();
        let snapshot = read::scope(&tx, &scope)?;
        let reply = evaluate(Some(&tx), &scope, &snapshot, request, &access, previous)?;
        tx.commit().map_err(internal)?;
        reauthorize()?;
        // Read the journal after the source fence. A write committed before
        // that fence is either still pinned by its outbox or already journalled.
        if !publication.matches_source(&conn)?
            || journal.head().map_err(internal)? != publication.head
        {
            return loading();
        }
        self.lifecycle.check(space.as_uuid())?;
        Ok(reply)
    }
}

fn bad_plan(
    request: &WindowRange,
    access: &WindowAccess,
    previous: Option<&WindowResult>,
) -> Option<WindowPlanReason> {
    use WindowPlanReason::*;
    let anchor = request.anchor.unwrap_or_default();
    let exact = access.count == WindowCountMode::Exact;
    if access.scope == WindowScope::Unqualified
        || (access.audience.guest() && !matches!(access.scope, WindowScope::NotesCollection(_)))
    {
        return Some(UnsupportedPlan);
    }
    if anchor == WindowAnchor::End && !exact {
        return Some(EndUnavailable);
    }
    let window = &limits().window;
    let valid_range = request.from < request.to
        && request.to <= 9_007_199_254_740_991
        && request.to - request.from <= window.max_request_span;
    if (access.audience.guest() && matches!(access.count, WindowCountMode::Estimate { .. }))
        || !valid_range
        || request.parent.as_deref() != access.scope.parent_ref()
        || request
            .parent
            .as_ref()
            .is_some_and(|parent| parent.is_empty() || parent.chars().count() > 128)
        || request.bind.is_empty()
        || request.bind.chars().count() > 64
    {
        return Some(UnsupportedPlan);
    }
    if anchor == WindowAnchor::Start
        && request.from > window.max_offset
        && (request.anchor.is_none() || exact || previous.is_none())
    {
        return Some(UnsupportedPlan);
    }
    None
}

fn result_version(
    scope: &(String, &str),
    snapshot: &read::Scope,
    request: &WindowRange,
    access: &WindowAccess,
) -> String {
    let source = format!(
        "{}:{}:{}:{}:{}:{}:{}:{}",
        access.view,
        scope.0,
        scope.1,
        snapshot.version,
        access.scope.reversed(),
        access.scope.parent_ref().unwrap_or_default(),
        request.bind,
        serde_json::to_string(access.scope.fields()).expect("encode approved projection")
    );
    blake3::hash(source.as_bytes()).to_hex().to_string()
}

fn evaluate(
    conn: Option<&Connection>,
    scope: &(String, &str),
    snapshot: &read::Scope,
    request: &WindowRange,
    access: &WindowAccess,
    previous: Option<&WindowResult>,
) -> Result<WindowReply> {
    use WindowPlanReason::*;
    let version = result_version(scope, snapshot, request, access);
    let anchor = request.anchor.unwrap_or_default();
    let width = request.to - request.from;
    let deep = anchor == WindowAnchor::Start && request.from > limits().window.max_offset;
    let count = match &access.count {
        WindowCountMode::None => None,
        WindowCountMode::Exact => Some(WindowCount {
            mode: "exact",
            value: snapshot.total,
        }),
        WindowCountMode::Estimate { .. } if deep => None,
        WindowCountMode::Estimate {
            value,
            result_version,
        } if !access.audience.guest() && *result_version == version => {
            if snapshot.total == 0 {
                Some(WindowCount {
                    mode: "exact",
                    value: 0,
                })
            } else {
                Some(WindowCount {
                    mode: "estimate",
                    value: *value,
                })
            }
        }
        WindowCountMode::Estimate { .. } => return Ok(UnsupportedPlan.into()),
    };
    let boundary = previous
        .filter(|prior| {
            anchor == WindowAnchor::Start
                && prior.anchor == WindowAnchor::Start
                && prior.result_version == version
                && prior.audience == access.audience
                && prior.offset + prior.rows.len() as u64 == request.from
        })
        .and_then(|prior| prior.boundary.as_ref());
    if deep && boundary.is_none() {
        return Ok(UnsupportedPlan.into());
    }
    let exact = count.as_ref().is_some_and(|count| count.mode == "exact")
        && access.count == WindowCountMode::Exact;
    if let (Some(conn), WindowScope::NotesCollection(_) | WindowScope::NotesCollectionNewest(_)) =
        (conn, &access.scope)
        && kinds::collection_newest(conn, &scope.0)?
            != matches!(access.scope, WindowScope::NotesCollectionNewest(_))
    {
        return Ok(UnsupportedPlan.into());
    }
    let title_field = if let Some((kind, order)) = access.scope.registered_order() {
        let Some(conn) = conn else {
            return Ok(UnsupportedPlan.into());
        };
        let Some(title) =
            kinds::registered_title(conn, kind, &order.field, request.parent.is_some())?
        else {
            return Ok(UnsupportedPlan.into());
        };
        title
    } else {
        "title".into()
    };
    let plan = match read::Plan::new(
        snapshot.total,
        request.from,
        width,
        anchor,
        exact,
        boundary,
        access.scope.reversed(),
    ) {
        Ok(plan) => plan,
        Err(reason) => return Ok(reason.into()),
    };
    let mut keys = Vec::new();
    let mut rows = Vec::new();
    if let Some(conn) = conn {
        let expected = plan.take as usize;
        keys = read::keys(conn, scope, plan)?;
        if keys.len() != expected {
            return Err(internal(
                "window scope count disagrees with its covering index",
            ));
        }
        rows.reserve(keys.len());
        let mut row_bytes = 0;
        let mut projection = read::Projection::new(conn, &title_field, access.scope.fields())?;
        for (_, id) in &keys {
            let row = WindowRow {
                id: blake3::hash(format!("{}:{id}", access.view).as_bytes())
                    .to_hex()
                    .to_string(),
                fields: projection.fields(id)?,
                item: Uuid::parse_str(id).map_err(internal)?,
            };
            row_bytes += serde_json::to_vec(&row).map_err(internal)?.len();
            if row_bytes > limits().window.max_encoded_bytes {
                return Ok(MaterializationLimit.into());
            }
            rows.push(row);
        }
    }
    let result = WindowResult {
        bind: request.bind.clone(),
        state: if snapshot.total == 0 && request.from == 0 {
            "empty"
        } else {
            "ready"
        },
        result_version: version,
        window_ref: Uuid::new_v4(),
        offset: request.from,
        parent: request.parent.clone(),
        rows,
        audience: access.audience,
        anchor,
        count,
        boundary: keys.into_iter().last(),
    };
    if serde_json::to_vec(&result).map_err(internal)?.len() > limits().window.max_encoded_bytes {
        return Ok(MaterializationLimit.into());
    }
    Ok(WindowReply::Result(result))
}
