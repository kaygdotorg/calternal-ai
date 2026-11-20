//! Host-registered scalar orders and child relations for plugin kinds.

use crate::{Content, Error, Result, SpaceId, types::internal};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use uuid::Uuid;

/// The named row set selected by a checked Bind. Raw renderer inputs must
/// never select this scope or an ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowScope {
    /// Live Notes, newest first. Requires space-level read authority.
    SpaceNotes,
    /// An explicit live Notes collection, ordered by visible title then ID.
    NotesCollection(Uuid),
    /// Explicit member collection provisioned in newest-first order.
    NotesCollectionNewest(Uuid),
    /// Notes in Trash, newest move first. Requires the host's Trash authority.
    NotesTrash,
    /// Authorized live rows of a registered claimed-text or built-in kind.
    Kind {
        /// Manifest kind identity, resolved by the host.
        kind: String,
        /// Qualified field and display direction.
        order: WindowOrder,
        /// Additional fields from the host's checked member projection.
        fields: Vec<WindowField>,
    },
    /// A child edge already checked against the entire live parent ancestry.
    Children {
        /// Registered child kind whose stored relation is the edge constraint.
        kind: String,
        /// Parent item matched by the host's checked edge.
        parent: Uuid,
        /// Opaque renderer token issued for this edge, Bind, and view.
        reference: String,
        /// Qualified field and display direction.
        order: WindowOrder,
    },
    /// A checked Bind whose source or ordering has no stored projection.
    Unqualified,
}

impl WindowScope {
    pub(super) fn key(&self) -> (String, &'static str) {
        match self {
            Self::SpaceNotes => ("notes".into(), "space"),
            Self::NotesCollection(id) => (id.to_string(), "explicit"),
            Self::NotesCollectionNewest(id) => (id.to_string(), "explicit_newest"),
            Self::NotesTrash => ("notes".into(), "trash"),
            Self::Kind { kind, .. } => (kind.clone(), "kind"),
            Self::Children { kind, parent, .. } => (format!("{kind}:{parent}"), "child"),
            Self::Unqualified => unreachable!("only qualified plans have a scope key"),
        }
    }
}

/// A typed additional member field resolved against an installed declaration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum WindowField {
    /// Project a named metadata boolean.
    Boolean(String),
    /// Project a named RFC 3339 metadata timestamp.
    Timestamp(String),
}

/// One order admitted against the host's approved and registered field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowOrder {
    /// Resolved indexed field name. The host reserves `updated` for the item commit timestamp.
    pub field: String,
    /// Whether the display orders this field descending.
    pub descending: bool,
}

impl WindowScope {
    pub(super) fn fields(&self) -> &[WindowField] {
        match self {
            Self::Kind { fields, .. } => fields,
            _ => &[],
        }
    }

    pub(super) fn parent_ref(&self) -> Option<&str> {
        match self {
            Self::Children { reference, .. } => Some(reference),
            _ => None,
        }
    }

    pub(super) fn registered_order(&self) -> Option<(&str, &WindowOrder)> {
        match self {
            Self::Kind { kind, order, .. } | Self::Children { kind, order, .. } => {
                Some((kind, order))
            }
            _ => None,
        }
    }

    pub(super) fn reversed(&self) -> bool {
        self.registered_order()
            .is_some_and(|(_, order)| order.descending != (order.field == "updated"))
    }
}

/// One qualified ordering and optional child relation from a checked kind
/// declaration. The host authorizes actual indexed fields and relation edges
/// before registering the projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowKindOrder {
    /// Metadata string field exposed as the generated title.
    pub title_field: String,
    /// Canonical scalar metadata field or the built-in `updated` timestamp.
    pub sort_field: String,
    /// Canonical metadata field carrying the stable parent UUID, if declared.
    pub parent_field: Option<String>,
}

fn field(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

impl Content {
    /// Provision the one covering order of a live explicit Notes collection.
    /// The host must establish that a member-only order is appropriate before
    /// replacing the guest-compatible order. Records the definition in one FULL
    /// commit. The window stays loading while its new generation is published.
    pub fn configure_notes_collection_order(
        &self,
        space: SpaceId,
        collection: Uuid,
        newest: bool,
    ) -> Result<()> {
        self.with_space(space, |space| {
            let collection = collection.to_string();
            let live: bool = space
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM items WHERE id=?1 AND kind='collections'
                 AND json_extract(metadata,'$.item_kind')='notes'
                 AND json_extract(metadata,'$.standard_key') IS NULL
                 AND substr(path,1,7)<>'.trash/' AND mat_state NOT IN ('purging','purged'))",
                    [&collection],
                    |row| row.get(0),
                )
                .map_err(internal)?;
            if !live {
                return Err(crate::types::missing());
            }
            if collection_newest(&space.conn, &collection)? == newest {
                return Ok(());
            }
            let tx = space
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(internal)?;
            if newest {
                tx.execute(
                    "INSERT INTO window_collection_newest VALUES(?1)",
                    [&collection],
                )
                .map_err(internal)?;
            } else {
                tx.execute(
                    "DELETE FROM window_collection_newest WHERE collection=?1",
                    [&collection],
                )
                .map_err(internal)?;
            }
            tx.commit().map_err(internal)
        })?;
        self.wake_window(space, false);
        Ok(())
    }

    /// Install an approved scalar ordering and optional parent relation on a
    /// registered claimed-text or built-in kind. The existing order remains
    /// current until the definition commit. The background worker publishes
    /// the new ordering using metadata only. Unchanged definitions are one indexed check and
    /// zero flushes. Schedule a changed definition off the async request path.
    pub fn configure_window_kind(
        &self,
        space: SpaceId,
        kind: &str,
        order: &WindowKindOrder,
    ) -> Result<()> {
        if !field(&order.title_field)
            || !field(&order.sort_field)
            || order
                .parent_field
                .as_deref()
                .is_some_and(|parent| !field(parent))
        {
            return Err(Error::invalid_input("invalid window index field"));
        }
        self.with_space(space, |space| {
            let old = space.conn.query_row(
                "SELECT title_field,sort_field,parent_field FROM window_kind_orders WHERE kind=?1",
                [kind],
                |row| Ok(WindowKindOrder { title_field: row.get(0)?, sort_field: row.get(1)?, parent_field: row.get(2)? }),
            ).optional().map_err(internal)?
                .ok_or_else(|| Error::invalid_input("kind does not qualify for a window order"))?;
            if old == *order { return Ok(()); }
            let tx = space.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(internal)?;
            tx.execute(
                "UPDATE window_kind_orders SET title_field=?2,sort_field=?3,parent_field=?4 WHERE kind=?1",
                params![kind, order.title_field, order.sort_field, order.parent_field],
            ).map_err(internal)?;
            tx.commit().map_err(internal)
        })?;
        self.wake_window(space, false);
        Ok(())
    }
}

pub(super) fn registered_title(
    conn: &Connection,
    kind: &str,
    sort: &str,
    parent: bool,
) -> Result<Option<String>> {
    conn.prepare_cached(
        "SELECT title_field FROM window_kind_orders WHERE kind=?1 AND sort_field=?2
         AND (NOT ?3 OR parent_field IS NOT NULL)",
    )
    .map_err(internal)?
    .query_row(params![kind, sort, parent], |row| row.get(0))
    .optional()
    .map_err(internal)
}

pub(super) fn collection_newest(conn: &Connection, collection: &str) -> Result<bool> {
    conn.prepare_cached("SELECT EXISTS(SELECT 1 FROM window_collection_newest WHERE collection=?1)")
        .map_err(internal)?
        .query_row([collection], |row| row.get(0))
        .map_err(internal)
}

pub(super) fn parent_live(conn: &Connection, parent: Uuid) -> Result<bool> {
    conn.prepare_cached(
        "SELECT EXISTS(SELECT 1 FROM items WHERE id=?1 AND substr(path,1,7)<>'.trash/'
         AND mat_state NOT IN ('purging','purged'))",
    )
    .map_err(internal)?
    .query_row([parent.to_string()], |row| row.get(0))
    .map_err(internal)
}
