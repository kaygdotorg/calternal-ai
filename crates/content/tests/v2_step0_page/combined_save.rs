//! Measure real Content saves with scoped membership triggers in content.db.
use super::covering::COUNTS;
use super::scoped::{DYNAMIC, SCHEMA, populate_membership};
use super::search_save;
use super::{RUNS, summary};
use calternal_content::{Content, Item, Request};
use calternal_types::{AccountId, SpaceId};
use rusqlite::{Connection, params};
use serde_json::json;
use std::path::Path;
use std::time::{Duration, Instant};

const MAX_SCOPES: i64 = 256;
const MEMBERS_PER_SCOPE: i64 = 6_000;

const PROJECTION: &str = "
    CREATE TABLE fixed_audiences(audience INTEGER PRIMARY KEY);
    CREATE TABLE save_projection_target(id TEXT PRIMARY KEY);
    CREATE TRIGGER move_scope_on_save AFTER UPDATE OF updated_at ON items
    WHEN NEW.id IN save_projection_target
    BEGIN
        UPDATE scoped_membership SET newest=-CAST(unixepoch(NEW.updated_at,'subsec')*1000 AS INTEGER)
        WHERE collection=1 AND audience IN fixed_audiences
        AND newest=-CAST(unixepoch(OLD.updated_at,'subsec')*1000 AS INTEGER)
        AND id=NEW.id;
        DELETE FROM scoped_membership WHERE collection=1
        AND audience=OLD.metadata->>'moving_audience'
        AND newest=-CAST(unixepoch(OLD.updated_at,'subsec')*1000 AS INTEGER)
        AND id=NEW.id;
        INSERT INTO scoped_membership
        SELECT 1,NEW.metadata->>'moving_audience',
               -CAST(unixepoch(NEW.updated_at,'subsec')*1000 AS INTEGER),NEW.id
        WHERE NEW.metadata->>'moving_audience' IS NOT NULL;
        UPDATE projection_version SET result=result+1;
    END;";

fn save(
    content: &Content,
    space: SpaceId,
    actor: AccountId,
    prior: Option<&Item>,
    scopes: i64,
    run: usize,
    sized_body: Option<&str>,
) -> Item {
    let moving = (scopes != 0).then_some(MAX_SCOPES + (run % 2) as i64);
    let key = format!("combined-{scopes}-{run}");
    // Match the ordinary small, distinct body used by the existing note-save
    // arm in crates/perf/src/notes_scenario.rs.
    let body = format!("seed 0 save {run}");
    content
        .write(
            space,
            prior.map(|i| i.id),
            "notes",
            prior.map(|i| i.revision.as_str()),
            json!({"title":"Combined save", "moving_audience": moving}),
            sized_body.unwrap_or(&body).as_bytes(),
            Request {
                account: actor,
                via: None,
                id: &key,
                operation: "notes.update",
                fingerprint: &key,
            },
            |item| json!({"id":item.id,"revision":item.revision}),
        )
        .unwrap_or_else(|e| panic!("save scopes={scopes} run={run}: {e:?}"))
}

fn set_scope(db: &mut Connection, initial: &Item, scopes: i64, old_target: Option<&Item>) {
    let tx = db.transaction().unwrap();
    if let Some(old_target) = old_target {
        tx.execute(
            "DELETE FROM scoped_membership WHERE collection=?1
             AND audience BETWEEN 1 AND ?4
             AND newest=-CAST(unixepoch(?2,'subsec')*1000 AS INTEGER)
             AND id=?3",
            params![
                DYNAMIC,
                old_target.updated_at,
                old_target.id.to_string(),
                MAX_SCOPES + 1
            ],
        )
        .unwrap();
    }
    tx.execute_batch("DELETE FROM fixed_audiences; DELETE FROM save_projection_target;")
        .unwrap();
    if scopes != 0 {
        tx.execute(
            "INSERT INTO save_projection_target VALUES(?1)",
            [initial.id.to_string()],
        )
        .unwrap();
        for audience in 1..scopes {
            tx.execute("INSERT INTO fixed_audiences VALUES(?1)", [audience])
                .unwrap();
        }
        tx.execute(
            "INSERT INTO scoped_membership
             SELECT ?1,audience,-CAST(unixepoch(?2,'subsec')*1000 AS INTEGER),?3 FROM fixed_audiences
             UNION ALL SELECT ?1,?4,-CAST(unixepoch(?2,'subsec')*1000 AS INTEGER),?3",
             params![DYNAMIC,initial.updated_at,initial.id.to_string(),MAX_SCOPES],
        ).unwrap();
    }
    tx.commit().unwrap();
}

fn verify(writer: &Connection, item: &Item, scopes: i64) {
    let moving = item.metadata["moving_audience"].as_i64();
    let keys: i64 = writer
        .query_row(
            "SELECT count(*) FROM scoped_membership
         WHERE collection=?1
         AND audience IN (SELECT audience FROM fixed_audiences UNION ALL SELECT ?4)
         AND newest=-CAST(unixepoch(?2,'subsec')*1000 AS INTEGER) AND id=?3",
            params![DYNAMIC, item.updated_at, item.id.to_string(), moving],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(keys, scopes);
    if scopes > 0 {
        let counts: (i64, i64) = writer
            .query_row(
                "SELECT (SELECT n FROM scoped_counts WHERE collection=1 AND audience=?1),
                    (SELECT n FROM scoped_counts WHERE collection=1 AND audience=?2)",
                [MAX_SCOPES, MAX_SCOPES + 1],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            counts,
            if moving == Some(MAX_SCOPES) {
                (MEMBERS_PER_SCOPE + 1, MEMBERS_PER_SCOPE)
            } else {
                (MEMBERS_PER_SCOPE, MEMBERS_PER_SCOPE + 1)
            }
        );
    }
}

pub(super) fn exercise(content: &Content, space: SpaceId, path: &Path) {
    run(content, space, path, None);
}

pub(super) fn exercise_search(content: &Content, space: SpaceId, path: &Path, enabled: bool) {
    run(content, space, path, Some(enabled));
}

fn run(content: &Content, space: SpaceId, path: &Path, search: Option<bool>) {
    let mut writer = Connection::open(path).unwrap();
    writer.busy_timeout(Duration::from_secs(5)).unwrap();
    writer.execute_batch(SCHEMA).unwrap();
    writer.execute_batch(COUNTS).unwrap();
    writer.execute_batch(PROJECTION).unwrap();
    let tx = writer.transaction().unwrap();
    populate_membership(&tx, "main");
    for audience in 2..=MAX_SCOPES + 1 {
        tx.execute(
            "INSERT INTO scoped_membership
             SELECT ?1,?2,newest,id FROM scoped_membership WHERE collection=?1 AND audience=1
             ORDER BY newest,id LIMIT ?3 OFFSET ?4",
            params![
                DYNAMIC,
                audience,
                MEMBERS_PER_SCOPE,
                (audience * 4_000) % 85_000
            ],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    if search == Some(true) {
        search_save::install(&mut writer, MAX_SCOPES);
    }
    writer
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();

    let registered: bool = writer
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM kinds WHERE kind='notes')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(registered);
    let actor = AccountId::new();
    let mut last = None;
    let widths = [0, 1, 4, 16, 32, 64, 256];
    let selected = if search.is_some() {
        Some(32)
    } else {
        std::env::var("STEP0_SCOPE").ok().map(|value| {
            let width = value.parse().unwrap();
            assert!(widths.contains(&width));
            width
        })
    };
    let corpus = [" oddmarker ", " evenmarker "].map(|marker| {
        format!(
            "{marker}{}",
            "notebook agenda workshop archive reference ".repeat(200)
        )[..8192]
            .to_owned()
    });
    for scopes in widths
        .into_iter()
        .filter(|width| selected.is_none_or(|selected| *width == selected))
    {
        let mut item = save(
            content,
            space,
            actor,
            None,
            scopes,
            0,
            search.map(|_| corpus[0].as_str()),
        );
        content.materialize(space, item.id).unwrap();
        set_scope(&mut writer, &item, scopes, last.as_ref());
        let mut samples = Vec::new();
        for run in 1..=RUNS {
            let body = search.map(|_| corpus[run % 2].as_str());
            let start = Instant::now();
            item = save(content, space, actor, Some(&item), scopes, run, body);
            samples.push(start.elapsed().as_micros());
            // Placement is the engine's separate durability stage, not part
            // of the user-visible save acknowledgment being timed here.
            content.materialize(space, item.id).unwrap();
            verify(&writer, &item, scopes);
            if search == Some(true) {
                search_save::verify(&writer, &item.id.to_string(), MAX_SCOPES, (run % 2) as i64);
            }
        }
        println!(
            "combined save search={search:?} scopes={scopes} n={RUNS} membership_keys={} count_rows={} us {}",
            2 * scopes,
            if scopes == 0 { 0 } else { 2 },
            summary(&mut samples)
        );
        last = Some(item);
    }
}
