//! Disposable retained-generation layout removal and Undo.
use super::fixture;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use std::time::{Duration, Instant};

const KEEP_UNDO: i64 = 60; // Fixture clock only. Product retention is host policy.
const GC_BATCH: i64 = 256;
const PAGE: &str = "SELECT item FROM placement
 WHERE section=?1 AND generation=?2 ORDER BY position,item LIMIT 200";
const SCHEMA: &str = "
CREATE TABLE layout(rev INTEGER NOT NULL) STRICT;
INSERT INTO layout VALUES(0);
CREATE TABLE section(
  id INTEGER PRIMARY KEY, generation INTEGER NOT NULL, position INTEGER NOT NULL,
  live INTEGER NOT NULL CHECK(live IN (0,1))
) STRICT;
CREATE INDEX section_order ON section(position,id) WHERE live=1;
CREATE TABLE placement(section INTEGER, generation INTEGER, position INTEGER, item TEXT,
  PRIMARY KEY(section,generation,position,item)
) STRICT, WITHOUT ROWID;
CREATE TABLE undo(
  token TEXT PRIMARY KEY, section INTEGER NOT NULL, generation INTEGER NOT NULL,
  rev INTEGER NOT NULL, deadline INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
CREATE INDEX expired_undo ON undo(deadline,token);
";

struct Layout {
    conn: Connection,
    actor: String,
    space: String,
}

impl Layout {
    fn new(path: &std::path::Path, actor: String, space: String, count: i64) -> Self {
        let mut conn = Connection::open(path).unwrap();
        conn.pragma_update(None, "synchronous", "FULL").unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        let tx = conn.transaction().unwrap();
        // This isolates the cardinality cost of the same placement projection.
        // The real Notes-backed fixture measures the rest of the engine path.
        tx.execute("INSERT INTO section VALUES(1,100,1,1),(2,200,2,1)", [])
            .unwrap();
        tx.execute(
            "WITH RECURSIVE rows(i) AS (
                 VALUES(1) UNION ALL SELECT i+1 FROM rows WHERE i<?1
             ) INSERT INTO placement SELECT 1,100,i,printf('00000000-0000-4000-8000-%012x',i) FROM rows",
            [count],
        ).unwrap();
        tx.execute(
            "INSERT INTO placement SELECT 2,200,position,item FROM placement
                    WHERE section=1 AND generation=100 AND position<=200",
            [],
        )
        .unwrap();
        tx.commit().unwrap();
        Self { conn, actor, space }
    }

    fn live(conn: &Connection, actor: &str, space: &str) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM space_account_roles WHERE account_id=?1 AND space_id=?2)",
            params![actor, space],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn generation(conn: &Connection, section: i64) -> Option<i64> {
        conn.query_row(
            "SELECT generation FROM section WHERE id=?1 AND live=1",
            [section],
            |row| row.get(0),
        )
        .optional()
        .unwrap()
    }

    fn retire(&mut self, section: i64, now: i64) -> Option<String> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        if !Self::live(&tx, &self.actor, &self.space) {
            return None;
        }
        let generation = Self::generation(&tx, section)?;
        let revision: i64 = tx
            .query_row("UPDATE layout SET rev=rev+1 RETURNING rev", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            tx.execute("UPDATE section SET live=0 WHERE id=?1", [section])
                .unwrap(),
            1
        );
        let token = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO undo VALUES(?1,?2,?3,?4,?5)",
            params![token, section, generation, revision, now + KEEP_UNDO],
        )
        .unwrap();
        tx.commit().unwrap();
        Some(token)
    }

    fn undo(&mut self, token: &str, now: i64) -> bool {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        if !Self::live(&tx, &self.actor, &self.space) {
            return false;
        }
        let marker: Option<(i64, i64)> = tx
            .query_row(
                "SELECT section,generation FROM undo WHERE token=?1 AND deadline>?2
             AND rev=(SELECT rev FROM layout)",
                params![token, now],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .unwrap();
        let Some((section, generation)) = marker else {
            return false;
        };
        assert_eq!(
            tx.execute(
                "UPDATE section SET live=1 WHERE id=?1 AND generation=?2 AND live=0",
                params![section, generation]
            )
            .unwrap(),
            1
        );
        assert_eq!(tx.execute("UPDATE layout SET rev=rev+1", []).unwrap(), 1);
        assert_eq!(
            tx.execute("DELETE FROM undo WHERE token=?1", [token])
                .unwrap(),
            1
        );
        tx.commit().unwrap();
        true
    }

    fn page(&mut self, section: i64) -> Option<Vec<String>> {
        let tx = self.conn.transaction().unwrap();
        if !Self::live(&tx, &self.actor, &self.space) {
            return None;
        }
        let generation = Self::generation(&tx, section)?;
        let rows = tx
            .prepare_cached(PAGE)
            .unwrap()
            .query_map(params![section, generation], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        Some(rows)
    }

    fn collect(&mut self, now: i64) -> Option<usize> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        let (token,section,generation): (String,i64,i64) = tx.query_row(
            "SELECT token,section,generation FROM undo WHERE deadline<=?1 ORDER BY deadline,token LIMIT 1",
            [now], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        ).optional().unwrap()?;
        let removed = Self::delete_batch(&tx, section, generation);
        if removed < GC_BATCH as usize {
            assert_eq!(
                tx.execute(
                    "DELETE FROM section WHERE id=?1 AND generation=?2 AND live=0",
                    params![section, generation]
                )
                .unwrap(),
                1
            );
            assert_eq!(
                tx.execute("DELETE FROM undo WHERE token=?1", [token])
                    .unwrap(),
                1
            );
        }
        tx.commit().unwrap();
        Some(removed)
    }

    fn delete_batch(tx: &Transaction<'_>, section: i64, generation: i64) -> usize {
        tx.execute(
            "DELETE FROM placement WHERE (section,generation,position,item) IN (
                SELECT section,generation,position,item FROM placement
                WHERE section=?1 AND generation=?2 ORDER BY position,item LIMIT ?3
            )",
            params![section, generation, GC_BATCH],
        )
        .unwrap()
    }
}

#[test]
#[ignore = "disposable step 0 bounded layout perf fixture"]
fn retained_layout_generation() {
    let dir = tempfile::tempdir().unwrap();
    let runs: usize = std::env::var("STEP0_RUNS").map_or(100, |n| n.parse().unwrap());
    for count in [256, 4096, 100_000] {
        let roots = fixture::roots(&dir.path().join(count.to_string()));
        let ids = fixture::spaces(&roots).seed_development().unwrap();
        let mut layout = Layout::new(
            &roots.appdata.join("system/auth.db"),
            ids.owner.to_string(),
            ids.shared.to_string(),
            count,
        );
        let expected: Vec<_> = (1..=200)
            .map(|i| format!("00000000-0000-4000-8000-{i:012x}"))
            .collect();
        let mut remove = Vec::new();
        let mut undo = Vec::new();
        let mut visible_before = Vec::new();
        let mut visible_during = Vec::new();
        let mut hidden = Vec::new();
        let mut changes = Vec::new();
        let syncs = sync_counter();
        let mut remove_syncs = std::collections::BTreeMap::new();
        let mut undo_syncs = std::collections::BTreeMap::new();
        for _ in 0..runs {
            let start = Instant::now();
            let baseline = layout.page(2);
            visible_before.push(start.elapsed());
            assert_eq!(baseline.as_ref(), Some(&expected));

            let before = layout.conn.total_changes();
            let before_sync = syncs.map(|read| unsafe { read() });
            let start = Instant::now();
            let token = layout.retire(1, 0).unwrap();
            remove.push(start.elapsed());
            if let Some((read, before)) = syncs.zip(before_sync) {
                *remove_syncs.entry(unsafe { read() } - before).or_insert(0) += 1;
            }
            changes.push(layout.conn.total_changes() - before);
            assert_eq!(layout.collect(KEEP_UNDO - 1), None);

            let start = Instant::now();
            let other = layout.page(2);
            visible_during.push(start.elapsed());
            assert_eq!(other.as_ref(), Some(&expected));

            let start = Instant::now();
            let absent = layout.page(1);
            hidden.push(start.elapsed());
            assert!(absent.is_none());

            let before = layout.conn.total_changes();
            let before_sync = syncs.map(|read| unsafe { read() });
            let start = Instant::now();
            assert!(layout.undo(&token, KEEP_UNDO - 1));
            undo.push(start.elapsed());
            if let Some((read, before)) = syncs.zip(before_sync) {
                *undo_syncs.entry(unsafe { read() } - before).or_insert(0) += 1;
            }
            changes.push(layout.conn.total_changes() - before);
            assert_eq!(layout.page(1).as_ref(), Some(&expected));
            assert!(!layout.undo(&token, KEEP_UNDO - 1));
        }
        assert!(changes.iter().all(|&n| n == 3));

        let token = layout.retire(1, 0).unwrap();
        assert!(!layout.undo(&token, KEEP_UNDO));
        let mut reclaimed = 0;
        let mut batches = Vec::new();
        while let Some(rows) = {
            let start = Instant::now();
            let next = layout.collect(KEEP_UNDO);
            if next.is_some() {
                batches.push(start.elapsed());
            }
            next
        } {
            reclaimed += rows;
            assert!(rows <= GC_BATCH as usize);
        }
        assert_eq!(reclaimed, count as usize);
        assert!(layout.page(1).is_none());
        assert_eq!(layout.page(2).as_ref(), Some(&expected));
        eprintln!(
            "retained placements={count} reclaim_rows={reclaimed} steps={}",
            batches.len()
        );
        if syncs.is_some() {
            eprintln!("sync_calls removal={remove_syncs:?} undo={undo_syncs:?}");
            assert!(remove_syncs.keys().all(|&n| n > 0));
            assert!(undo_syncs.keys().all(|&n| n > 0));
        }
        for (name, values) in [
            ("generation remove", &mut remove),
            ("generation undo", &mut undo),
            ("visible page before", &mut visible_before),
            ("visible page during", &mut visible_during),
            ("retired page denied", &mut hidden),
            ("reclamation batch", &mut batches),
        ] {
            fixture::summary(name, values);
        }
        for samples in [&remove, &undo] {
            if samples[(runs * 95).div_ceil(100) - 1] > Duration::from_millis(40) {
                eprintln!("gate_miss bounded_layout placements={count}");
            }
        }
    }
}

fn sync_counter() -> Option<unsafe extern "C" fn() -> u64> {
    // The Linux-only perf preload exports this counter. Normal smoke runs omit it.
    let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"step0_sync_calls".as_ptr()) };
    (!symbol.is_null()).then(|| unsafe {
        std::mem::transmute::<*mut libc::c_void, unsafe extern "C" fn() -> u64>(symbol)
    })
}
