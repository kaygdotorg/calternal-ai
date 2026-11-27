use std::collections::BTreeMap;

use super::{Directive, definition, format, kind, list, matching_id, name, required, sections};
use crate::{
    Diagnostic, Expression, Field,
    lines::{self, Site},
};

/// A source declaration interpreted by the host before formula evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Source {
    /// Items of the given local or qualified kind.
    Kind(String),
    /// Authorized records from items of the given kind.
    Records(String),
    /// Authorized files and folders.
    Files,
    /// Plain checkbox rows from authorized notes.
    CheckboxLines,
}

/// Controls which space contributes the collection's authorized source rows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ShareSource {
    /// Include eligible local and shared items.
    #[default]
    LocalAndShared,
    /// Include eligible shared items only.
    Shared,
    /// Include local items only.
    Local,
}

/// A sort column, with manual order represented by the caller's row order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sort {
    /// Field name, or manual to keep host-supplied relative order.
    pub field: String,
    /// Reverse the field's ordinary ascending order.
    pub descending: bool,
}

/// An aggregate over the collection's visible items.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Total {
    /// Count, sum, min, max or average.
    pub function: String,
    /// Field to aggregate. Count of all items has no field.
    pub field: Option<String>,
}

/// One collection view, with column expressions retained as whole source strings.
#[derive(Clone, Debug)]
pub struct CollectionView {
    /// The declared view name, including the chart style where present.
    pub kind: String,
    /// Ordered view arguments, split only at top-level commas.
    pub columns: Vec<String>,
}

/// A compiled collection declaration.
#[derive(Clone, Debug)]
pub struct Collection {
    /// The collection ID.
    pub id: String,
    /// The display title.
    pub title: String,
    /// Optional catalogue icon token for this collection.
    pub icon: Option<String>,
    /// Declared sources to authorize and load.
    pub sources: Vec<Source>,
    /// Whether to skip the source kind's default filter.
    pub ignoring_default: bool,
    /// Which eligible shared rows the host includes.
    pub share_source: ShareSource,
    /// The filter over each item.
    pub condition: Option<Expression>,
    /// Ordered sort precedence.
    pub sort: Vec<Sort>,
    /// Formula whose values form groups.
    pub group: Option<Expression>,
    /// Aggregates over the overall list and each group.
    pub totals: Vec<Total>,
    /// Collection-local computed columns.
    pub fields: BTreeMap<String, Field>,
    /// Expand occurrences in a date window.
    pub expand_repeat: bool,
    /// Checked drop action, where the collection offers one.
    pub drop: Option<Directive>,
    /// Ordered view declarations, default first.
    pub views: Vec<Directive>,
    /// Checked view arguments for presentation, in the same order as views.
    pub compiled_views: Vec<CollectionView>,
    /// Hand-added items kept as Markdown lines.
    pub body: String,
    pub(crate) computed: Vec<String>,
    pub(crate) site: Site,
}

pub(super) fn parse(source: &str, id: &str) -> Result<Collection, Diagnostic> {
    let document = definition(source)?;
    let entries = sections(&document.header, 0)?;
    required(
        &entries,
        &["collection", "title", "format", "from", "views"],
    )?;
    let mut result = Collection {
        id: id.into(),
        title: String::new(),
        icon: None,
        sources: Vec::new(),
        ignoring_default: false,
        share_source: ShareSource::default(),
        condition: None,
        sort: Vec::new(),
        group: None,
        totals: Vec::new(),
        fields: BTreeMap::new(),
        expand_repeat: false,
        drop: None,
        views: Vec::new(),
        compiled_views: Vec::new(),
        body: document.body.into(),
        computed: Vec::new(),
        site: entries[0].site,
    };
    for entry in &entries {
        let key = entry.key;
        let value = entry.value;
        let site = entry.site;
        if !matches!(key, "fields" | "views" | "totals") {
            entry.alone()?
        }
        match key {
            "collection" => matching_id(entry, id)?,
            "title" => result.title = name(value, site)?,
            "format" => format(value, site)?,
            "icon" => result.icon = Some(super::icon(value, site)?),
            "from" => sources(value, site, &mut result)?,
            "where" => result.condition = Some(entry.expression(value)?),
            "group" => result.group = Some(entry.expression(value)?),
            "sort" => result.sort = sort(value, site)?,
            "expand" if value == "repeat" => result.expand_repeat = true,
            "expand" => return Err(site.error("Use expand: repeat")),
            "fields" => {
                entry.block()?;
                result.fields = kind::fields(entry.children, entry.line.depth + 1, key)?;
                if let Some(site) = result.fields.values().find_map(|field| field.badge) {
                    return Err(site.error("Declare the badge on a kind's row field"));
                }
                result.computed = kind::computed_order(&result.fields)?;
                for field in result.fields.values_mut() {
                    field.viewer_dependent = field.expression.is_some();
                }
            }
            "drop" => {
                crate::operation::assignment(value, site)?;
                result.drop = Some(entry.directive()?);
            }
            "views" => {
                entry.block()?;
                for view in sections(entry.children, entry.line.depth + 1)? {
                    view.alone()?;
                    if !matches!(
                        view.key,
                        "list"
                            | "table"
                            | "board"
                            | "gallery"
                            | "calendar"
                            | "timeline"
                            | "chart"
                            | "chart line"
                            | "chart bar"
                            | "map"
                            | "folders"
                            | "compact"
                            | "strip"
                    ) {
                        return Err(view
                            .line
                            .site
                            .error(format!("Unknown collection view {}", view.key)));
                    }
                    if view.value.is_empty() {
                        return Err(view.site.error("Give the view its columns or fields"));
                    }
                    let columns = view.line.parts(view.value)?;
                    if matches!(
                        view.key,
                        "list"
                            | "table"
                            | "board"
                            | "compact"
                            | "strip"
                            | "gallery"
                            | "map"
                            | "folders"
                    ) {
                        for &(value, _) in &columns {
                            view.expression(value)?;
                        }
                    }
                    result.compiled_views.push(CollectionView {
                        kind: view.key.into(),
                        columns: columns
                            .into_iter()
                            .map(|(value, _)| value.to_owned())
                            .collect(),
                    });
                    result.views.push(view.directive()?);
                }
                if result.views.is_empty() {
                    return Err(site.error("Declare at least one collection view"));
                }
            }
            "totals" => {
                entry.block()?;
                for line in entry.children {
                    kind::flat(line, entry)?;
                    let value = line.content()?;
                    let (total, field) = if value == "count" {
                        (value, None)
                    } else if let Some((function, field)) =
                        value.strip_suffix(')').and_then(|v| v.split_once('('))
                    {
                        (function, Some(field.to_owned()))
                    } else {
                        return Err(line
                            .site
                            .error("Named totals and whole-kind formulas are deferred"));
                    };
                    if !matches!(total, "count" | "sum" | "min" | "max" | "average")
                        || field.as_ref().is_some_and(String::is_empty)
                    {
                        return Err(line
                            .site
                            .error("Write count, sum, min, max or average as a collection total"));
                    }
                    result.totals.push(Total {
                        function: total.into(),
                        field,
                    });
                }
            }
            _ => {
                return Err(entry
                    .line
                    .site
                    .error(format!("Unknown collection key {key}")));
            }
        }
    }
    for (index, line) in document.body.lines().enumerate() {
        if !line.trim().is_empty() && !(line.starts_with("- [") && line.contains("](")) {
            return Err(Site {
                line: document.body_site.line + index,
                column: 1,
            }
            .error("Write each hand-added item as a Markdown list link"));
        }
    }
    if result.sources != [Source::Files]
        && let Some(view) = result.views.iter().find(|view| view.key == "folders")
    {
        return Err(view.site.error("The folders view requires from: files"));
    }
    Ok(result)
}

fn sources(value: &str, site: Site, collection: &mut Collection) -> Result<(), Diagnostic> {
    for (part, site) in lines::parts(value, site)? {
        match part {
            "ignoring default" if !collection.ignoring_default => {
                collection.ignoring_default = true
            }
            "shared with me" | "ignoring shared"
                if collection.share_source == ShareSource::LocalAndShared =>
            {
                collection.share_source = if part == "shared with me" {
                    ShareSource::Shared
                } else {
                    ShareSource::Local
                };
            }
            "files" => collection.sources.push(Source::Files),
            "lines(checkbox)" => collection.sources.push(Source::CheckboxLines),
            _ => {
                let (name, is_record) = match part.strip_suffix(".records") {
                    Some(name) => (name, true),
                    None => (part, false),
                };
                let name = name
                    .strip_prefix('[')
                    .and_then(|name| name.strip_suffix(']'))
                    .unwrap_or(name);
                let mut ids = name.split('.');
                lines::id(ids.next().unwrap(), site)?;
                if let Some(kind) = ids.next() {
                    lines::kind_id(kind, site)?
                }
                if ids.next().is_some() {
                    return Err(site.error("Name a local or qualified kind as the source"));
                }
                collection.sources.push(if is_record {
                    Source::Records(name.into())
                } else {
                    Source::Kind(name.into())
                });
            }
        }
    }
    if collection.sources.is_empty() {
        return Err(site.error("Name at least one collection source"));
    }
    if collection.share_source != ShareSource::LocalAndShared
        && collection
            .sources
            .iter()
            .any(|source| !matches!(source, Source::Kind(_)))
    {
        return Err(site.error("Only kind item sources can include shared rows"));
    }
    Ok(())
}

pub(super) fn sort(value: &str, site: Site) -> Result<Vec<Sort>, Diagnostic> {
    let mut result = Vec::new();
    for field in list(value, site)? {
        let (field, descending) = field.strip_suffix(" desc").map_or_else(
            || (field.strip_suffix(" asc").unwrap_or(&field), false),
            |field| (field, true),
        );
        if field == "manual" && (!result.is_empty() || descending) {
            return Err(site.error("Use manual only as the first sort term"));
        }
        result.push(Sort {
            field: field.into(),
            descending,
        });
    }
    Ok(result)
}
