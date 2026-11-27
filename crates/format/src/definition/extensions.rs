//! Compile namespaced field, operation and presentation additions.

use std::collections::{BTreeMap, BTreeSet};

use super::{InstallOptions, Kind, Plugin, Section, SidebarEntry, kind, list, secondary, sections};
use crate::{
    Change, Diagnostic, EvalContext, Field, FieldType, Fields, Operation, OperationForm,
    OperationInput, Row, Value,
    expr::{Node, NodeKind, Op},
    lines::{self, Site},
    operation::typed_fields,
    query::computed_values,
};

/// One plugin contribution, in manifest order.
#[derive(Clone, Debug)]
pub enum Extension {
    /// Add fields, operations or presentation entries to selected kinds.
    Kind(Box<KindExtension>),
    /// Append entries to a sidebar or Home.
    Sidebar(SidebarExtension),
}

/// The visible kinds a compiled contribution can extend.
#[derive(Clone, Debug)]
pub enum KindSelector {
    /// One explicitly named, installed kind.
    Named {
        /// Owning plugin ID.
        plugin: String,
        /// Kind ID within the owning plugin.
        kind: String,
    },
    /// Every visible kind with at least one stored field of this exact type.
    StoredType(FieldType),
}

impl KindSelector {
    /// Test current or newly installed kinds against the selector.
    pub fn matches(&self, owner: &str, kind: &Kind) -> bool {
        !kind.hidden
            && match self {
                Self::Named { plugin, kind: id } => plugin == owner && id == &kind.id,
                Self::StoredType(ty) => kind
                    .fields
                    .values()
                    .any(|field| field.field_type.as_ref() == Some(ty)),
            }
    }
}

/// A compiled extension of a kind. The host controls enablement and visibility.
#[derive(Clone, Debug)]
pub struct KindExtension {
    /// The kinds this declaration can extend.
    pub target: KindSelector,
    /// Added fields keyed by names local to the extension.
    pub fields: BTreeMap<String, Field>,
    /// Added operations, invoked as the extension's plugin.
    pub operations: BTreeMap<String, Operation>,
    /// Atomic merge groups containing only this contribution's fields.
    pub together: Vec<Vec<String>>,
    /// Ordered row additions, with local added names taking precedence.
    pub row: Vec<String>,
    /// Ordered page additions.
    pub page: Vec<ExtensionPage>,
    computed: Vec<String>,
    site: Site,
}

/// Presentation entries appended to a kind page.
#[derive(Clone, Debug)]
pub struct ExtensionPage {
    /// Optional heading for the appended section.
    pub heading: Option<String>,
    /// Names interpreted relative to the extension.
    pub fields: Vec<String>,
}

/// A contribution to an installed sidebar or to Home.
#[derive(Clone, Debug)]
pub struct SidebarExtension {
    /// Owning plugin and sidebar ID. A missing target means Home.
    pub sidebar: Option<(String, String)>,
    /// New entries appended in this order.
    pub entries: Vec<SidebarEntry>,
    site: Site,
}

impl KindExtension {
    /// Resolve a local added name to its flat storage key.
    pub fn storage_key(&self, plugin: &str, name: &str) -> Option<String> {
        self.fields
            .contains_key(name)
            .then(|| format!("{plugin}.{name}"))
    }

    /// Compute the extension's columns from an authorized item.
    /// Returns the added computed values under their qualified names.
    pub fn compute_fields(
        &self,
        plugin: &str,
        owner: &str,
        kind: &Kind,
        row: Row<'_>,
        context: &EvalContext<'_>,
    ) -> Result<Fields, Diagnostic> {
        self.verify_kind(&self.target, owner, kind)?;
        let fields = self.scoped_fields(plugin, kind, row.fields)?;
        let fields = computed_values(
            fields,
            row.metadata,
            &self.fields,
            &self.computed,
            context,
            &mut 50_000,
        )?;
        Ok(self
            .computed
            .iter()
            .map(|key| (format!("{plugin}.{key}"), fields[key].clone()))
            .collect())
    }

    /// Evaluate an extension operation on an already-authorized target.
    /// Field updates use qualified keys. Code dispatch and open plans still
    /// need the host's ordinary authorization, lookup and persistence checks.
    pub fn plan_operation(
        &self,
        plugin: &str,
        owner: &str,
        kind: &Kind,
        name: &str,
        input: &OperationInput<'_>,
        context: &EvalContext<'_>,
    ) -> Result<Vec<Change>, Diagnostic> {
        self.verify_kind(&self.target, owner, kind)?;
        let operation = self
            .operations
            .get(name)
            .ok_or_else(|| self.site.error("Name a declared extension operation"))?;
        let row = input
            .item
            .ok_or_else(|| self.site.error("Supply the authorized item to extend"))?;
        let stored = self.scoped_fields(plugin, kind, row.fields)?;
        let fields = computed_values(
            stored,
            row.metadata,
            &self.fields,
            &self.computed,
            context,
            &mut 50_000,
        )?;
        let row = Row {
            fields: &fields,
            metadata: row.metadata,
        };
        crate::operation::plan_extension(
            operation,
            &self.fields,
            plugin,
            row,
            input.fields,
            context,
        )
    }

    fn scoped_fields(
        &self,
        plugin: &str,
        kind: &Kind,
        values: &Fields,
    ) -> Result<Fields, Diagnostic> {
        let mut visible = typed_fields(&kind.fields, values, |_| self.site, true)?;
        for (name, field) in &self.fields {
            let key = format!("{plugin}.{name}");
            let raw = values.get(&key).unwrap_or(&Value::Empty);
            let value = match &field.field_type {
                Some(ty) => crate::field::coerce(ty, raw, field.site)?,
                None => Value::Empty,
            };
            visible.insert(name.clone(), value);
        }
        Ok(visible)
    }

    fn verify_kind(
        &self,
        selector: &KindSelector,
        owner: &str,
        kind: &Kind,
    ) -> Result<(), Diagnostic> {
        if !selector.matches(owner, kind) {
            return Err(self
                .site
                .error("The extension does not match this visible kind"));
        }
        self.verify_presentation(kind)
    }

    fn verify_presentation(&self, kind: &Kind) -> Result<(), Diagnostic> {
        for name in self
            .row
            .iter()
            .chain(self.page.iter().flat_map(|page| &page.fields))
        {
            if !self.fields.contains_key(name) {
                if kind.protected.contains(name) {
                    return Err(self
                        .site
                        .error(format!("An extension cannot cover protected field {name}")));
                }
                if !kind.fields.contains_key(name) {
                    return Err(self
                        .site
                        .error(format!("Unknown extension presentation field {name}")));
                }
            }
        }
        Ok(())
    }
}

pub(super) fn parse(entry: &Section<'_>) -> Result<Vec<Extension>, Diagnostic> {
    entry.block()?;
    let mut extensions = Vec::new();
    for target in sections(entry.children, entry.line.depth + 1)? {
        target.block()?;
        if target.children.is_empty() {
            return Err(target
                .site
                .error("Declare at least one contribution to this target"));
        }
        let name = target
            .key
            .strip_prefix('[')
            .and_then(|key| key.strip_suffix(']'))
            .unwrap_or(target.key);
        if name == "home" || name.contains(":sidebars/") {
            let sidebar = if let Some((plugin, id)) = name.split_once(":sidebars/") {
                lines::id(plugin, target.site)?;
                lines::id(id, target.site)?;
                Some((plugin.into(), id.into()))
            } else {
                None
            };
            let mut entries = Vec::new();
            for line in target.children {
                if line.depth != target.line.depth + 1 {
                    return Err(line.site.error("Indent each sidebar contribution one step"));
                }
                entries.push(secondary::sidebar_entry(
                    &line.text,
                    line.site,
                    "collections/",
                )?);
            }
            extensions.push(Extension::Sidebar(SidebarExtension {
                sidebar,
                entries,
                site: target.site,
            }));
            continue;
        }
        let selector = if let Some(ty) = name.strip_prefix("any kind with ") {
            KindSelector::StoredType(match ty {
                "url" => FieldType::Url,
                "file" => FieldType::File,
                "person" => FieldType::Person,
                "place" => FieldType::Place,
                "time" => FieldType::Time,
                "date" => FieldType::Date,
                _ => {
                    return Err(target.site.error(
                        "Choose url, file, person, place, time or date for a dynamic target",
                    ));
                }
            })
        } else {
            let (plugin, kind) = name
                .split_once('.')
                .ok_or_else(|| target.site.error("Name a qualified kind to extend"))?;
            lines::id(plugin, target.site)?;
            lines::kind_id(kind, target.site)?;
            KindSelector::Named {
                plugin: plugin.into(),
                kind: kind.into(),
            }
        };
        extensions.push(Extension::Kind(Box::new(parse_kind(&target, selector)?)));
    }
    if extensions.is_empty() {
        return Err(entry.site.error("Declare at least one extension target"));
    }
    Ok(extensions)
}

fn parse_kind(target: &Section<'_>, selector: KindSelector) -> Result<KindExtension, Diagnostic> {
    let mut result = KindExtension {
        target: selector,
        fields: BTreeMap::new(),
        operations: BTreeMap::new(),
        together: Vec::new(),
        row: Vec::new(),
        page: Vec::new(),
        computed: Vec::new(),
        site: target.site,
    };
    for entry in sections(target.children, target.line.depth + 1)? {
        let depth = entry.line.depth + 1;
        match entry.key {
            "fields" => {
                entry.block()?;
                result.fields = kind::fields(entry.children, depth, "fields")?;
            }
            "operations" => {
                entry.block()?;
                result.operations = crate::operation::parse(entry.children, depth)?;
            }
            "together" => {
                entry.block()?;
                for line in entry.children {
                    kind::flat(line, &entry)?;
                    let names = list(line.content()?, line.site)?;
                    if names.iter().collect::<BTreeSet<_>>().len() != names.len() || names.len() < 2
                    {
                        return Err(line.site.error("Group at least two distinct added fields"));
                    }
                    result.together.push(names);
                }
            }
            "row" | "page" => {
                entry.alone()?;
                let mut additions = entry.value.strip_prefix("+ ").ok_or_else(|| {
                    entry
                        .site
                        .error("Prefix extension presentation additions with +")
                })?;
                let heading = if entry.key == "page"
                    && let Some(section) = additions.strip_prefix("section ")
                {
                    let (label, tail) = section.split_once("\": ").ok_or_else(|| {
                        entry
                            .site
                            .error("Write section followed by a double-quoted heading and a colon")
                    })?;
                    if !label.starts_with('"') {
                        return Err(entry.site.error("Quote the section heading"));
                    }
                    additions = tail;
                    Some(super::text(
                        &section[..label.len() + 1],
                        entry.site,
                        40,
                        false,
                    )?)
                } else {
                    None
                };
                let fields = list(additions, entry.site)?;
                if fields.is_empty() {
                    return Err(entry.site.error("List at least one presentation field"));
                }
                if entry.key == "row" {
                    result.row = fields;
                } else {
                    result.page.push(ExtensionPage { heading, fields });
                }
            }
            _ => {
                return Err(entry
                    .site
                    .error("Extend a kind with fields, operations, together, row or page"));
            }
        }
    }
    if result.fields.contains_key("body") {
        return Err(target
            .site
            .error("The Markdown body is reserved on an extended kind"));
    }
    for field in result.fields.values_mut() {
        field.viewer_dependent = field.expression.is_some();
        if field.heading || field.from_metadata || field.sidecar.is_some() || field.is_badge() {
            return Err(field
                .site
                .error("An extension field cannot replace an owner projection"));
        }
        if let Some(expression) = &field.expression {
            super::formulas::walk(&expression.root, &mut |node| {
                if matches!(&node.kind, NodeKind::Call(name, _) if name == "from" || name == "all")
                {
                    return Err(node
                        .site
                        .error("Computed extensions cannot use from() or all()"));
                }
                Ok(())
            })?;
        }
    }
    for name in result.together.iter().flatten() {
        if result
            .fields
            .get(name)
            .is_none_or(|field| field.field_type.is_none())
        {
            return Err(target
                .site
                .error("Group only stored fields added by this extension"));
        }
    }
    for op in result.operations.values() {
        lines::operation_name(&op.name, op.site)?;
        if op.placements.iter().any(|place| place != "menu") {
            return Err(op
                .site
                .error("An extension operation may use menu placement only"));
        }
        match &op.form {
            OperationForm::Code | OperationForm::Open(_) => {}
            OperationForm::Set(assignments)
                if assignments.iter().all(|assignment| {
                    !assignment.state && result.fields.contains_key(&assignment.target)
                }) => {}
            _ => {
                return Err(op
                    .site
                    .error("Use code, open where, or set on this extension's added fields"));
            }
        }
        op.verify(&result.fields, &BTreeMap::new(), &BTreeMap::new(), false)?;
    }
    result.computed = kind::computed_order(&result.fields)?;
    Ok(result)
}

fn check_marks(extension: &KindExtension, target: Option<&Kind>) -> Result<(), Diagnostic> {
    if !extension.fields.values().any(|field| field.mark) {
        return Ok(());
    }
    let mut booleans = BTreeSet::new();
    for name in &extension.computed {
        let field = &extension.fields[name];
        let local = |name: &str| {
            booleans.contains(name)
                || extension
                    .fields
                    .get(name)
                    .or_else(|| target.and_then(|kind| kind.fields.get(name)))
                    .is_some_and(|field| field.mark || field.field_type == Some(FieldType::Bool))
        };
        let formula = field.expression.as_ref().expect("computed extension field");
        if boolean(&formula.root, &local) {
            booleans.insert(name.as_str());
        } else if field.mark {
            return Err(field.site.error("A mark requires a yes/no formula"));
        }
    }
    Ok(())
}

fn boolean(node: &Node, local: &impl Fn(&str) -> bool) -> bool {
    let optional =
        |node: &Node| matches!(&node.kind, NodeKind::Value(Value::Empty)) || boolean(node, local);
    match &node.kind {
        NodeKind::Value(Value::Bool(_)) => true,
        NodeKind::Name(name) => local(name),
        NodeKind::Member(base, name) => {
            matches!(&base.kind, NodeKind::Name(scope) if scope == "this") && local(name)
        }
        NodeKind::Unary(Op::Not, _) => true,
        NodeKind::Binary(
            Op::And | Op::Or | Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::In,
            _,
            _,
        ) => true,
        NodeKind::Call(name, args) => match name.as_str() {
            "contains" | "links to" => true,
            "if" => optional(&args[1]) && optional(&args[2]),
            "first" => args.iter().all(optional),
            _ => false,
        },
        _ => false,
    }
}

pub(super) fn verify(
    plugin: &Plugin,
    options: &InstallOptions<'_>,
    paths: &BTreeSet<&str>,
) -> Result<(), Diagnostic> {
    let installed = |owner| {
        options
            .installed
            .iter()
            .find(|other| other.manifest.id == owner && owner != plugin.manifest.id)
    };
    for extension in &plugin.manifest.extensions {
        match extension {
            Extension::Kind(extension) => {
                if extension.operations.values().any(Operation::is_coded)
                    && !paths.contains("operations.ts")
                {
                    return Err(extension
                        .site
                        .error("Supply operations.ts for the extension's coded operations"));
                }
                if let KindSelector::Named {
                    plugin: owner,
                    kind,
                } = &extension.target
                {
                    let target = installed(owner.as_str())
                        .and_then(|target| target.kinds.get(kind))
                        .ok_or_else(|| {
                            extension
                                .site
                                .error("Extend a visible kind in another installed plugin")
                        })?;
                    extension.verify_kind(&extension.target, owner, target)?;
                    check_marks(extension, Some(target))?;
                } else {
                    if extension
                        .row
                        .iter()
                        .chain(extension.page.iter().flat_map(|page| &page.fields))
                        .any(|name| !extension.fields.contains_key(name))
                    {
                        return Err(extension
                            .site
                            .error("Present only the added fields on a dynamic extension target"));
                    }
                    check_marks(extension, None)?;
                }
            }
            Extension::Sidebar(extension) => {
                if let Some((owner, id)) = &extension.sidebar
                    && installed(owner.as_str())
                        .is_none_or(|target| !target.sidebars.contains_key(id))
                {
                    return Err(extension
                        .site
                        .error("Extend a sidebar in another installed plugin"));
                }
                for entry in &extension.entries {
                    if let Some((id, None)) = &entry.collection_ref
                        && !plugin.collections.contains_key(id)
                    {
                        return Err(entry
                            .site
                            .error("Declare the linked collection in the extending plugin"));
                    }
                }
            }
        }
    }
    for target in options
        .installed
        .iter()
        .filter(|target| target.manifest.id != plugin.manifest.id)
    {
        for kind in target.kinds.values() {
            verify_matching_names(plugin, &target.manifest.id, kind)?;
        }
    }
    for kind in plugin.kinds.values() {
        let owner = &plugin.manifest.id;
        verify_matching_names(plugin, owner, kind)?;
        for contributor in options
            .installed
            .iter()
            .filter(|other| other.manifest.id != *owner)
        {
            verify_matching_names(contributor, owner, kind)?;
        }
    }
    Ok(())
}

fn verify_matching_names(contributor: &Plugin, owner: &str, kind: &Kind) -> Result<(), Diagnostic> {
    let mut fields = BTreeSet::new();
    let mut operations = BTreeSet::new();
    for extension in &contributor.manifest.extensions {
        let Extension::Kind(extension) = extension else {
            continue;
        };
        if !extension.target.matches(owner, kind) {
            continue;
        }
        let duplicate = extension
            .fields
            .keys()
            .find(|name| !fields.insert(*name))
            .map(|name| ("field", name))
            .or_else(|| {
                extension
                    .operations
                    .keys()
                    .find(|name| !operations.insert(*name))
                    .map(|name| ("operation", name))
            });
        if let Some((category, name)) = duplicate {
            let plugin = &contributor.manifest.id;
            return Err(extension.site.error(format!(
                "Extensions add the same {category} {plugin}.{name} to {owner}.{}",
                kind.id
            )));
        }
    }
    Ok(())
}
