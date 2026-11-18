//! Bounded reads and guarded writes of the reserved per-space skill directory.
use calternal_content::{FileFormat, Item, Placement, Request};
use calternal_core::{
    JsonObject, SkillDocument, SkillMatch, SkillRemove, SkillResult, SkillTarget, SkillWrite,
    SkillsResult,
};
use calternal_fsio::beneath::Folder;
use calternal_types::{Actor, Error, SpaceId};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, Read};

const DIRECTORIES: usize = 512;
const DESCRIPTORS: usize = 50;
const FRONTMATTER_BYTES: usize = 8192;
const DOCUMENT_BYTES: usize = 256 * 1024;

#[derive(Deserialize)]
struct Frontmatter {
    name: String,
    description: String,
    compatibility: Option<String>,
    #[serde(default)]
    metadata: std::collections::BTreeMap<String, String>,
}

fn directory(store: &crate::store::Store, space: SpaceId) -> io::Result<Folder> {
    Folder::open_existing(&store.content().userdata_root())?
        .open_sub(&format!("spaces/{space}/AI/Skills"), false)
}

pub(super) fn list(
    store: &crate::store::Store,
    space: SpaceId,
    prefix: &str,
) -> Result<Value, Error> {
    let directory = match directory(store, space) {
        Ok(directory) => directory,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return super::to_value(SkillsResult { skills: Vec::new() });
        }
        Err(e) => return Err(storage(e)),
    };
    let mut names = directory.entry_names(DIRECTORIES).map_err(storage)?;
    names.sort_unstable();
    let prefix = prefix.trim().to_ascii_lowercase();
    let mut skills = Vec::new();
    for name in names {
        if skills.len() >= DESCRIPTORS {
            break;
        }
        if !valid_name(&name) || !name.starts_with(&prefix) {
            continue;
        }
        let Ok(mut file) = directory.open_file(&format!("{name}/SKILL.md")) else {
            continue;
        };
        let mut bytes = Vec::new();
        if (&mut file)
            .take(FRONTMATTER_BYTES as u64)
            .read_to_end(&mut bytes)
            .is_err()
        {
            continue;
        }
        // A bounded prefix can end mid-codepoint in the body. Validate UTF-8
        // only through the complete frontmatter terminator.
        let Some((end, marker)) = [(b"\n---\n".as_slice(), 4), (b"\r\n---\r\n".as_slice(), 5)]
            .into_iter()
            .find_map(|(marker, skip)| {
                bytes
                    .windows(marker.len())
                    .skip(skip)
                    .position(|window| window == marker)
                    .map(|end| (end + skip, marker.len()))
            })
        else {
            continue;
        };
        let Ok(front) = std::str::from_utf8(&bytes[..end + marker]) else {
            continue;
        };
        if let Ok(mut descriptor) = descriptor(front, &name) {
            descriptor.id = managed(store, space, &name)?.map(|row| row.id.to_string());
            skills.push(descriptor);
        }
    }
    super::to_value(SkillsResult { skills })
}

fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn check_name(name: &str) -> Result<(), Error> {
    if !valid_name(name) {
        return Err(Error::invalid_input(
            "skill names use up to 64 lowercase ASCII letters, digits, and single hyphens",
        ));
    }
    Ok(())
}

fn descriptor(text: &str, expected: &str) -> Result<SkillMatch, Error> {
    let normalized;
    let text = if text.contains('\r') {
        normalized = text.replace("\r\n", "\n");
        normalized.as_str()
    } else {
        text
    };
    let front = text
        .strip_prefix("---\n")
        .and_then(|text| text.split_once("\n---\n").map(|(front, _)| front))
        .filter(|front| front.len() < FRONTMATTER_BYTES - 9)
        .ok_or_else(|| Error::invalid_input("SKILL.md needs bounded YAML frontmatter"))?;
    let meta: Frontmatter = serde_yaml::from_str(front)
        .map_err(|_| Error::invalid_input("SKILL.md frontmatter is not valid YAML"))?;
    if meta.name != expected || !valid_name(&meta.name) {
        return Err(Error::invalid_input(
            "the skill name must match its directory",
        ));
    }
    if meta.description.trim().is_empty()
        || meta.description.chars().count() > 1024
        || meta
            .compatibility
            .is_some_and(|line| line.chars().count() > 500)
    {
        return Err(Error::invalid_input(
            "invalid description or compatibility length",
        ));
    }
    let agents = meta
        .metadata
        .get("agents")
        .map(|agents| {
            agents
                .split(',')
                .map(str::trim)
                .filter(|agent| !agent.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Ok(SkillMatch {
        name: meta.name,
        description: meta
            .description
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
        agents,
        path: format!("AI/Skills/{expected}/SKILL.md"),
        id: None,
    })
}

fn kind(name: &str) -> String {
    format!("skill-{name}")
}

fn managed(
    store: &crate::store::Store,
    space: SpaceId,
    name: &str,
) -> Result<Option<calternal_content::Summary>, Error> {
    Ok(store.content().list(space, &kind(name), 1)?.pop())
}

fn document(item: &Item, name: &str) -> Result<SkillDocument, Error> {
    let text = std::str::from_utf8(&item.bytes)
        .map_err(|_| Error::internal("invalid stored skill document"))?;
    let mut descriptor = descriptor(text, name)?;
    descriptor.id = Some(item.id.to_string());
    Ok(SkillDocument {
        descriptor,
        markdown: text.to_owned(),
        revision: item.revision.clone(),
    })
}

pub(crate) fn mutate_or_read(
    store: &crate::store::Store,
    space: SpaceId,
    actor: &Actor,
    rid: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let content = store.content();
    if op == "skills.read" {
        let i: SkillTarget = super::decode(input)?;
        check_name(&i.name)?;
        if let Some(row) = managed(store, space, &i.name)? {
            return super::to_value(SkillResult {
                skill: document(&content.get(space, row.id)?, &i.name)?,
            });
        }
        let name = i.name;
        let file = directory(store, space)
            .and_then(|dir| dir.open_file(&format!("{name}/SKILL.md")))
            .map_err(|e| {
                if e.kind() == io::ErrorKind::NotFound {
                    Error::not_found("skill not found")
                } else {
                    storage(e)
                }
            })?;
        let mut markdown = String::new();
        file.take(DOCUMENT_BYTES as u64 + 1)
            .read_to_string(&mut markdown)
            .map_err(storage)?;
        if markdown.len() > DOCUMENT_BYTES {
            return Err(Error::unavailable("SKILL.md exceeds the document limit"));
        }
        let revision = crate::store::sha256_hex(markdown.as_bytes());
        return super::to_value(SkillResult {
            skill: SkillDocument {
                descriptor: descriptor(&markdown, &name)?,
                markdown,
                revision,
            },
        });
    }
    let fp = crate::api::fingerprint(op, &input);
    if let Some(saved) = content.receipt(space, actor.account, rid)? {
        if saved.operation != op || saved.fingerprint != fp {
            return Err(Error::request_reused());
        }
        if let Some(id) = saved.item {
            content.materialize(space, id)?;
        }
        return Ok(saved.result);
    }
    let request = Request {
        account: actor.account,
        via: actor.via.as_ref(),
        id: rid,
        operation: op,
        fingerprint: &fp,
    };
    if op == "skills.remove" {
        let i: SkillRemove = super::decode(input)?;
        check_name(&i.name)?;
        let row = managed(store, space, &i.name)?
            .ok_or_else(|| Error::not_found("managed skill not found"))?;
        let item = content.get(space, row.id)?;
        if i.base_revision != item.revision {
            return Err(Error::conflict("the skill has a newer revision"));
        }
        content.trash(
            &item,
            item.metadata.clone(),
            &item.bytes,
            request,
            |_| json!({"ok":true}),
            || Ok(()),
        )?;
        content.materialize(space, item.id)?;
        return Ok(json!({"ok":true}));
    }
    let i: SkillWrite = super::decode(input)?;
    let name = i.name;
    check_name(&name)?;
    if i.markdown.len() > DOCUMENT_BYTES {
        return Err(Error::invalid_input("SKILL.md exceeds the document limit"));
    }
    let descriptor = descriptor(&i.markdown, &name)?;
    let kind = kind(&name);
    let id = if op == "skills.create" {
        if i.base_revision.is_some() || managed(store, space, &name)?.is_some() {
            return Err(Error::conflict("a skill with that name already exists"));
        }
        content.register_kind(
            space,
            &kind,
            Placement::VisibleTree {
                folder: "AI".into(),
                subdirectories: vec!["Skills".into(), name.clone()],
            },
            FileFormat::Opaque {
                extension: "md".into(),
            },
        )?;
        None
    } else {
        if i.base_revision.is_none() {
            return Err(Error::invalid_input("base_revision is required"));
        }
        Some(
            managed(store, space, &name)?
                .ok_or_else(|| Error::not_found("managed skill not found"))?
                .id,
        )
    };
    let item = content.write(
        space,
        id,
        &kind,
        i.base_revision.as_deref(),
        json!({"title":"SKILL","description":descriptor.description}),
        i.markdown.as_bytes(),
        request,
        |item| json!({"skill":document(item,&name).expect("validated skill document")}),
    )?;
    content.materialize(space, item.id)?;
    super::to_value(SkillResult {
        skill: document(&item, &name)?,
    })
}

fn storage(error: io::Error) -> Error {
    tracing::warn!(%error, "could not read space skills");
    Error::internal("could not read space skills")
}
