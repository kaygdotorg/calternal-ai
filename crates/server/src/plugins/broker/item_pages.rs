//! Indexed kind pages scoped to the invocation's package and authority.

use super::{Capability, Directory, Error, SpaceHost};
use crate::api::cursors::CursorScope;
use calternal_content::ItemOrder;
use calternal_core::{JsonObject, PluginItemPageInput, PluginItemPageResult, PluginItemSummary};
use serde::Deserialize;
use serde_json::{Value, json};

const PAGE_LIMIT: u16 = 100;
const TITLE_LIMIT: usize = 256;

impl<D: Directory> SpaceHost<D> {
    /// Page a declared kind with one indexed read and no body loads or writes.
    /// The cursor signs account, package, space and filters before reuse.
    pub(super) fn list_items(&self, input: &Value) -> Result<Value, Error> {
        let input = PluginItemPageInput::deserialize(input).map_err(|_| Error::Invalid)?;
        let limit = input.limit.unwrap_or(PAGE_LIMIT);
        let label = input.label.as_deref();
        if !(1..=PAGE_LIMIT).contains(&limit)
            || label.is_some_and(|label| {
                label.is_empty() || label.len() > TITLE_LIMIT || label.chars().any(char::is_control)
            })
        {
            return Err(Error::Invalid);
        }
        if !self.plugin.declares_kind(&input.kind)
            || !self.has(Capability::SpaceRead)
            || self.kind_actor(&input.kind, false).is_none()
        {
            return Err(Error::Refused);
        }
        let mut scoped = JsonObject::from([
            ("space_id".into(), json!(self.space)),
            ("plugin_id".into(), json!(self.plugin.manifest.id)),
            ("kind".into(), json!(input.kind)),
            ("label".into(), json!(label)),
        ]);
        if let Some(cursor) = input.cursor {
            scoped.insert("cursor".into(), Value::String(cursor));
        }
        let scope = CursorScope::open(
            &self.state.store.cursor_key,
            self.identity.id,
            "list_items_v1",
            &mut scoped,
        )
        .map_err(|_| Error::Invalid)?;
        let page = self
            .state
            .store
            .content()
            .item_page(
                self.space,
                &input.kind,
                ItemOrder::Updated { label },
                scoped.get("cursor").and_then(Value::as_str),
                usize::from(limit),
            )
            .map_err(|_| Error::Refused)?;
        let items = page
            .rows
            .into_iter()
            .map(|row| PluginItemSummary {
                id: row.id.to_string(),
                revision: row.revision,
                updated_at: row.updated_at,
                title: row
                    .metadata
                    .get("title")
                    .and_then(Value::as_str)
                    .map(|title| {
                        title
                            .chars()
                            .filter(|c| !c.is_control())
                            .take(TITLE_LIMIT)
                            .collect()
                    }),
            })
            .collect();
        let mut cursor = json!(page.next);
        scope.seal(&mut cursor);
        serde_json::to_value(PluginItemPageResult {
            items,
            next_cursor: cursor.as_str().map(str::to_owned),
        })
        .map_err(|_| Error::Invalid)
    }
}
