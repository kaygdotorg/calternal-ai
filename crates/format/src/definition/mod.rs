use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Diagnostic, Expression,
    expr::parse,
    lines::{self, Document, Line, Site},
};

mod collection;
mod dav;
mod formulas;
mod install;
mod kind;
mod line_claims;
pub use dav::{DavComponent, DavMapping, DavProperty};
pub use line_claims::{CheckboxMapping, LineClaim, LineForm};
mod manifest;
mod places;
mod secondary;

pub use collection::{Collection, CollectionView, ShareSource, Sort, Source, Total};
pub use kind::{Editability, ItemForm, Kind, PageEntry, Rights};
pub use manifest::{Manifest, Program, Refusal};
pub use places::{ComposerBindings, Fill, FillBinding, Slot};
pub use secondary::{AgentRule, AgentRules, Sidebar, SidebarEntry, parse_agent_rules};

/// One supplied entry in a plugin folder.
#[derive(Clone, Copy, Debug)]
pub struct PluginFile<'a> {
    /// Normalized plugin-relative path with forward slash separators.
    pub path: &'a str,
    /// UTF-8 text if available. Definitions and templates require it.
    pub text: Option<&'a str>,
}

/// Host-attested source policy and existing definitions used at install.
#[derive(Clone, Copy, Debug, Default)]
pub struct InstallOptions<'a> {
    /// ID the host expects from this source, if already established.
    pub expected_id: Option<&'a str>,
    /// Official IDs and display names, from the operator's trusted source list.
    pub official: &'a [(&'a str, &'a str)],
    /// Definitions already installed in the same space.
    pub installed: &'a [Plugin],
}

/// A plugin folder checked as one unit, ready for the host to install.
#[derive(Clone, Debug)]
pub struct Plugin {
    /// Identity and declarations from plugin.md.
    pub manifest: Manifest,
    /// Kinds keyed by their plugin-local ID.
    pub kinds: BTreeMap<String, Kind>,
    /// Collections keyed by ID.
    pub collections: BTreeMap<String, Collection>,
    /// Sidebars keyed by ID.
    pub sidebars: BTreeMap<String, Sidebar>,
    /// Template bodies keyed by their filename stem.
    pub templates: BTreeMap<String, String>,
    /// Whether this plugin qualifies for non-Owner installation.
    pub description_only: bool,
}

/// A checked definition directive passed on to a host surface.
#[derive(Clone, Debug)]
pub struct Directive {
    /// The directive's declared name. Section lines may use an empty name.
    pub key: String,
    /// Value as written, without its surrounding comment.
    pub value: String,
    /// Nested declaration lines in source order.
    pub children: Vec<Directive>,
    pub(crate) site: Site,
    pub(crate) fill: Option<Box<Fill>>,
}

/// Parse a plugin's definition files and check relationships within the folder.
/// The caller attests that supplied files are regular files inside the folder.
pub fn parse_plugin(
    files: &[PluginFile<'_>],
    options: &InstallOptions<'_>,
) -> Result<Plugin, Diagnostic> {
    let paths = files
        .iter()
        .map(|file| {
            path(file.path, Site::START).map_err(|error| located(error, file.path))?;
            Ok(file.path)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if files.len() != paths.len() {
        return Err(Site::START.error("List each plugin file once"));
    }
    let manifest_file = files
        .iter()
        .find(|file| file.path == "plugin.md")
        .ok_or_else(|| {
            located(
                Site::START.error("Include plugin.md in the plugin folder"),
                "plugin.md",
            )
        })?;
    let manifest =
        manifest::parse(manifest_file, options).map_err(|error| located(error, "plugin.md"))?;
    let mut plugin = Plugin {
        description_only: manifest.is_description_only()
            && !files.iter().any(|file| {
                file.path == "operations.ts"
                    || file.path.starts_with("frames/")
                    || file.text.is_some_and(html)
                    || file.path.rsplit_once('.').is_some_and(|(_, ext)| {
                        matches!(
                            ext.to_ascii_lowercase().as_str(),
                            "js" | "mjs" | "cjs" | "ts" | "html" | "htm" | "svg"
                        )
                    })
            }),
        manifest,
        kinds: BTreeMap::new(),
        collections: BTreeMap::new(),
        sidebars: BTreeMap::new(),
        templates: BTreeMap::new(),
    };
    for file in files {
        let Some((folder, filename)) = file.path.split_once('/') else {
            continue;
        };
        if !matches!(folder, "kinds" | "collections" | "sidebars" | "templates") {
            continue;
        }
        let parse = || -> Result<_, Diagnostic> {
            let stem = filename
                .strip_suffix(".md")
                .ok_or_else(|| Site::START.error("Use a Markdown definition with a .md suffix"))?;
            lines::id(stem, Site::START)?;
            let text = file
                .text
                .ok_or_else(|| Site::START.error("Supply the definition as UTF-8 text"))?;
            match folder {
                "kinds" => {
                    plugin.kinds.insert(
                        stem.into(),
                        kind::parse(text, stem, &plugin.manifest.settings)?,
                    );
                }
                "collections" => {
                    plugin
                        .collections
                        .insert(stem.into(), collection::parse(text, stem)?);
                }
                "sidebars" => {
                    plugin
                        .sidebars
                        .insert(stem.into(), secondary::sidebar(text, stem)?);
                }
                "templates" => {
                    plugin.templates.insert(stem.into(), text.into());
                }
                _ => unreachable!(),
            }
            Ok(())
        };
        let mut parse = parse;
        parse().map_err(|error| located(error, file.path))?;
    }
    install::verify(&plugin, &paths, options)?;
    Ok(plugin)
}

fn html(text: &str) -> bool {
    text.split('<').skip(1).any(|tail| {
        let tail = tail.strip_prefix('/').unwrap_or(tail);
        let name_end = tail
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(tail.len());
        tail.starts_with(['!', '?'])
            || (name_end > 0
                && tail.as_bytes()[0].is_ascii_alphabetic()
                && tail
                    .get(name_end..)
                    .is_some_and(|after| after.starts_with([' ', '\t', '/', '>'])))
    })
}

pub(super) fn located(mut error: Diagnostic, path: &str) -> Diagnostic {
    error.path = Some(path.into());
    error
}

pub(super) fn path(value: &str, site: Site) -> Result<(), Diagnostic> {
    if value.is_empty()
        || value.contains(['\\', '\0'])
        || value
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || value.starts_with(['~', '/'])
        || value.get(..2).is_some_and(|text| text.ends_with(':'))
        || !lines::is_nfc(value)
        || value.chars().any(lines::forbidden)
    {
        return Err(site.error("Use a normalized relative path without . or .. components"));
    }
    Ok(())
}

pub(super) fn text(
    value: &str,
    site: Site,
    limit: usize,
    multiline: bool,
) -> Result<String, Diagnostic> {
    let value = if multiline {
        value.into()
    } else {
        lines::unquote(value, site)?
    };
    let length = if multiline {
        value.len()
    } else {
        value.chars().count()
    };
    if length > limit
        || !lines::is_nfc(&value)
        || value
            .chars()
            .any(|ch| lines::forbidden(ch) && !(multiline && matches!(ch, '\n' | '\r')))
    {
        return Err(site.error(format!(
            "Use normalized text of at most {limit} characters without controls"
        )));
    }
    Ok(value)
}

fn icon(value: &str, site: Site) -> Result<String, Diagnostic> {
    lines::id(value, site)?;
    Ok(value.into())
}

pub(super) fn name(value: &str, site: Site) -> Result<String, Diagnostic> {
    let value = text(value, site, 40, false)?;
    #[cfg(feature = "unicode")]
    {
        use unicode_security::MixedScript as _;
        if !value.as_str().is_single_script() {
            return Err(site.error("Use a single script in a display name"));
        }
    }
    if value.trim().is_empty() {
        return Err(site.error("Give the definition a display name"));
    }
    Ok(value)
}

pub(super) fn definition(source: &str) -> Result<Document<'_>, Diagnostic> {
    lines::document(source)
}

pub(super) struct Section<'a> {
    pub key: &'a str,
    pub value: &'a str,
    pub site: Site,
    pub line: &'a Line,
    pub children: &'a [Line],
}

impl Section<'_> {
    pub fn alone(&self) -> Result<(), Diagnostic> {
        if let Some(child) = self.children.first() {
            Err(child
                .site
                .error(format!("{} does not take nested lines", self.key)))
        } else {
            Ok(())
        }
    }

    pub fn block(&self) -> Result<(), Diagnostic> {
        if !self.value.is_empty() {
            Err(self
                .site
                .error(format!("Write the {} entries on nested lines", self.key)))
        } else {
            Ok(())
        }
    }

    pub fn expression(&self, formula: &str) -> Result<Expression, Diagnostic> {
        let start = formula.as_ptr() as usize - self.line.text.as_ptr() as usize;
        parse::parse_at(formula, |offset| self.line.site_at(start + offset))
    }

    pub fn directive(&self) -> Result<Directive, Diagnostic> {
        Ok(Directive {
            key: self.key.into(),
            value: self.value.into(),
            children: directives(self.children)?,
            site: self.line.site,
            fill: None,
        })
    }
}

pub(super) fn sections(lines: &[Line], depth: usize) -> Result<Vec<Section<'_>>, Diagnostic> {
    let mut output = Vec::new();
    let mut at = 0;
    let mut seen = BTreeSet::new();
    while let Some(line) = lines.get(at) {
        if line.depth != depth {
            return Err(line
                .site
                .error("Use one two-space step after the containing key"));
        }
        let (key, value, site) = line.entry()?.ok_or_else(|| {
            line.site
                .error("Write a definition key followed by a colon")
        })?;
        if key.starts_with("- ") {
            return Err(line.site.error("A list entry is not allowed here"));
        }
        if !seen.insert(key) {
            return Err(line.site.error(format!("The key {key} is written twice")));
        }
        let end = at
            + 1
            + lines[at + 1..]
                .iter()
                .take_while(|line| line.depth > depth)
                .count();
        output.push(Section {
            key,
            value,
            site,
            line,
            children: &lines[at + 1..end],
        });
        at = end;
    }
    Ok(output)
}

pub(super) fn directives(lines: &[Line]) -> Result<Vec<Directive>, Diagnostic> {
    let mut output = Vec::new();
    let mut at = 0;
    while let Some(line) = lines.get(at) {
        let end = at
            + 1
            + lines[at + 1..]
                .iter()
                .take_while(|child| child.depth > line.depth)
                .count();
        let (key, value) = line.entry()?.map_or_else(
            || line.content().map(|value| ("", value)),
            |(key, value, _)| Ok((key, value)),
        )?;
        if key.starts_with("- ") || value.starts_with("- ") {
            return Err(line.site.error("A list entry is not allowed here"));
        }
        output.push(Directive {
            key: key.into(),
            value: value.into(),
            children: directives(&lines[at + 1..end])?,
            site: line.site,
            fill: None,
        });
        at = end;
    }
    Ok(output)
}

pub(super) fn format(value: &str, site: Site) -> Result<(), Diagnostic> {
    if value == "1" {
        Ok(())
    } else {
        Err(site.error("This engine understands format: 1"))
    }
}

pub(super) fn list(value: &str, site: Site) -> Result<Vec<String>, Diagnostic> {
    lines::parts(value, site)?
        .into_iter()
        .map(|(part, site)| lines::unquote(part, site))
        .collect()
}

pub(super) fn required(sections: &[Section<'_>], keys: &[&str]) -> Result<(), Diagnostic> {
    for key in keys {
        if !sections.iter().any(|section| section.key == *key) {
            return Err(Site::START.error(format!("Include the required {key} key")));
        }
    }
    Ok(())
}

pub(super) fn matching_id(section: &Section<'_>, expected: &str) -> Result<(), Diagnostic> {
    if section.value != expected {
        return Err(section.site.error(format!(
            "The {} must match the filename {expected}.md",
            section.key
        )));
    }
    Ok(())
}
