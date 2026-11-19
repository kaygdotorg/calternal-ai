//! Disposable per-member layout storage and atomic protected inverse edits.
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use uuid::Uuid;

const SCHEMA: &str = "
CREATE TABLE spike_layout_item(space TEXT, id TEXT, PRIMARY KEY(space,id)) STRICT, WITHOUT ROWID;
CREATE TABLE spike_layout_revision (
  space TEXT, actor TEXT, app TEXT, rev INTEGER NOT NULL,
  PRIMARY KEY(space,actor,app)
) STRICT, WITHOUT ROWID;
CREATE TABLE spike_layout_sections (
  space TEXT, actor TEXT, app TEXT, id TEXT, title TEXT NOT NULL, position INTEGER NOT NULL,
  PRIMARY KEY(space,actor,app,id)
) STRICT, WITHOUT ROWID;
CREATE INDEX spike_layout_sections_order ON spike_layout_sections(space,actor,app,position,id);
CREATE TABLE spike_placement (
  space TEXT, actor TEXT, app TEXT, id TEXT, section TEXT NOT NULL, position INTEGER NOT NULL,
  PRIMARY KEY(space,actor,app,id)
) STRICT, WITHOUT ROWID;
CREATE INDEX spike_placement_order ON spike_placement(space,actor,app,section,position,id);
CREATE TABLE spike_undo (
  token TEXT PRIMARY KEY, space TEXT NOT NULL, actor TEXT NOT NULL, app TEXT NOT NULL,
  rev INTEGER NOT NULL, inverse TEXT NOT NULL
) STRICT, WITHOUT ROWID;";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct Section(pub(super) String, pub(super) String, pub(super) i64);
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct Entry(pub(super) String, pub(super) String, pub(super) i64);

#[derive(Deserialize, Serialize)]
enum Inverse {
    DropSection(String),
    SetSection(Section, Vec<Entry>),
    SetEntries(Vec<Entry>),
    DropEntry(String),
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub(super) enum Edit<'a> {
    Create(&'a str, &'a str),
    Rename(&'a str, &'a str),
    Remove(&'a str),
    To(&'a str, &'a str),
    Up(&'a str),
    Down(&'a str),
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub(super) enum Cut {
    None,
    Rollback,
    BeforeCommit,
    AfterCommit,
    DuringReply,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct Scope<'a> {
    pub(super) space: &'a str,
    pub(super) actor: &'a str,
    pub(super) app: &'a str,
}

type Parent<'a> = (Scope<'a>, Uuid, i64, String);

#[derive(Default)]
pub(super) struct LayoutWindows<'a> {
    issued: HashMap<Uuid, Parent<'a>>,
}

impl<'a> LayoutWindows<'a> {
    pub(super) fn sections(
        &mut self,
        conn: &mut Connection,
        scope: Scope<'a>,
        view: Uuid,
        offset: i64,
        size: i64,
    ) -> Option<Vec<(Section, Uuid)>> {
        let tx = conn.transaction().unwrap();
        if !scope.live(&tx) {
            return None;
        }
        let pinned = scope.revision(&tx);
        Some(
            scope
                .sections(&tx, offset, size)
                .into_iter()
                .map(|section| {
                    let token = Uuid::new_v4();
                    self.issued
                        .insert(token, (scope, view, pinned, section.0.clone()));
                    (section, token)
                })
                .collect(),
        )
    }

    pub(super) fn placements(
        &self,
        conn: &mut Connection,
        scope: Scope<'_>,
        view: Uuid,
        token: Uuid,
        range: std::ops::Range<i64>,
    ) -> Option<Vec<Entry>> {
        let (issued_scope, issued_view, revision, section) = self.issued.get(&token)?;
        if scope != *issued_scope || view != *issued_view {
            return None;
        }
        scope.window(
            conn,
            section,
            *revision,
            range.start,
            range.end - range.start,
        )
    }
}

pub(super) fn open(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.busy_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    conn.pragma_update(None, "synchronous", "FULL").unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    conn
}

impl Scope<'_> {
    fn live(self, conn: &Connection) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM space_account_roles WHERE space_id=?1 AND account_id=?2)",
            params![self.space, self.actor],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn revision(self, conn: &Connection) -> i64 {
        conn.query_row(
            "SELECT rev FROM spike_layout_revision WHERE space=?1 AND actor=?2 AND app=?3",
            params![self.space, self.actor, self.app],
            |r| r.get(0),
        )
        .optional()
        .unwrap()
        .unwrap_or(0)
    }

    fn advance(self, conn: &Connection, rev: i64) {
        conn.execute(
            "INSERT INTO spike_layout_revision VALUES(?1,?2,?3,?4)
             ON CONFLICT(space,actor,app) DO UPDATE SET rev=excluded.rev",
            params![self.space, self.actor, self.app, rev],
        )
        .unwrap();
    }

    fn section(self, conn: &Connection, id: &str) -> Option<Section> {
        conn.query_row(
            "SELECT id,title,position FROM spike_layout_sections WHERE space=?1 AND actor=?2 AND app=?3 AND id=?4",
            params![self.space, self.actor, self.app, id],
            |r| Ok(Section(r.get(0)?, r.get(1)?, r.get(2)?)),
        ).optional().unwrap()
    }

    fn entry(self, conn: &Connection, id: &str) -> Option<Entry> {
        conn.query_row(
            "SELECT id,section,position FROM spike_placement WHERE space=?1 AND actor=?2 AND app=?3 AND id=?4",
            params![self.space, self.actor, self.app, id],
            |r| Ok(Entry(r.get(0)?, r.get(1)?, r.get(2)?)),
        ).optional().unwrap()
    }

    fn set_section(self, conn: &Connection, row: &Section) {
        conn.execute(
            "INSERT INTO spike_layout_sections VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(space,actor,app,id) DO UPDATE SET title=excluded.title,position=excluded.position",
            params![self.space, self.actor, self.app, row.0, row.1, row.2],
        ).unwrap();
    }

    fn set_entry(self, conn: &Connection, row: &Entry) {
        conn.execute(
            "INSERT INTO spike_placement VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(space,actor,app,id) DO UPDATE SET section=excluded.section,position=excluded.position",
            params![self.space, self.actor, self.app, row.0, row.1, row.2],
        ).unwrap();
    }

    fn remove_section(self, conn: &Connection, id: &str) {
        conn.execute(
            "DELETE FROM spike_placement WHERE space=?1 AND actor=?2 AND app=?3 AND section=?4",
            params![self.space, self.actor, self.app, id],
        )
        .unwrap();
        conn.execute(
            "DELETE FROM spike_layout_sections WHERE space=?1 AND actor=?2 AND app=?3 AND id=?4",
            params![self.space, self.actor, self.app, id],
        )
        .unwrap();
    }

    fn rows(self, conn: &Connection, section: &str, offset: i64, size: i64) -> Vec<Entry> {
        conn.prepare_cached(
            "SELECT id,section,position FROM spike_placement
             WHERE space=?1 AND actor=?2 AND app=?3 AND section=?4
             ORDER BY position,id LIMIT ?5 OFFSET ?6",
        )
        .unwrap()
        .query_map(
            params![self.space, self.actor, self.app, section, size, offset],
            |r| Ok(Entry(r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
        .map(Result::unwrap)
        .collect()
    }

    fn window(
        self,
        conn: &mut Connection,
        section: &str,
        pinned: i64,
        offset: i64,
        size: i64,
    ) -> Option<Vec<Entry>> {
        let tx = conn.transaction().unwrap();
        if !self.live(&tx) || self.revision(&tx) != pinned || self.section(&tx, section).is_none() {
            return None;
        }
        Some(self.rows(&tx, section, offset, size))
    }

    fn max_position(self, conn: &Connection, section: Option<&str>) -> i64 {
        match section {
            Some(section) => conn.query_row(
                "SELECT coalesce(max(position),0) FROM spike_placement
                 WHERE space=?1 AND actor=?2 AND app=?3 AND section=?4",
                params![self.space, self.actor, self.app, section],
                |r| r.get(0),
            ),
            None => conn.query_row(
                "SELECT coalesce(max(position),0) FROM spike_layout_sections
                 WHERE space=?1 AND actor=?2 AND app=?3",
                params![self.space, self.actor, self.app],
                |r| r.get(0),
            ),
        }
        .unwrap()
    }

    fn sections(self, conn: &Connection, offset: i64, size: i64) -> Vec<Section> {
        conn.prepare_cached(
            "SELECT id,title,position FROM spike_layout_sections
             WHERE space=?1 AND actor=?2 AND app=?3 ORDER BY position,id LIMIT ?4 OFFSET ?5",
        )
        .unwrap()
        .query_map(
            params![self.space, self.actor, self.app, size, offset],
            |r| Ok(Section(r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
        .map(Result::unwrap)
        .collect()
    }

    pub(super) fn snapshot(self, conn: &mut Connection) -> Option<Vec<(Section, Vec<Entry>)>> {
        let tx = conn.transaction().unwrap();
        if !self.live(&tx) {
            return None;
        }
        Some(
            self.sections(&tx, 0, -1)
                .into_iter()
                .map(|s| {
                    let entries = self.rows(&tx, &s.0, 0, -1);
                    (s, entries)
                })
                .collect(),
        )
    }

    pub(super) fn edit(
        self,
        conn: &mut Connection,
        pinned: i64,
        edit: Edit<'_>,
        cut: Cut,
    ) -> Option<Uuid> {
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        if !self.live(&tx) || self.revision(&tx) != pinned {
            return None;
        }
        let inverse =
            match edit {
                Edit::Create(id, title) => {
                    if self.section(&tx, id).is_some() {
                        return None;
                    }
                    self.set_section(
                        &tx,
                        &Section(id.into(), title.into(), self.max_position(&tx, None) + 1),
                    );
                    Inverse::DropSection(id.into())
                }
                Edit::Rename(id, title) => {
                    let before = self.section(&tx, id)?;
                    self.set_section(&tx, &Section(id.into(), title.into(), before.2));
                    Inverse::SetSection(before, vec![])
                }
                Edit::Remove(id) => {
                    let before = self.section(&tx, id)?;
                    let entries = self.rows(&tx, id, 0, -1);
                    self.remove_section(&tx, id);
                    Inverse::SetSection(before, entries)
                }
                Edit::To(id, destination) => {
                    self.section(&tx, destination)?;
                    let allowed: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM spike_layout_item WHERE space=?1 AND id=?2)",
                    params![self.space, id], |r| r.get(0),
                ).unwrap();
                    if !allowed {
                        return None;
                    }
                    let before = self.entry(&tx, id);
                    self.set_entry(
                        &tx,
                        &Entry(
                            id.into(),
                            destination.into(),
                            self.max_position(&tx, Some(destination)) + 1,
                        ),
                    );
                    before.map_or_else(
                        || Inverse::DropEntry(id.into()),
                        |row| Inverse::SetEntries(vec![row]),
                    )
                }
                Edit::Up(id) => self.swap(&tx, id, true)?,
                Edit::Down(id) => self.swap(&tx, id, false)?,
            };
        self.advance(&tx, pinned + 1);
        let token = Uuid::new_v4();
        tx.execute(
            "INSERT INTO spike_undo VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                token.to_string(),
                self.space,
                self.actor,
                self.app,
                pinned + 1,
                serde_json::to_string(&inverse).unwrap()
            ],
        )
        .unwrap();
        match cut {
            Cut::Rollback => return None,
            Cut::BeforeCommit => std::process::exit(71),
            _ => {}
        }
        tx.commit().unwrap();
        match cut {
            Cut::AfterCommit => std::process::exit(72),
            Cut::DuringReply => {
                use std::io::Write;
                let mut out = std::io::stdout().lock();
                write!(out, r#"{{"undo":""#).unwrap();
                out.flush().unwrap();
                std::process::exit(73);
            }
            _ => Some(token),
        }
    }

    fn swap(self, tx: &Transaction<'_>, id: &str, up: bool) -> Option<Inverse> {
        let entry = self.entry(tx, id)?;
        let sql = if up {
            "SELECT id,section,position FROM spike_placement WHERE space=?1 AND actor=?2 AND app=?3
             AND section=?4 AND position<?5 ORDER BY position DESC,id DESC LIMIT 1"
        } else {
            "SELECT id,section,position FROM spike_placement WHERE space=?1 AND actor=?2 AND app=?3
             AND section=?4 AND position>?5 ORDER BY position,id LIMIT 1"
        };
        let other = tx
            .query_row(
                sql,
                params![self.space, self.actor, self.app, entry.1, entry.2],
                |r| Ok(Entry(r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .unwrap()?;
        self.set_entry(tx, &Entry(entry.0.clone(), entry.1.clone(), other.2));
        self.set_entry(tx, &Entry(other.0.clone(), other.1.clone(), entry.2));
        Some(Inverse::SetEntries(vec![entry, other]))
    }

    pub(super) fn undo(self, conn: &mut Connection, token: Uuid) -> bool {
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        if !self.live(&tx) {
            return false;
        }
        let rev = self.revision(&tx);
        let inverse: Option<String> = tx.query_row(
            "SELECT inverse FROM spike_undo WHERE token=?1 AND space=?2 AND actor=?3 AND app=?4 AND rev=?5",
            params![token.to_string(), self.space, self.actor, self.app, rev], |r| r.get(0),
        ).optional().unwrap();
        let Some(inverse) = inverse else {
            return false;
        };
        match serde_json::from_str(&inverse).unwrap() {
            Inverse::DropSection(id) => self.remove_section(&tx, &id),
            Inverse::SetSection(section, entries) => {
                self.set_section(&tx, &section);
                for entry in &entries {
                    self.set_entry(&tx, entry);
                }
            }
            Inverse::SetEntries(entries) => {
                for entry in &entries {
                    self.set_entry(&tx, entry);
                }
            }
            Inverse::DropEntry(id) => {
                tx.execute(
                    "DELETE FROM spike_placement WHERE space=?1 AND actor=?2 AND app=?3 AND id=?4",
                    params![self.space, self.actor, self.app, id],
                )
                .unwrap();
            }
        }
        tx.execute("DELETE FROM spike_undo WHERE token=?1", [token.to_string()])
            .unwrap();
        self.advance(&tx, rev + 1);
        tx.commit().unwrap();
        true
    }

    pub(super) fn pin(self, conn: &Connection) -> i64 {
        self.revision(conn)
    }
}
