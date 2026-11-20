//! The one call path: context, call table, authorize, audit, receipts.
//!
//! Every call goes `dispatch` → `authorize` → audit row → handler. There is no
//! other way into a handler. The call table ([`TABLE`]) is the one place an
//! operation's name, handler and the check it needs are declared.
use crate::host::{Space, fail};
use calternal_types::{AccountId, Error, SpaceId};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

/// Who is asking, through which plugin, in which space, as which request.
/// Every handler takes it, from the first handler on.
#[derive(Debug, Clone)]
pub struct Ctx {
    /// The space the call acts in. Handlers receive the space itself already
    /// locked, so none reads this yet; it is what a handler would pass on to a
    /// call it makes in turn (step 8).
    #[allow(dead_code)]
    pub space: SpaceId,
    /// The acting person.
    pub person: AccountId,
    /// The plugin acting (`host` for the shell's own screens).
    pub plugin: String,
    /// The request ID, derived by the host ([`request_id`]).
    pub request: String,
    /// The acting person's role in the space, filled by `authorize` once the call is allowed.
    pub role: String,
}

/// A handler: context, the space (locked: this is the space's single writer), the input.
pub type Handler = fn(&Ctx, &mut Space, &Value) -> Result<Value, Error>;

/// What `authorize` must establish before the handler runs.
#[derive(Debug, Clone, Copy)]
pub enum Need {
    /// The person is a manager or owner and the plugin is the host's own shell.
    Manage,
    /// The acting plugin holds the write grant for the kind named by `input.kind`.
    WriteKind,
    /// The acting plugin holds the write grant for the kind of the file named by `input.file`.
    WriteFile,
    /// The acting plugin owns or may write the kind named by the query (`input.kind` or a collection).
    ReadKind,
    /// The acting plugin may read the kind of the file named by `input.file`.
    ReadFile,
}

/// One row of the call table.
pub struct Entry {
    /// The operation's name.
    pub name: &'static str,
    /// What must hold before it runs.
    pub need: Need,
    /// Whether it writes (writes keep a receipt; retries return it).
    pub writes: bool,
    /// The handler.
    pub handler: Handler,
}

/// The call table: every operation the host serves. Seven handlers.
pub const TABLE: &[Entry] = &[
    Entry {
        name: "plugins.install",
        need: Need::Manage,
        writes: true,
        handler: crate::files::install,
    },
    Entry {
        name: "files.create",
        need: Need::WriteKind,
        writes: true,
        handler: crate::files::create,
    },
    Entry {
        name: "files.set",
        need: Need::WriteFile,
        writes: true,
        handler: crate::files::set,
    },
    Entry {
        name: "record.append",
        need: Need::WriteFile,
        writes: true,
        handler: crate::records::append,
    },
    Entry {
        name: "record.delete",
        need: Need::WriteFile,
        writes: true,
        handler: crate::records::delete,
    },
    Entry {
        name: "record.list",
        need: Need::ReadFile,
        writes: false,
        handler: crate::records::list,
    },
    Entry {
        name: "query",
        need: Need::ReadKind,
        writes: false,
        handler: crate::query::query,
    },
];

/// Derive the request ID (owner question A22, recommended: the host derives it;
/// authors never write one). It is a hash of who, through what, which call,
/// the exact input, and the client's `attempt` key for this gesture. Without an
/// attempt key there is nothing stable to derive from, so a fresh random ID is
/// used and a retry is a new request. Cost: one hash over the input.
pub fn request_id(
    space: SpaceId,
    person: AccountId,
    plugin: &str,
    name: &str,
    input: &Value,
    attempt: Option<&str>,
) -> String {
    let Some(attempt) = attempt else {
        return format!("r-{}", uuid::Uuid::new_v4().simple());
    };
    let mut h = blake3::Hasher::new();
    for part in [
        space.to_string().as_str(),
        &person.to_string(),
        plugin,
        name,
        &input.to_string(),
        attempt,
    ] {
        h.update(&(part.len() as u64).to_le_bytes());
        h.update(part.as_bytes());
    }
    format!("d-{}", &h.finalize().to_hex()[..32])
}

/// Run one call: authorize, audit, receipt (a retry is answered from it), handler. The only entry to a handler.
///
/// # Errors
/// `not_found` for an unknown operation, `forbidden` from `authorize`, or the handler's error.
pub fn dispatch(
    mut ctx: Ctx,
    space: &mut Space,
    name: &str,
    input: &Value,
) -> Result<Value, Error> {
    let entry = TABLE
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| Error::not_found("no such operation"))?;
    if let Err(e) = authorize(&mut ctx, space, entry, input) {
        audit(&space.db, &ctx, name, Some(&e.message))?;
        return Err(e);
    }
    if !entry.writes {
        // A read has no commit of its own to join, so its audit row is a commit (and a flush).
        audit(&space.db, &ctx, name, None)?;
        return (entry.handler)(&ctx, space, input);
    }
    let stored: Option<(String, String)> = space
        .db
        .query_row(
            "SELECT name, result FROM receipts WHERE request = ?1",
            [&ctx.request],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(fail)?;
    if let Some((n, result)) = stored {
        audit(
            &space.db,
            &ctx,
            name,
            Some("retry answered from its receipt"),
        )?;
        return if n == name {
            serde_json::from_str(&result).map_err(fail)
        } else {
            Err(Error::request_reused())
        };
    }
    // A write's audit row is inserted by `receipt`, inside the handler's own commit,
    // so a write costs one ledger flush, not two. A write that fails before its
    // commit is audited here instead.
    let result = (entry.handler)(&ctx, space, input);
    if let Err(e) = &result {
        audit(&space.db, &ctx, name, Some(&e.message))?;
    }
    result
}

/// One audit row: who, through what, which call and request, allowed or the refusal.
/// The `allowed` column is 1 for a call that passed `authorize` (its outcome is in `reason`).
fn audit(
    db: &rusqlite::Connection,
    ctx: &Ctx,
    name: &str,
    reason: Option<&str>,
) -> Result<(), Error> {
    let allowed = !ctx.role.is_empty();
    db.execute(
        "INSERT INTO audit(at, person, plugin, name, request, allowed, reason) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![calternal_types::now_ms(), ctx.person.to_string(), ctx.plugin, name, ctx.request, allowed, reason],
    )
    .map_err(fail)?;
    Ok(())
}

/// The one check, run for every call: the person is a member, and the plugin's
/// grant covers what the call touches. A stub: grants come from declarations,
/// roles from the `members` table; there is no sharing, consent or reach set yet.
fn authorize(ctx: &mut Ctx, space: &Space, entry: &Entry, input: &Value) -> Result<(), Error> {
    let (role, _) = space
        .role(ctx.person)?
        .ok_or_else(|| Error::forbidden("not a member of this space"))?;
    let writer = matches!(role.as_str(), "editor" | "manager" | "owner");
    let kind_of_file = |space: &Space| -> Result<String, Error> {
        let id = input
            .get("file")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::input_field("/file", "a file ID"))?;
        Ok(space.file(id)?.kind)
    };
    let (kind, write) = match entry.need {
        Need::Manage => {
            return if ctx.plugin == "host" && matches!(role.as_str(), "manager" | "owner") {
                ctx.role = role;
                Ok(())
            } else {
                Err(Error::forbidden(
                    "installing needs a manager, from the host's own screen",
                ))
            };
        }
        Need::WriteKind => (
            input
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            true,
        ),
        Need::WriteFile => (kind_of_file(space)?, true),
        Need::ReadFile => (kind_of_file(space)?, false),
        Need::ReadKind => (crate::query::asked_kind(space, &ctx.plugin, input)?, false),
    };
    if write && !writer {
        return Err(Error::forbidden("your role cannot change files here"));
    }
    let plugin = space
        .plugins
        .get(&ctx.plugin)
        .ok_or_else(|| Error::forbidden("plugin not installed"))?;
    let granted =
        plugin.grant.write.contains(&kind) || (!write && plugin.kinds.contains_key(&kind));
    if !granted {
        return Err(Error::forbidden(format!(
            "{} has no grant for {kind}",
            plugin.id
        )));
    }
    ctx.role = role;
    Ok(())
}

/// Insert the call's receipt and its audit row inside the handler's own commit,
/// so a retry after a crash either finds the whole write or none of it, and the
/// audit costs no flush of its own. Cost: two inserts.
///
/// # Errors
/// `internal` on SQL failure.
pub fn receipt(tx: &Transaction<'_>, ctx: &Ctx, name: &str, result: &Value) -> Result<(), Error> {
    tx.execute(
        "INSERT INTO receipts(request, name, result, at) VALUES (?1, ?2, ?3, ?4)",
        params![
            ctx.request,
            name,
            result.to_string(),
            calternal_types::now_ms()
        ],
    )
    .map_err(fail)?;
    audit(tx, ctx, name, None)
}

/// Write one stamp: who wrote which revision (or record) through what, when, with which hash.
///
/// # Errors
/// `internal` on SQL failure.
pub fn stamp(
    tx: &Transaction<'_>,
    ctx: &Ctx,
    file: &str,
    rev: i64,
    anchor: Option<&str>,
    op: &str,
    hash: &[u8],
) -> Result<i64, Error> {
    tx.execute(
        "INSERT INTO stamps(file, rev, anchor, op, person, plugin, request, at, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![file, rev, anchor, op, ctx.person.to_string(), ctx.plugin, ctx.request, calternal_types::now_ms(), hash],
    )
    .map_err(fail)?;
    Ok(tx.last_insert_rowid())
}
