use super::Rights;
use std::collections::BTreeMap;

pub(super) use super::formulas::computed_order;

use super::{Section, definition, format, list, matching_id, name, path, required, sections, text};
use crate::{
    Diagnostic, Expression, Field, FieldType, Operation, dates, field,
    lines::{self, Line, Site},
};

/// Whether a kind can be edited through the file surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Editability {
    /// Ordinary add, change and remove are supplied when absent.
    Outside,
    /// Only listed operations can request changes.
    Operations,
}

/// How an item is placed in a kind's folder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ItemForm {
    /// One Markdown file.
    Markdown,
    /// A folder with one main Markdown file.
    Folder(String),
    /// A binary original and optional pairing with sibling originals.
    Binary {
        /// The accepted media types.
        media_types: Vec<String>,
        /// Whether related originals form a single item.
        paired: bool,
    },
}

/// One checked component in a kind page.
#[derive(Clone, Debug)]
pub enum PageEntry {
    /// The Markdown body.
    Body,
    /// The kind's property panel.
    Properties,
    /// The record list, optionally with named fields.
    Records(Option<Vec<String>>),
    /// One field placed on the page.
    Field(String),
}

/// The compiled definition of one kind.
#[derive(Clone, Debug)]
pub struct Kind {
    /// The plugin-local kind ID.
    pub id: String,
    /// The displayed kind name.
    pub name: String,
    /// Relative folder pattern. A hidden kind's path is host-managed.
    pub folder: Option<String>,
    /// Filename pattern interpreted by the host.
    pub file: String,
    /// Include folders marked by the user for this kind.
    pub plus_marked_folders: bool,
    /// The file or folder shape of a stored item.
    pub item: ItemForm,
    /// Whether the host presents the file for outside edits.
    pub editable: Editability,
    /// Narrowing declarations for the host authorization check.
    pub rights: Rights,
    /// Stored and computed item fields.
    pub fields: BTreeMap<String, Field>,
    /// Stored and computed fields of records in this kind.
    pub records: BTreeMap<String, Field>,
    /// Declarations of per-person state values.
    pub state: BTreeMap<String, Field>,
    /// Operations keyed by their route name.
    pub operations: BTreeMap<String, Operation>,
    /// The optional field that gives an item's deterministic uniqueness key.
    pub unique: Option<String>,
    /// Fields treated as one unit when the host merges concurrent edits.
    pub together: Vec<Vec<String>>,
    /// Save-time conditions and their optional caller-facing failure messages.
    pub checks: Vec<(Expression, Option<String>)>,
    /// Filter applied by default to all listings.
    pub default_where: Option<Expression>,
    /// Retention declaration in format duration syntax.
    pub keep: Option<String>,
    /// Whether this is private plugin state with expiry.
    pub hidden: bool,
    /// Starting Markdown body name, or the literal none.
    pub template: Option<String>,
    /// Ordered fields drawn in a row.
    pub row: Vec<String>,
    /// Ordered fields and components drawn on a page.
    pub page: Vec<String>,
    /// Checked page components in the same order as page.
    pub compiled_page: Vec<PageEntry>,
    /// Ordered search fields.
    pub search: Vec<String>,
    /// Fields that stay visible when a page is extended.
    pub protected: Vec<String>,
    /// Note line notation and source kinds claimed by this kind.
    pub line_claim: Option<super::LineClaim>,
    /// Checked calendar or contact mappings for host adapters.
    pub dav: Vec<super::DavMapping>,
    /// The numeric field surfaced as operation progress.
    pub progress: Option<String>,
    /// The Markdown copied into the host-authored folder description.
    pub body: String,
    pub(crate) computed: Vec<String>,
    pub(crate) record_computed: Vec<String>,
    pub(crate) site: Site,
}

pub(super) fn parse(
    source: &str,
    id: &str,
    settings: &BTreeMap<String, Field>,
) -> Result<Kind, Diagnostic> {
    lines::kind_id(id, Site::START)?;
    let document = definition(source)?;
    let entries = sections(&document.header, 0)?;
    required(&entries, &["kind", "name", "format", "fields"])?;
    let hidden = entries.iter().any(|entry| entry.key == "hidden");
    if !hidden {
        required(&entries, &["folder", "editable"])?
    }
    let body = text(document.body, document.body_site, 4096, true)?;
    if !hidden && body.trim().is_empty() {
        return Err(document
            .body_site
            .error("Describe what this kind stores in the body"));
    }
    let mut kind = Kind {
        id: id.into(),
        name: String::new(),
        folder: None,
        file: String::new(),
        plus_marked_folders: false,
        item: ItemForm::Markdown,
        editable: Editability::Operations,
        rights: Rights::default(),
        fields: BTreeMap::new(),
        records: BTreeMap::new(),
        state: BTreeMap::new(),
        operations: BTreeMap::new(),
        unique: None,
        together: Vec::new(),
        checks: Vec::new(),
        default_where: None,
        keep: None,
        hidden,
        template: None,
        row: Vec::new(),
        page: Vec::new(),
        compiled_page: Vec::new(),
        search: Vec::new(),
        protected: Vec::new(),
        progress: None,
        line_claim: None,
        dav: Vec::new(),
        body,
        computed: Vec::new(),
        record_computed: Vec::new(),
        site: entries[0].line.site,
    };
    for entry in &entries {
        let Section {
            key,
            value,
            site,
            children,
            ..
        } = *entry;
        if !matches!(
            key,
            "fields"
                | "records"
                | "state"
                | "together"
                | "checks"
                | "operations"
                | "rights"
                | "vcard"
                | "ical"
        ) {
            entry.alone()?
        }
        match key {
            "kind" => matching_id(entry, id)?,
            "name" => kind.name = name(value, site)?,
            "format" => format(value, site)?,
            "folder" => {
                let parts = list(value, site)?;
                let [folder, rest @ ..] = parts.as_slice() else {
                    return Err(site.error("Declare a folder pattern"));
                };
                if !matches!(rest, [] | [_])
                    || rest
                        .first()
                        .is_some_and(|flag| flag != "plus marked folders")
                {
                    return Err(site.error("Unknown folder flag"));
                }
                path(folder, site)?;
                if folder.to_ascii_lowercase().starts_with(".calternal") {
                    return Err(site.error("Kind folders cannot start with .calternal"));
                }
                kind.plus_marked_folders = !rest.is_empty();
                kind.folder = Some(folder.clone());
            }
            "file" => {
                path(value, site)?;
                if value.contains('/') {
                    return Err(site.error("Use a single filename pattern"));
                }
                kind.file = value.into();
            }
            "item" => kind.item = item_form(value, site)?,
            "editable" if !hidden => {
                kind.editable = match value {
                    "outside" => Editability::Outside,
                    "operations" => Editability::Operations,
                    _ => return Err(site.error("Use editable: outside or editable: operations")),
                }
            }
            "editable" => return Err(site.error("A hidden kind is always operations only")),
            "rights" => super::rights::parse(entry, &mut kind.rights)?,
            "fields" | "records" | "state" => {
                entry.block()?;
                let parsed = fields(children, entry.line.depth + 1, key)?;
                match key {
                    "fields" => kind.fields = parsed,
                    "records" => kind.records = parsed,
                    _ => kind.state = parsed,
                }
            }
            "operations" => {
                entry.block()?;
                kind.operations = crate::operation::parse(children, entry.line.depth + 1)?;
            }
            "unique" => kind.unique = Some(value.into()),
            "together" => {
                entry.block()?;
                for child in children {
                    flat(child, entry)?;
                    let fields = list(child.content()?, child.site)?;
                    if fields.len() < 2 {
                        return Err(child.site.error("Group at least two fields"));
                    }
                    kind.together.push(fields);
                }
            }
            "checks" => {
                entry.block()?;
                for child in children {
                    flat(child, entry)?;
                    let parts = lines::parts(child.content()?, child.site)?;
                    let [(predicate, site), message @ ..] = parts.as_slice() else {
                        return Err(child.site.error("Write a condition to check"));
                    };
                    if message.len() > 1 {
                        return Err(child.site.error("Quote a check message containing a comma"));
                    }
                    let message = message
                        .first()
                        .map(|(value, site)| text(value, *site, 120, false))
                        .transpose()?;
                    let start = predicate.as_ptr() as usize - child.text.as_ptr() as usize;
                    let predicate = crate::expr::parse::parse_at(predicate, |offset| {
                        child.site_at(start + offset)
                    })?;
                    let _ = site;
                    kind.checks.push((predicate, message));
                }
            }
            "default where" => kind.default_where = Some(entry.expression(value)?),
            "keep" | "hidden" => {
                let duration = if key == "hidden" {
                    value.strip_prefix("keep ").ok_or_else(|| {
                        site.error("Declare a hidden kind as hidden: keep <duration>")
                    })?
                } else {
                    value
                };
                if kind.keep.is_some()
                    || dates::duration(duration).is_none()
                    || duration.starts_with('-')
                {
                    return Err(site.error("Declare one nonnegative retention such as 7d or 6mo"));
                }
                kind.keep = Some(duration.into());
            }
            "template" => {
                lines::id(value, site)?;
                kind.template = Some(value.into());
            }
            "row" => kind.row = list(value, site)?,
            "page" => {
                for (part, position) in entry.line.parts(value)? {
                    let part = lines::unquote(part, position)?;
                    let component = match part.as_str() {
                        "body" => PageEntry::Body,
                        "properties" => PageEntry::Properties,
                        "records" => PageEntry::Records(None),
                        value => match value
                            .strip_prefix("records(")
                            .and_then(|x| x.strip_suffix(')'))
                        {
                            Some(fields) => {
                                PageEntry::Records(Some(list(fields, position.after("records("))?))
                            }
                            None => PageEntry::Field(part.clone()),
                        },
                    };
                    kind.page.push(part);
                    kind.compiled_page.push(component);
                }
            }
            "search" => kind.search = list(value, site)?,
            "protected" => kind.protected = list(value, site)?,
            "progress" => kind.progress = Some(value.into()),
            "lines" | "ical" | "vcard" => {}
            _ => return Err(entry.line.site.error(format!("Unknown kind key {key}"))),
        }
    }
    if kind.file.is_empty() {
        kind.file = if kind.fields.contains_key("title") {
            "{title}".into()
        } else {
            kind.id.clone()
        }
    }
    for entry in kind.operations.values() {
        if let Some(field) = entry.inputs.get("item")
            && (!field.required
                || !matches!(&field.field_type, Some(FieldType::Link(link)) if link == &kind.id))
        {
            return Err(field
                .site
                .error("Declare item as a required link to this kind."));
        }
        entry.verify(
            &kind.fields,
            &kind.state,
            &kind.records,
            kind.unique.is_some(),
        )?;
    }
    if kind.editable == Editability::Operations && kind.operations.is_empty() {
        return Err(kind
            .site
            .error("List the operations for an operations-only kind"));
    }
    verify_fields(&mut kind, &entries, settings)?;
    for entry in &entries {
        match entry.key {
            "lines" => kind.line_claim = Some(super::line_claims::parse(entry, &kind)?),
            "ical" | "vcard" => kind.dav.push(super::dav::parse(entry, &kind)?),
            _ => {}
        }
    }
    Ok(kind)
}

fn item_form(value: &str, site: Site) -> Result<ItemForm, Diagnostic> {
    if value == "file" {
        return Ok(ItemForm::Markdown);
    }
    if let Some(main) = value.strip_prefix("folder with ") {
        path(main, site)?;
        if !main.ends_with(".md") || main.contains('/') {
            return Err(site.error("Name one Markdown file inside the item folder"));
        }
        return Ok(ItemForm::Folder(main.into()));
    }
    if let Some(rest) = value.strip_prefix("file(") {
        let (media, flag) = rest
            .split_once(')')
            .ok_or_else(|| site.error("Close the media type list with )"))?;
        let media_types = list(media, site)?;
        if !media_types.is_empty()
            && media_types.iter().all(|media| media.contains('/'))
            && matches!(flag, "" | ", paired")
        {
            return Ok(ItemForm::Binary {
                media_types,
                paired: !flag.is_empty(),
            });
        }
    }
    Err(site.error("Use file, folder with <main file>, or file(<media types>)"))
}

pub(super) fn fields(
    lines: &[Line],
    depth: usize,
    scope: &str,
) -> Result<BTreeMap<String, Field>, Diagnostic> {
    let mut result = BTreeMap::new();
    for entry in sections(lines, depth)? {
        entry.alone()?;
        crate::lines::field_name(entry.key, entry.line.site)?;
        let field = field::declaration(entry.value, entry.site, entry.line)?;
        if let Some(expression) = &field.expression {
            super::formulas::formula_scope(expression, field.period.is_some(), false)?;
        }
        if field.period.is_some() && scope != "fields" {
            return Err(entry
                .site
                .error("Declare per-period formulas on item fields"));
        }
        if let Some(site) = field.badge
            && scope != "fields"
        {
            return Err(site.error("A badge is a row field flag"));
        }
        if (scope != "settings" && field.per_person)
            || (field.field_type == Some(FieldType::Anchor) && scope != "state")
            || (scope == "state" && (field.expression.is_some() || field.required))
            || (scope != "fields"
                && (field.heading || field.from_metadata || field.sidecar.is_some()))
        {
            return Err(entry
                .site
                .error(format!("This field type or flag is not allowed in {scope}")));
        }
        result.insert(entry.key.into(), field);
    }
    Ok(result)
}

pub(super) fn flat(line: &Line, entry: &Section<'_>) -> Result<(), Diagnostic> {
    if line.depth != entry.line.depth + 1 || line.content()?.starts_with("- ") {
        return Err(line.site.error(format!(
            "Write a {} line one indentation step inside its section",
            entry.key
        )));
    }
    Ok(())
}

fn verify_fields(
    kind: &mut Kind,
    entries: &[Section<'_>],
    settings: &BTreeMap<String, Field>,
) -> Result<(), Diagnostic> {
    let site = |key| {
        entries
            .iter()
            .find(|section| section.key == key)
            .map_or(kind.site, |section| section.site)
    };
    if let Some(conflict) = kind.fields.get(&kind.id) {
        return Err(conflict
            .site
            .error("A kind and its own field cannot have the same name"));
    }
    if kind
        .fields
        .values()
        .filter(|field| field.field_type == Some(FieldType::Repeat))
        .count()
        > 1
    {
        return Err(site("fields").error("A kind can declare only one repeat field"));
    }
    for (pattern, key) in [
        (kind.folder.as_deref().unwrap_or(""), "folder"),
        (&kind.file, "file"),
    ] {
        for (index, part) in pattern.split('{').enumerate() {
            if index == 0 {
                if part.contains('}') {
                    return Err(site(key).error("Unmatched closing brace in the pattern"));
                }
                continue;
            }
            let (field, tail) = part
                .split_once('}')
                .ok_or_else(|| site(key).error("Close the field placeholder with }"))?;
            let (field, format) = field
                .split_once(':')
                .map_or((field, None), |(field, format)| (field, Some(format)));
            if tail.contains('}')
                || format.is_some_and(|format| {
                    !matches!(
                        format,
                        "YYYY" | "MM" | "DD" | "YYYY-MM" | "YYYY-MM-DD" | "GGGG-[W]WW"
                    )
                })
                || kind
                    .fields
                    .get(field)
                    .is_none_or(|field| !field.required && field.default.is_none())
            {
                return Err(site(key).error(format!("Pattern field {field} needs a required field or a default and a supported date format")));
            }
        }
    }
    if let Some(unique) = &kind.unique
        && kind
            .fields
            .get(unique)
            .is_none_or(|field| field.field_type.is_none())
    {
        return Err(site("unique").error("Name one stored field as the uniqueness key"));
    }
    let mut protected = std::collections::BTreeSet::new();
    for name in &kind.protected {
        if !kind.fields.contains_key(name) && !matches!(name.as_str(), "id" | "created" | "edited")
        {
            return Err(site("protected")
                .error(format!("Name a declared protected field instead of {name}")));
        }
        if !protected.insert(name) {
            return Err(site("protected").error(format!("List protected field {name} once")));
        }
    }
    if let Some(progress) = &kind.progress
        && kind
            .fields
            .get(progress)
            .is_none_or(|field| !matches!(field.field_type, Some(FieldType::Number(_))))
    {
        return Err(site("progress").error("Name a numeric field for operation progress"));
    }
    if !matches!(kind.item, ItemForm::Binary { .. })
        && let Some(field) = kind
            .fields
            .values()
            .find(|field| field.from_metadata || field.sidecar.is_some())
    {
        return Err(field
            .site
            .error("Only binary file kinds read metadata or sidecars"));
    }
    for fields in &kind.together {
        for field in fields {
            if kind
                .fields
                .get(field)
                .is_none_or(|decl| decl.field_type.is_none())
            {
                return Err(site("together").error(format!("Unknown field {field}")));
            }
        }
    }
    kind.computed = computed_order(&kind.fields)?;
    kind.record_computed = computed_order(&kind.records)?;
    super::formulas::check_badges(&kind.fields, &kind.computed, |scope, name| {
        match scope {
            "state" => kind.state.get(name),
            "settings" => settings.get(name),
            "records" => kind.records.get(name),
            _ => None,
        }
        .and_then(|field| field.field_type.as_ref())
    })?;
    super::formulas::mark_viewer_fields(&mut kind.fields, &kind.computed, settings);
    super::formulas::mark_viewer_fields(&mut kind.records, &kind.record_computed, settings);
    if let Some(field) = kind
        .search
        .iter()
        .find(|name| kind.fields.get(*name).is_some_and(Field::is_per_viewer))
    {
        return Err(site("search").error(format!(
            "The viewer-dependent field {field} cannot appear in the shared search index"
        )));
    }
    Ok(())
}
