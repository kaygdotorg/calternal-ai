use super::{definition, format, list, matching_id, name, path, required, sections};
use crate::{
    Diagnostic, Expression,
    expr::parse,
    lines::{self, Site},
};

/// A plugin's ordered default sidebar.
#[derive(Clone, Debug)]
pub struct Sidebar {
    /// Sidebar ID local to the plugin.
    pub id: String,
    /// Title shown for the sidebar.
    pub title: String,
    /// Sidebar lines in display order.
    pub entries: Vec<SidebarEntry>,
}

/// One parsed entry in a sidebar.
#[derive(Clone, Debug)]
pub struct SidebarEntry {
    /// The original directive including the Markdown list prefix.
    pub source: String,
    /// Validated title of a section or collection link, if written.
    pub title: Option<String>,
    /// Referenced collection, when this entry opens one.
    pub collection: Option<String>,
    /// The local collection ID and optional owning plugin, as resolved from link syntax.
    pub collection_ref: Option<(String, Option<String>)>,
    /// Optional catalogue icon token on this entry, before collection fallback.
    pub icon: Option<String>,
    pub(crate) site: Site,
}

pub(super) fn sidebar(source: &str, id: &str) -> Result<Sidebar, Diagnostic> {
    let document = definition(source)?;
    let header = sections(&document.header, 0)?;
    required(&header, &["sidebar", "title", "format"])?;
    let mut result = Sidebar {
        id: id.into(),
        title: String::new(),
        entries: Vec::new(),
    };
    for entry in &header {
        entry.alone()?;
        match entry.key {
            "sidebar" => matching_id(entry, id)?,
            "title" => result.title = name(entry.value, entry.site)?,
            "format" => format(entry.value, entry.site)?,
            key => return Err(entry.line.site.error(format!("Unknown sidebar key {key}"))),
        }
    }
    for (source, site) in body_lines(document.body, document.body_site)? {
        result
            .entries
            .push(sidebar_entry(source, site, "../collections/")?);
    }
    Ok(result)
}

pub(super) fn sidebar_entry(
    source: &str,
    site: Site,
    relative: &str,
) -> Result<SidebarEntry, Diagnostic> {
    let line = source
        .strip_prefix("- ")
        .ok_or_else(|| site.error("Begin each sidebar entry with - and a space"))?;
    let mut icon = None;
    let mut collection_ref = None;
    let (collection, title) = if let Some(title) = line.strip_prefix("section: ") {
        super::text(title, site, 40, false)?;
        (None, Some(title.to_owned()))
    } else if let Some(folders) = line.strip_prefix("folders: ") {
        for folder in list(folders, site)? {
            path(&folder, site)?
        }
        (None, None)
    } else if let Some(each) = line.strip_prefix("each: ") {
        let (source, target) = each
            .rsplit_once(" -> ")
            .ok_or_else(|| site.error("Give each a collection after ->"))?;
        let target = target
            .split_once(" by ")
            .map_or(target, |(collection, _)| collection);
        lines::id(target, site)?;
        for (part, site) in lines::parts(source, site)? {
            if let Some(where_clause) = part.strip_prefix("where ") {
                parse::parse(where_clause, site)?;
            }
        }
        collection_ref = Some((target.into(), None));
        (Some(target.into()), None)
    } else {
        let (title, target) = line
            .strip_prefix('[')
            .and_then(|line| line.split_once("]("))
            .ok_or_else(|| site.error("Write a section, folders, each, or a collection link"))?;
        super::text(title, site, 40, false)?;
        let (target, suffix) = target
            .split_once(')')
            .ok_or_else(|| site.error("Close the sidebar link with )"))?;
        let suffix_site = site.after(&source[..source.len() - suffix.len()]);
        let suffix = lines::uncommented(suffix, suffix_site)?;
        if !suffix.is_empty() {
            let token = suffix.strip_prefix(", icon: ").ok_or_else(|| {
                suffix_site.error("Follow a collection link only with , icon: <token>")
            })?;
            icon = Some(super::icon(token, suffix_site.after(", icon: "))?);
        }
        let target = target
            .strip_prefix('<')
            .and_then(|target| target.strip_suffix('>'))
            .unwrap_or(target);
        if let Some(local) = target
            .strip_prefix(relative)
            .and_then(|path| path.strip_suffix(".md"))
        {
            lines::id(local, site)?;
            collection_ref = Some((local.into(), None));
            (Some(local.into()), Some(title.to_owned()))
        } else {
            let (owner, id) = collection_reference(target, site)?;
            collection_ref = Some((id.into(), Some(owner.into())));
            (Some(target.into()), Some(title.to_owned()))
        }
    };
    Ok(SidebarEntry {
        source: source.into(),
        title,
        collection,
        collection_ref,
        icon,
        site,
    })
}

/// Parsed agent rules in file order. The host applies precedence and authority.
#[derive(Clone, Debug)]
pub struct AgentRules {
    /// Individual rules with conditions compiled over authorized values.
    pub rules: Vec<AgentRule>,
}

/// One agent rule declaration.
#[derive(Clone, Debug)]
pub struct AgentRule {
    /// Decision word, one of allow, ask or deny.
    pub decision: String,
    /// Operation pattern or fetch target.
    pub target: String,
    /// Optional kind selector.
    pub kind: Option<String>,
    /// Qualified collection or normalized folder selector.
    pub location: Option<String>,
    /// Optional formula the host tests before and after the change.
    pub condition: Option<Expression>,
    /// Whether the rule only applies outside the current chat.
    pub outside_current_chat: bool,
    /// Optional harness ID restriction.
    pub harness: Option<String>,
}

/// Read and compile an agent rules file without applying it.
pub fn parse_agent_rules(source: &str) -> Result<AgentRules, Diagnostic> {
    let document = definition(source)?;
    let header = sections(&document.header, 0)?;
    required(&header, &["rules", "format"])?;
    for entry in &header {
        entry.alone()?;
        match entry.key {
            "rules" if entry.value == "agents" => {}
            "format" => format(entry.value, entry.site)?,
            _ => return Err(entry.line.site.error("Use rules: agents and format: 1")),
        }
    }
    let mut rules = Vec::new();
    for (source, site) in body_lines(document.body, document.body_site)? {
        let (decision, remainder) = source[2..]
            .split_once(' ')
            .ok_or_else(|| site.error("Write allow, ask or deny followed by a target"))?;
        if !matches!(decision, "allow" | "ask" | "deny") {
            return Err(site.error("Use allow, ask or deny"));
        }
        let (remainder, harness) = clause(remainder, " for ");
        if let Some(harness) = harness {
            lines::id(harness, site)?
        }
        let outside = remainder.ends_with(" outside this chat");
        let remainder = remainder
            .strip_suffix(" outside this chat")
            .unwrap_or(remainder);
        let (remainder, condition) = clause(remainder, " where ");
        let condition = condition
            .map(|condition| {
                parse::parse(
                    condition,
                    site.after(&source[..condition.as_ptr() as usize - source.as_ptr() as usize]),
                )
            })
            .transpose()?;
        let (remainder, location) = clause(remainder, " in ");
        let (target, kind) = clause(remainder, " on ");
        if let Some(location) = location {
            if let Some(folder) = location.strip_suffix('/') {
                path(folder, site)?
            } else {
                collection_reference(location, site)?;
            }
        }
        if let Some(host) = target.strip_prefix("fetch ") {
            super::manifest::hostname(host, site)?;
            if kind.is_some() || location.is_some() || condition.is_some() || outside {
                return Err(site.error("A fetch rule can only narrow the host and harness"));
            }
        } else if let Some((plugin, operation)) = target.split_once('.') {
            lines::id(plugin, site)?;
            if operation.is_empty() {
                return Err(site.error("Name an operation after the plugin"));
            }
        } else if !matches!(target, "add" | "change" | "remove") || location.is_none() {
            return Err(
                site.error("Name a qualified operation, host or file operation with a folder")
            );
        }
        rules.push(AgentRule {
            decision: decision.into(),
            target: target.into(),
            kind: kind.map(str::to_owned),
            location: location.map(str::to_owned),
            condition,
            outside_current_chat: outside,
            harness: harness.map(str::to_owned),
        });
    }
    Ok(AgentRules { rules })
}

fn body_lines(body: &str, start: Site) -> Result<Vec<(&str, Site)>, Diagnostic> {
    let mut result = Vec::new();
    for (i, text) in body.lines().enumerate() {
        let site = Site {
            line: start.line + i,
            column: 1,
        };
        if text.trim().is_empty() || text.starts_with("# ") || text == "#" {
            continue;
        }
        if !text.starts_with("- ") || text.contains('\t') {
            return Err(site.error("Start the line with - and use spaces instead of tabs"));
        }
        result.push((text, site));
    }
    Ok(result)
}

fn collection_reference(target: &str, site: Site) -> Result<(&str, &str), Diagnostic> {
    let (plugin, collection) = target.split_once(":collections/").ok_or_else(|| {
        site.error("Name a collection as plugin:collections/id or a folder ending in /")
    })?;
    lines::id(plugin, site)?;
    lines::id(collection, site)?;
    Ok((plugin, collection))
}

fn clause<'a>(source: &'a str, separator: &str) -> (&'a str, Option<&'a str>) {
    let mut quote = false;
    let mut escaped = false;
    for (offset, ch) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            quote = !quote;
        }
        if !quote && source[offset..].starts_with(separator) {
            return (&source[..offset], Some(&source[offset + separator.len()..]));
        }
    }
    (source, None)
}
