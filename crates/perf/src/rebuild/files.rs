//! Stage current metadata and bytes using the disposable fixture's Markdown.
use super::*;
use std::{fs::File, io};

pub(super) struct Document {
    pub(super) kind: String,
    id: String,
    path: String,
}

pub(super) fn select(
    db: &Path,
    userdata: &Path,
    routes: &Path,
    space: &str,
    notes: usize,
) -> Result<Vec<Document>> {
    let mut conn = Connection::open(db)?;
    conn.execute(
        "ATTACH DATABASE ?1 AS routes",
        [routes.to_str().ok_or("non-UTF-8 fixture path")?],
    )?;
    let tx = conn.transaction()?;
    tx.execute_batch("CREATE TEMP TABLE retained(id TEXT PRIMARY KEY);")?;
    tx.execute(
        "INSERT INTO retained SELECT id FROM items WHERE kind='notes' ORDER BY rowid LIMIT ?1",
        [notes as i64],
    )?;
    tx.execute_batch("INSERT INTO retained SELECT id FROM items WHERE kind='threads';")?;
    let mut removed = false;
    {
        let mut excess = tx.prepare("SELECT path FROM items WHERE id NOT IN retained")?;
        let mut rows = excess.query([])?;
        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            let path = Path::new(&path);
            if path
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                return Err("nonrelative file path in fixture".into());
            }
            std::fs::remove_file(userdata.join(path))?;
            removed = true;
        }
    }
    tx.execute(
        "DELETE FROM routes.items WHERE space=?1 AND id NOT IN retained",
        [space],
    )?;
    tx.execute_batch(
        "DELETE FROM versions WHERE item NOT IN retained;
         DELETE FROM items WHERE id NOT IN retained;",
    )?;
    let rows = {
        let mut select = tx.prepare("SELECT kind,id,path FROM items ORDER BY rowid")?;
        select
            .query_map([], |r| {
                Ok(Document {
                    kind: r.get(0)?,
                    id: r.get(1)?,
                    path: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    tx.commit()?;
    if removed {
        // Reproduce the smaller corpus on disk, including its directory reads
        // and SQLite page footprint. This fixture preparation is untimed.
        conn.execute_batch("VACUUM main; VACUUM routes;")?;
    }
    Ok(rows)
}

pub(super) fn warm(userdata: &Path, db: &Path, docs: &[Document]) -> Result<()> {
    for path in
        std::iter::once(db.to_path_buf()).chain(docs.iter().map(|doc| userdata.join(&doc.path)))
    {
        io::copy(&mut File::open(path)?, &mut io::sink())?;
    }
    Ok(())
}

pub(super) fn stage(userdata: &Path, db: &Path, docs: &[Document]) -> Result<()> {
    let mut conn = Connection::open(db)?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    let tx = conn.transaction()?;
    {
        let mut update_head = tx.prepare_cached("UPDATE items SET metadata=?2 WHERE id=?1")?;
        let mut update_revision = tx.prepare_cached(
            "UPDATE versions SET metadata=?2,bytes=?3 WHERE item=?1 AND seq=(SELECT seq FROM items WHERE id=?1)",
        )?;
        for doc in docs {
            let path = Path::new(&doc.path);
            let text = std::fs::read_to_string(userdata.join(path))?;
            let (properties, body) = text
                .strip_prefix("---\n")
                .and_then(|rest| rest.split_once("\n---\n"))
                .ok_or("fixture lacks Markdown envelope")?;
            let (identity, properties) = properties.split_once('\n').unwrap_or((properties, ""));
            if identity.strip_prefix("calternal-id: ") != Some(&doc.id) {
                return Err("fixture file identity differs from retained content core".into());
            }
            let canonical = if properties.is_empty() {
                body.to_owned()
            } else {
                format!("---\n{properties}\n---\n{body}")
            };
            let metadata = if doc.kind == "notes" {
                let fallback = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or("fixture note has no title")?;
                calternal_server::rebuild_probe::note_metadata(fallback, &canonical)?
            } else {
                let mut fields = serde_json::Map::new();
                for field in properties.lines() {
                    let (key, value) = field
                        .split_once(": ")
                        .ok_or("fixture thread has malformed metadata")?;
                    fields.insert(key.replace('-', "_"), serde_json::from_str(value)?);
                }
                Value::Object(fields)
            };
            let metadata = serde_json::to_string(&metadata)?;
            update_head.execute(params![doc.id, metadata])?;
            update_revision.execute(params![doc.id, metadata, canonical.as_bytes()])?;
        }
    }
    tx.commit()?;
    Ok(())
}
