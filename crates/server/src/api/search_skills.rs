//! Bounded reads of the reserved per-space skill directory.
use calternal_core::{SkillMatch, SkillsResult};
use calternal_fsio::beneath::Folder;
use calternal_types::{Error, SpaceId};
use serde_json::Value;
use std::io::{self, BufRead, BufReader, Read};

const DIRECTORIES: usize = 512;
const DESCRIPTORS: usize = 50;
const FRONTMATTER_BYTES: u64 = 8192;

pub(super) fn list(
    store: &crate::store::Store,
    space: SpaceId,
    prefix: &str,
) -> Result<Value, Error> {
    let root = Folder::open(&store.content().userdata_root()).map_err(storage)?;
    let directory = match root
        .child("spaces", false)
        .and_then(|spaces| spaces.child(&space.to_string(), false))
        .and_then(|space| space.child("AI", false))
        .and_then(|ai| ai.child("Skills", false))
    {
        Ok(directory) => directory,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return super::to_value(SkillsResult { skills: Vec::new() });
        }
        Err(e) => return Err(storage(e)),
    };
    let mut names = directory.entry_names(DIRECTORIES).map_err(storage)?;
    names.sort_unstable();
    let prefix = calternal_content::fold_title(prefix.trim());
    let mut skills = Vec::new();
    for name in names {
        if skills.len() >= DESCRIPTORS {
            break;
        }
        if !valid_name(&name) || !name.starts_with(&prefix) {
            continue;
        }
        let Ok(file) = directory
            .child(&name, false)
            .and_then(|dir| dir.open_file("SKILL.md"))
        else {
            continue;
        };
        if let Ok(Some(description)) =
            description(BufReader::new(file.take(FRONTMATTER_BYTES)), &name)
        {
            skills.push(SkillMatch { name, description });
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

fn description(reader: impl BufRead, expected_name: &str) -> io::Result<Option<String>> {
    let mut lines = reader.lines();
    if lines.next().transpose()?.as_deref() != Some("---") {
        return Ok(None);
    }
    let mut name = None;
    let mut description = None;
    for line in lines {
        let line = line?;
        if line == "---" {
            return Ok(description.filter(|_| name.as_deref() == Some(expected_name)));
        }
        if let Some(value) = line.strip_prefix("name:") {
            name = scalar(value);
        } else if let Some(value) = line.strip_prefix("description:") {
            description = scalar(value).filter(|v| {
                (1..=1024).contains(&v.chars().count()) && !v.chars().any(char::is_control)
            });
        }
    }
    Ok(None)
}

fn scalar(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.starts_with('"') {
        serde_json::from_str::<String>(value).ok()
    } else if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        Some(inner.replace("''", "'"))
    } else if !value.starts_with(['|', '>', '\'']) {
        Some(
            value
                .split_once(" #")
                .map_or(value, |(text, _)| text)
                .trim_end()
                .to_owned(),
        )
    } else {
        None
    }
}

fn storage(error: io::Error) -> Error {
    tracing::warn!(%error, "could not list space skills");
    Error::internal("could not list space skills")
}
