//! Outside file edits fork the checkpoint for the last file we can prove.
use super::{Body, Content, FileFormat, Item, Result, Space, SpaceId, internal, style::Style};
use crate::{Request, db::Header, file_hash, files, mutations::Destination, types::blocked};
use calternal_types::{AccountId, Error};
use rusqlite::params;
use serde_json::json;
use uuid::Uuid;

struct Outside {
    bytes: Vec<u8>,
    base: String,
    observed: String,
}

impl Content {
    /// Reconcile changed Markdown from the item's current pathname as a room
    /// edit. The triggering account authorizes the attempt but the operation
    /// records its origin in the file. A raced or unprovable original fails
    /// closed. An unchanged file performs no write.
    /// Cost: one guarded file read on an unchanged item. A change can inspect
    /// up to 64 recent revisions and commits one content revision and receipt.
    pub fn reconcile_room_file(
        &self,
        space: SpaceId,
        kind: &str,
        id: Uuid,
        account: AccountId,
    ) -> Result<bool> {
        let Some(outside) = self.with_space(space, |s| {
            self.publish_pending(s)?;
            s.observe_external(kind, id)
        })?
        else {
            return Ok(false);
        };
        let fingerprint =
            blake3::hash(format!("{id}\0{}\0{}", outside.base, outside.observed).as_bytes())
                .to_hex()
                .to_string();
        let request_id = format!("room-import/{fingerprint:0>128}");
        let op = format!("{kind}.room-import");
        let request = Request {
            account,
            via: None,
            id: &request_id,
            operation: &op,
            fingerprint: &fingerprint,
        };
        self.write_placed(
            space,
            Some(id),
            kind,
            Some(&outside.base),
            serde_json::Value::Null,
            Body::Outside {
                bytes: &outside.bytes,
                observed: &outside.observed,
            },
            request,
            |item| json!({"id":item.id,"revision":item.revision}),
            Destination::Current,
            None,
            None,
            None,
            || Ok(()),
            |_| Ok(()),
        )?;
        Ok(true)
    }
}

impl Space {
    fn observe_external(&self, kind: &str, id: Uuid) -> Result<Option<Outside>> {
        if !self.live_editing(kind)? {
            return Ok(None);
        }
        let head = self.header(id)?;
        if head.kind != kind || head.path.starts_with(".trash/") || head.old_path.is_some() {
            return Err(blocked());
        }
        let Some(expected) = head.file_hash.as_deref() else {
            return Ok(None);
        };
        let Some(bytes) = self.observe_file(&head)? else {
            return Err(blocked());
        };
        let observed = file_hash(&bytes);
        if observed == expected {
            return Ok(None);
        }
        let current = self.get_selected(id, true)?;
        if head.state == "pending" && file_hash(&files::encoded_live(&current)?) == observed {
            return Ok(None);
        }
        let base = self.disk_base(current, head.seq, expected)?;
        let contents = text_from_file(&bytes, &base)?;
        Ok(Some(Outside {
            bytes: contents,
            base: base.revision,
            observed,
        }))
    }

    fn observe_file(&self, head: &Header) -> Result<Option<Vec<u8>>> {
        calternal_fsio::recover_exact(&self.files.join(&head.path)).map_err(internal)?;
        files::bounded_file(&self.files, &head.path)
    }

    pub(crate) fn verify_external(&self, head: &Header, observed: &str) -> Result<()> {
        if head.old_path.is_some()
            || !self
                .observe_file(head)?
                .is_some_and(|bytes| file_hash(&bytes) == observed)
        {
            return Err(blocked());
        }
        Ok(())
    }

    fn disk_base(&self, mut current: Item, seq: i64, expected: &str) -> Result<Item> {
        if file_hash(&files::encoded_live(&current)?) == expected
            || file_hash(&files::encoded(&current, &FileFormat::Markdown)?) == expected
        {
            return Ok(current);
        }
        let mut stmt = self.conn.prepare_cached(
            "SELECT bytes,revision FROM versions WHERE item=?1 AND seq<?2 ORDER BY seq DESC LIMIT 63",
        ).map_err(internal)?;
        let mut rows = stmt
            .query(params![current.id.to_string(), seq])
            .map_err(internal)?;
        while let Some(row) = rows.next().map_err(internal)? {
            current.bytes = row.get(0).map_err(internal)?;
            current.revision = row.get(1).map_err(internal)?;
            if file_hash(&files::encoded_live(&current)?) == expected {
                return Ok(current);
            }
        }
        Err(
            Error::conflict("the outside file is preserved and its saved base could not be proved")
                .with_details(json!({"reason":"external_change"})),
        )
    }
}

fn text_from_file(bytes: &[u8], base: &Item) -> Result<Vec<u8>> {
    let (style, observed) = Style::split(std::str::from_utf8(bytes).map_err(|_| blocked())?);
    let (_, original) = Style::split(std::str::from_utf8(&base.bytes).map_err(internal)?);
    let original = crate::markdown_frontmatter(&original);
    let original_format = original.as_ref().and_then(|front| {
        front
            .properties
            .lines()
            .find_map(|line| line.strip_prefix("format:").map(str::trim))
    });
    let front = crate::markdown_frontmatter(&observed).ok_or_else(blocked)?;
    let mut seen_id = false;
    let mut seen_format = false;
    let mut text = String::from(front.opening);
    for line in front.properties.split_inclusive('\n') {
        if let Some(id) = line.strip_prefix("calternal-id:") {
            if seen_id || Uuid::parse_str(id.trim()).ok() != Some(base.id) {
                return Err(blocked());
            }
            seen_id = true;
        } else if let Some(format) = line.strip_prefix("format:") {
            if seen_format || format.trim() != original_format.unwrap_or("calternal-note/v1") {
                return Err(blocked());
            }
            seen_format = true;
            if original_format.is_some() {
                text.push_str(line);
            }
        } else {
            text.push_str(line);
        }
    }
    if !seen_id || !seen_format {
        return Err(blocked());
    }
    if text.len() == front.opening.len() && original.is_none() {
        return Ok(style.encode(front.body));
    }
    text.push_str(front.closing);
    text.push_str(front.body);
    Ok(style.encode(&text))
}
