//! Prototype search maintenance in the same FULL content transaction as a save.
use super::search::SCHEMA;
use rusqlite::{Connection, params};

fn names(audience: i64) -> (String, String) {
    (
        format!("audience_{audience}_docs"),
        format!("audience_{audience}_fts"),
    )
}

pub(super) fn install(conn: &mut Connection, moving: i64) {
    // One FTS instance per already-authorized scope. Use the existing column,
    // prefix and unicode61 options. Each scope also has its title-range index.
    let docs_schema = SCHEMA
        .split_once("CREATE TABLE IF NOT EXISTS search_docs")
        .unwrap()
        .1
        .split_once("CREATE VIRTUAL TABLE IF NOT EXISTS search_title_grams")
        .unwrap()
        .0;
    let tables = format!("CREATE TABLE IF NOT EXISTS search_docs{docs_schema}");
    let tx = conn.transaction().unwrap();
    let mut inserts = String::new();
    for audience in (1..32).chain([moving, moving + 1]) {
        let (docs, fts) = names(audience);
        let schema = tables
            .replace("search_docs", &docs)
            .replace("search_text", &fts)
            .replace("search_kind_prefix", &format!("{docs}_kind_prefix"))
            .replace("search_prefix", &format!("{docs}_prefix"));
        tx.execute_batch(&schema).unwrap();
        tx.execute(
            &format!(
                "INSERT INTO {docs}
             SELECT i.rowid,i.id,'notes','Existing note','existing note'
             FROM scoped_membership m JOIN items i ON i.id=m.id
             WHERE m.collection=1 AND m.audience=?1
             LIMIT 6000"
            ),
            [audience],
        )
        .unwrap();
        tx.execute_batch(&format!(
            "INSERT INTO {fts}(rowid,title,body)
             SELECT rowid,title,'notebook archive reference project weekly meeting' FROM {docs};
             INSERT INTO {fts}({fts}) VALUES('optimize');"
        ))
        .unwrap();

        // Skip any scope where the row has no old or new grant. The content
        // trigger has already staged the live title/sort and the new version.
        let eligible = format!(
            "({audience} IN fixed_audiences OR {audience}=NEW.metadata->>'moving_audience')"
        );
        inserts.push_str(&format!(
            "DELETE FROM {fts} WHERE rowid=(SELECT rowid FROM {docs} WHERE id=NEW.item);
             DELETE FROM {docs} WHERE id=NEW.item;
             INSERT INTO {docs}
             SELECT rowid,id,kind,NEW.metadata->>'title',lower(NEW.metadata->>'title')
             FROM items WHERE id=NEW.item AND {eligible};
             INSERT INTO {fts}(rowid,title,body)
             SELECT rowid,title,substr(CAST(NEW.bytes AS TEXT),1,8192)
             FROM {docs} WHERE id=NEW.item AND {eligible};"
        ));
    }
    tx.execute_batch(&format!(
        "CREATE TRIGGER search_on_save AFTER INSERT ON versions
         WHEN NEW.item IN save_projection_target BEGIN {inserts} END;"
    ))
    .unwrap();
    tx.commit().unwrap();
}

pub(super) fn verify(conn: &Connection, id: &str, moving: i64, bit: i64) {
    for (audience, wanted) in [(1, true), (moving + bit, true), (moving + 1 - bit, false)] {
        let (docs, fts) = names(audience);
        let matches: i64 = conn
            .query_row(
                &format!(
                    "SELECT count(*) FROM {fts} WHERE {fts} MATCH ?1
                       AND rowid=(SELECT rowid FROM {docs} WHERE id=?2)"
                ),
                params![if bit == 0 { "oddmarker" } else { "evenmarker" }, id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(matches, i64::from(wanted));
        let old: i64 = conn
            .query_row(
                &format!(
                    "SELECT count(*) FROM {fts} WHERE {fts} MATCH ?1
                       AND rowid=(SELECT rowid FROM {docs} WHERE id=?2)"
                ),
                params![if bit == 0 { "evenmarker" } else { "oddmarker" }, id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(old, 0);
    }
}
