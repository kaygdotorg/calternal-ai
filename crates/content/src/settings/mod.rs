//! Recoverable space settings. A blocked settings intent is local to settings.
mod document;

use super::*;
use calternal_fsio::beneath::Folder;
use document::{Document, FORMAT, MAX_ALTERNATIVES, MAX_FILE_BYTES, MAX_VALUE_BYTES};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

const DIRECTORY: &str = ".calternal";
const FILE: &str = "settings.json";
const RELATIVE_FILE: &str = ".calternal/settings.json";
const PENDING: &str = "plugin-settings.intent/v1";

/// A space's stored value and the offline edits retained for manual resolution.
pub struct SettingRecord {
    /// Override. Null means use the declared default.
    pub value: Value,
    /// Other proposed values for the same key.
    pub alternatives: Vec<Value>,
    /// Caller-safe repair hint when writes are unavailable.
    pub repair: Option<String>,
}

/// A compare-and-merge edit for one manifest-declared key.
pub struct SettingEdit<'a> {
    /// Installed package ID.
    pub plugin: &'a str,
    /// Manifest setting ID.
    pub key: &'a str,
    /// Override seen when editing began. Null represents no override.
    pub base: &'a Value,
    /// Proposed override. Null clears the override.
    pub value: &'a Value,
    /// Explicit resolution, echoing the alternatives already seen.
    pub resolve: Option<&'a [Value]>,
}

#[derive(Deserialize, Serialize)]
struct Intent {
    account: AccountId,
    request: String,
    operation: String,
    fingerprint: String,
    before: Option<String>,
    after: String,
    change: Value,
    base: Value,
    value: Value,
    resolve: Option<Vec<Value>>,
}

fn apply(text: Option<&str>, edit: &SettingEdit<'_>) -> Result<(String, bool)> {
    let mut document = decode(text)?;
    let qualified = format!("{}/{}", edit.plugin, edit.key);
    let conflicted = document.edit(&qualified, edit.base, edit.value, edit.resolve)?;
    let after = document.encode().map_err(internal)?;
    if after.len() > MAX_FILE_BYTES {
        return Err(Error::invalid_input("settings would exceed 1 MiB"));
    }
    Ok((String::from_utf8(after).map_err(internal)?, conflicted))
}

fn repair(line: usize, why: &str) -> Error {
    let line = line.max(1);
    Error::conflict(format!(
        "Repair {RELATIVE_FILE} at line {line}: {why}. Then retry."
    ))
    .with_details(serde_json::json!({"reason":"settings_repair","file":RELATIVE_FILE,"line":line}))
}

fn folder(root: &Path, id: SpaceId, create: bool) -> std::io::Result<Folder> {
    Folder::open(root)?
        .child("spaces", create)?
        .child(&id.to_string(), create)?
        .child(DIRECTORY, create)
}

fn image(root: &Path, space: &db::Space) -> Result<Option<String>> {
    let dir = match folder(root, space.id, false) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        result => {
            result.map_err(|_| repair(1, "use a regular file in the space settings folder"))?
        }
    };
    dir.check_replacement(FILE)
        .map_err(|_| repair(1, "use a regular settings file"))?;
    // The path is composed solely from trusted roots, the admitted UUID and
    // fixed host names. The folder and destination are checked before recovery.
    calternal_fsio::recover_exact(&space.files.join(RELATIVE_FILE))
        .map_err(|_| repair(1, "finish recovering the interrupted file exchange"))?;
    let file = match dir.open_file(FILE) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        result => result.map_err(internal)?,
    };
    let mut text = String::new();
    file.take((MAX_FILE_BYTES + 1) as u64)
        .read_to_string(&mut text)
        .map_err(|_| repair(1, "use UTF-8 JSON"))?;
    if text.len() > MAX_FILE_BYTES {
        return Err(repair(1, "reduce the file to 1 MiB or less"));
    }
    Ok(Some(text))
}

fn decode(text: Option<&str>) -> Result<Document> {
    text.map(|text| {
        Document::read(text.as_bytes()).map_err(|err| {
            repair(
                err.line(),
                &format!("use valid {FORMAT} JSON with unique keys"),
            )
        })
    })
    .unwrap_or_else(|| Ok(Document::default()))
}

fn pending(space: &db::Space) -> Result<Option<Intent>> {
    space
        .conn
        .query_row(
            "SELECT value FROM controls WHERE key=?1",
            [PENDING],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(internal)?
        .map(|value| serde_json::from_str(&value).map_err(internal))
        .transpose()
}

impl Content {
    /// Read one setting under its space lock, recovering any committed settings-only
    /// intent first. A normal read uses one indexed intent lookup and one bounded file
    /// read with no flush. A damaged file returns a repair hint and uses defaults.
    pub fn plugin_setting(&self, id: SpaceId, plugin: &str, key: &str) -> Result<SettingRecord> {
        self.with_space(id, |space| {
            let outcome = self.settings_document(space);
            let result = outcome.and_then(|(_, document)| {
                let key = format!("{plugin}/{key}");
                Ok(SettingRecord {
                    value: document.value(&key),
                    alternatives: document
                        .alternatives(&key)
                        .map_err(|err| repair(err.line(), "repair the conflict array"))?,
                    repair: None,
                })
            });
            match result {
                Err(error) if error.code == ErrorCode::Conflict => Ok(SettingRecord {
                    value: Value::Null,
                    alternatives: Vec::new(),
                    repair: Some(error.message),
                }),
                other => other,
            }
        })
    }

    /// Save a scoped edit or retain its alternative when the recorded base is stale.
    /// The space lock serializes the bounded intent, file exchange and receipt across
    /// syncs. A failed exchange keeps one intent and blocks only settings in that space.
    pub fn edit_plugin_setting(
        &self,
        id: SpaceId,
        request: Request<'_>,
        edit: SettingEdit<'_>,
    ) -> Result<Value> {
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
        if edit.base.to_string().len() > MAX_VALUE_BYTES
            || edit.resolve.is_some_and(|values| {
                values.len() > MAX_ALTERNATIVES
                    || values.iter().any(|v| v.to_string().len() > MAX_VALUE_BYTES)
            })
        {
            return Err(Error::invalid_input(
                "setting base or alternatives exceed the edit bounds",
            ));
        }
        self.with_space(id, |space| {
            self.finish_settings(space)?;
            if let Some(receipt) = self.journals.receipt(&space.journal, request.account, request.id)? {
                return if receipt.operation == request.operation && receipt.fingerprint == request.fingerprint {
                    Ok(receipt.result)
                } else {
                    Err(Error::request_reused())
                };
            }

            let before = image(&self.roots.userdata_ssd, space)?;
            let (after, conflicted) = apply(before.as_deref(), &edit)?;
            let change = serde_json::json!({ "plugin_id": edit.plugin, "key": edit.key, "conflicted": conflicted });
            let intent = Intent {
                account: request.account,
                request: request.id.into(),
                operation: request.operation.into(),
                fingerprint: request.fingerprint.into(),
                before,
                after,
                change,
                base: edit.base.clone(),
                value: edit.value.clone(),
                resolve: edit.resolve.map(<[Value]>::to_vec),
            };
            space.conn.execute("INSERT INTO controls VALUES(?1,?2)", params![PENDING, serde_json::to_string(&intent).map_err(internal)?])
                .map_err(internal)?;
            (self.fault)(FaultPoint::AfterCommit);
            self.publish_settings(space, intent)
        })
    }

    fn settings_document(&self, space: &db::Space) -> Result<(Option<String>, Document)> {
        self.finish_settings(space)?;
        let text = image(&self.roots.userdata_ssd, space)?;
        let document = decode(text.as_deref())?;
        Ok((text, document))
    }

    fn finish_settings(&self, space: &db::Space) -> Result<()> {
        if let Some(intent) = pending(space)? {
            self.publish_settings(space, intent)?;
        }
        Ok(())
    }

    fn publish_settings(&self, space: &db::Space, mut intent: Intent) -> Result<Value> {
        let existing = self
            .journals
            .receipt(&space.journal, intent.account, &intent.request)?;
        if let Some(existing) = &existing
            && (existing.operation != intent.operation
                || existing.fingerprint != intent.fingerprint)
        {
            return Err(Error::request_reused());
        }
        if existing.is_none() {
            let current = image(&self.roots.userdata_ssd, space)?;
            if current.as_deref() != Some(&intent.after) {
                if current != intent.before {
                    // Keep later hand edits and reapply just the pending key.
                    // Persist this exact pre/post image before its next exchange.
                    let (after, conflicted) = apply(
                        current.as_deref(),
                        &SettingEdit {
                            plugin: intent.change["plugin_id"]
                                .as_str()
                                .ok_or_else(|| internal("invalid setting intent package"))?,
                            key: intent.change["key"]
                                .as_str()
                                .ok_or_else(|| internal("invalid setting intent key"))?,
                            base: &intent.base,
                            value: &intent.value,
                            resolve: intent.resolve.as_deref(),
                        },
                    )?;
                    intent.before = current;
                    intent.after = after;
                    intent.change["conflicted"] = Value::Bool(conflicted);
                    space
                        .conn
                        .execute(
                            "UPDATE controls SET value=?2 WHERE key=?1",
                            params![PENDING, serde_json::to_string(&intent).map_err(internal)?],
                        )
                        .map_err(internal)?;
                }
                folder(&self.roots.userdata_ssd, space.id, true).map_err(internal)?;
                let prepared = calternal_fsio::Prepared::new(
                    &space.files.join(RELATIVE_FILE),
                    intent.after.as_bytes(),
                )
                .map_err(internal)?;
                if prepared
                    .place_exact_recoverable(intent.before.as_deref().map(str::as_bytes))
                    .map_err(internal)?
                {
                    return Err(repair(
                        1,
                        "finish editing the settings file before retrying",
                    ));
                }
            }
            self.journals.publish(
                &space.journal,
                intent.account,
                None,
                &intent.request,
                &intent.operation,
                &intent.fingerprint,
                None,
                PENDING,
                intent.change.clone(),
            )?;
        }
        space
            .conn
            .execute("DELETE FROM controls WHERE key=?1", [PENDING])
            .map_err(internal)?;
        Ok(intent.change)
    }
}
