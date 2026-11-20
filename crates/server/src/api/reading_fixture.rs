//! Explicit development-only install of the SDK's pinned Reading List example.
//! Only the checked member list is admitted. No worker is installed or executed.

use super::{Shared, Value, window_views::Binding};
use calternal_auth::sharing::Audience;
use calternal_content::{
    FileFormat, Placement, Request, WindowField, WindowKindOrder, WindowOrder, WindowScope,
};
use serde_json::json;

const MANIFEST: &[u8] = include_bytes!("../../tests/fixtures/v2-reading-list/manifest.json");
const TEMPLATE_NAME: &str =
    "app-reading-2304bcde509a0e500cd804eca2bd1a4aa02b81c64069c55cf3c623e0c0c81722.json";
const TEMPLATE: &[u8] = include_bytes!(
    "../../tests/fixtures/v2-reading-list/app-reading-2304bcde509a0e500cd804eca2bd1a4aa02b81c64069c55cf3c623e0c0c81722.json"
);

const PACKAGE: &str = "calternal-reading-list";
const KIND: &str = "reading-item";

pub(super) fn install(state: &Shared) -> Result<Binding, String> {
    // This is the parser used before the distribution installer can persist a
    // package. Keep the explicit fixture until that loader accepts v2 itself.
    if calternal_plugin_host::parse_manifest(MANIFEST).is_ok() {
        return Err("remove the Reading List fixture bypass once the installer accepts it".into());
    }
    let rejection = match serde_json::from_slice::<calternal_core::ManifestV1>(MANIFEST) {
        Err(error) => error.to_string(),
        Ok(manifest) => calternal_plugin_host::validate_manifest_detailed(&manifest)
            .expect_err("ordinary loader refused the manifest"),
    };
    tracing::info!(%rejection, package = PACKAGE, "ordinary manifest loader refused the v2 fixture");
    let (binding, order) = compile()?;
    let auth = &state.directory.spaces;
    let ids = auth.seed_development().map_err(|e| e.to_string())?;
    let content = state.store.content();
    for space in [ids.personal, ids.shared] {
        content
            .register_kind(
                space,
                KIND,
                Placement::Hidden {
                    plugin_id: PACKAGE.into(),
                },
                FileFormat::Markdown,
            )
            .map_err(|e| e.message)?;
        content
            .configure_window_kind(space, KIND, &order)
            .map_err(|e| e.message)?;
        for (index, (title, added, read)) in [
            ("First article", "2026-11-19T10:00:00.000Z", false),
            ("Second article", "2026-11-19T12:00:00.000Z", true),
            ("Third article", "2026-11-19T11:00:00.000Z", false),
        ]
        .into_iter()
        .enumerate()
        {
            let key = format!("v2-reading-list-fixture-{index}");
            if content
                .receipt(space, ids.owner, &key)
                .map_err(|e| e.message)?
                .is_none()
            {
                let item = content.write(
                    space, None, KIND, None,
                    json!({"title":title,"added":added,"read":read,"url":"https://example.invalid/private"}),
                    b"Disposable reading list fixture",
                    Request { account: ids.owner, via: None, id: &key, operation: "fixture.reading-list", fingerprint: &key },
                    |item| json!({"id":item.id}),
                ).map_err(|e| e.message)?;
                content.materialize(space, item.id).map_err(|e| e.message)?;
            }
        }
    }
    // The current protected grant schema admits Notes and collections, so
    // demonstrate item-only authority in the same space with a real Note.
    let note_key = "v2-reading-list-fixture-guest-note";
    content
        .register_kind(
            ids.personal,
            "notes",
            Placement::Visible {
                folder: "Notes".into(),
            },
            FileFormat::Markdown,
        )
        .map_err(|e| e.message)?;
    let note = if let Some(receipt) = content
        .receipt(ids.personal, ids.owner, note_key)
        .map_err(|e| e.message)?
    {
        receipt.item.ok_or("fixture receipt has no item")?
    } else {
        let note = content
            .write(
                ids.personal,
                None,
                "notes",
                None,
                json!({"title":"Individually shared control note"}),
                b"An unrelated item-only grant confers no Reading List authority",
                Request {
                    account: ids.owner,
                    via: None,
                    id: note_key,
                    operation: "fixture.reading-control",
                    fingerprint: note_key,
                },
                |item| json!({"id":item.id}),
            )
            .map_err(|e| e.message)?;
        content
            .materialize(ids.personal, note.id)
            .map_err(|e| e.message)?;
        note.id
    };
    if auth
        .item_authority_blocking(ids.guest, ids.personal, note, Vec::new())
        .map_err(|e| e.to_string())?
        .1
        == 0
    {
        auth.grant_item(
            ids.owner,
            ids.personal,
            note,
            "notes".into(),
            Audience::Account(ids.guest),
            false,
            (
                "v2-reading-list-fixture-grant",
                "fixture.reading-grant",
                "v2-reading-list-fixture-grant",
            ),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(binding)
}

fn compile() -> Result<(Binding, WindowKindOrder), String> {
    let manifest: Value = serde_json::from_slice(MANIFEST).map_err(|e| e.to_string())?;
    let template = &manifest["apps"][0]["description"]["template"];
    if manifest["id"] != PACKAGE
        || manifest["api_version"] != 2
        || !manifest["reads"]
            .as_array()
            .is_some_and(|reads| reads.iter().any(|kind| kind == KIND))
        || template["path"] != TEMPLATE_NAME
        || template["sha256"] != super::sha256_hex(TEMPLATE)
    {
        return Err("Reading List fixture identity or template pin does not match".into());
    }
    let kind = manifest["kinds"]
        .as_array()
        .and_then(|kinds| kinds.iter().find(|kind| kind["id"] == KIND))
        .ok_or("missing fixture kind")?;
    if kind["claimed_text"] != true
        || kind["storage"] != json!({"hidden":true})
        || kind["title_field"] != "title"
    {
        return Err("unsupported Reading List fixture kind".into());
    }
    let description: Value = serde_json::from_slice(TEMPLATE).map_err(|e| e.to_string())?;
    let bind = description["binds"]
        .as_array()
        .and_then(|binds| binds.iter().find(|bind| bind["id"] == "items"))
        .ok_or("missing fixture items Bind")?;
    if bind["source"] != json!({"type":"list","kind":KIND})
        || bind.get("filter").is_some()
        || bind["window"]["count"] != "exact"
        || bind["sort"] != json!([{"field":"added","direction":"desc"}])
    {
        return Err("unqualified Reading List fixture plan".into());
    }
    let mut fields = Vec::new();
    let mut projected = Vec::new();
    for field in bind["fields"].as_array().ok_or("missing fixture fields")? {
        let field = field.as_str().ok_or("invalid fixture field")?;
        projected.push(field.to_owned());
        if field == "title" {
            continue;
        }
        let indexed = &kind["indexed_fields"][field];
        if indexed["audience"] != "member_item" {
            return Err("fixture field must belong to member items".into());
        }
        fields.push(match indexed["type"].as_str() {
            Some("Timestamp") => WindowField::Timestamp(field.into()),
            Some("Boolean") => WindowField::Boolean(field.into()),
            _ => return Err("unqualified Reading List fixture field".into()),
        });
    }
    if projected != ["title", "added", "read"] {
        return Err("unexpected Reading List fixture field projection".into());
    }
    let order = WindowOrder {
        field: "added".into(),
        descending: true,
    };
    Ok((
        Binding {
            bind: "items".into(),
            fields: projected,
            scope: WindowScope::Kind {
                kind: KIND.into(),
                order,
                fields,
            },
            description: Some(description),
        },
        WindowKindOrder {
            title_field: "title".into(),
            sort_field: "added".into(),
            parent_field: None,
        },
    ))
}
