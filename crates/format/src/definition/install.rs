use std::collections::BTreeSet;

use super::{FillBinding, InstallOptions, Kind, Plugin, Source, located};
use crate::{Diagnostic, FieldType};

pub(super) fn verify(
    plugin: &Plugin,
    paths: &BTreeSet<&str>,
    options: &InstallOptions<'_>,
) -> Result<(), Diagnostic> {
    let mut claims = BTreeSet::new();
    for claimant in options
        .installed
        .iter()
        .filter(|other| other.manifest.id != plugin.manifest.id)
        .chain(std::iter::once(plugin))
    {
        for kind in claimant.kinds.values() {
            if let Some(claim) = &kind.line_claim {
                for target in &claim.targets {
                    let (owner, target) = target
                        .split_once('.')
                        .unwrap_or((&claimant.manifest.id, target));
                    if !claims.insert((claim.form, owner, target)) {
                        return Err(located(
                            kind.site.error(format!(
                                "More than one kind claims the same line form in {owner}.{target}"
                            )),
                            &format!("kinds/{}.md", kind.id),
                        ));
                    }
                    let target_kind = std::iter::once(plugin)
                        .chain(options.installed.iter())
                        .find(|candidate| candidate.manifest.id == owner)
                        .and_then(|candidate| candidate.kinds.get(target));
                    if target_kind.is_some_and(|kind| kind.hidden) {
                        return Err(located(
                            kind.site.error("A line source must be visible"),
                            "plugin.md",
                        ));
                    }
                }
            }
        }
    }
    let mut folders: Vec<&str> = options
        .installed
        .iter()
        .filter(|other| other.manifest.id != plugin.manifest.id)
        .flat_map(|plugin| {
            plugin
                .kinds
                .values()
                .filter_map(|kind| kind.folder.as_deref())
        })
        .collect();
    for kind in plugin.kinds.values() {
        let issue =
            |message: String| located(kind.site.error(message), &format!("kinds/{}.md", kind.id));
        if kind.hidden && plugin.description_only {
            return Err(issue(
                "A description-only plugin cannot declare a hidden kind".into(),
            ));
        }
        if kind.operations.values().any(crate::Operation::is_coded)
            && !paths.contains("operations.ts")
        {
            return Err(issue(
                "Supply operations.ts for the declared coded operations".into(),
            ));
        }
        if let Some(folder) = kind.folder.as_deref() {
            if folders.iter().any(|other| folder_overlap(other, folder)) {
                return Err(issue(format!(
                    "The folder {folder} overlaps another kind's folder"
                )));
            }
            folders.push(folder);
        }
        if let Some(template) = &kind.template
            && template != "none"
            && !plugin.templates.contains_key(template)
        {
            return Err(issue(format!(
                "Include the template templates/{template}.md"
            )));
        }
    }
    for collection in plugin.collections.values() {
        for field in collection.named_totals.values() {
            let expression = field
                .expression
                .as_ref()
                .expect("named totals are computed");
            super::formulas::walk(&expression.root, &mut |node| {
                use crate::expr::NodeKind;
                let NodeKind::Call(name, args) = &node.kind else {
                    return Ok(());
                };
                if name != "all" {
                    return Ok(());
                }
                let key =
                    crate::expr::eval::reference_key(&args[0]).expect("checked kind selector");
                let (owner, kind) = key.split_once('.').unwrap_or((&plugin.manifest.id, &key));
                let err = |message| {
                    located(
                        node.site.error(message),
                        &format!("collections/{}.md", collection.id),
                    )
                };
                if owner != plugin.manifest.id && !plugin.manifest.reads.contains(&key) {
                    return Err(err("Declare reads for the foreign kind in all()"));
                }
                let target = std::iter::once(plugin)
                    .chain(options.installed.iter())
                    .find(|candidate| candidate.manifest.id == owner)
                    .and_then(|candidate| candidate.kinds.get(kind))
                    .filter(|kind| !kind.hidden)
                    .ok_or_else(|| err("all() requires a visible kind"))?;
                if let Some(condition) = args.get(1) {
                    super::formulas::walk(condition, &mut |node| {
                        if let NodeKind::Name(name) = &node.kind
                            && !matches!(
                                name.as_str(),
                                "this"
                                    | "settings"
                                    | "state"
                                    | "period"
                                    | "id"
                                    | "kind"
                                    | "created"
                                    | "edited"
                            )
                            && !target.fields.contains_key(name)
                        {
                            return Err(err("Name a declared field of the whole-kind candidate"));
                        }
                        Ok(())
                    })?;
                }
                Ok(())
            })?;
        }
        for source in &collection.sources {
            if let Source::Kind(kind) | Source::Records(kind) = source {
                if let Some((plugin_id, kind_id)) = kind.split_once('.') {
                    let source_plugin = if plugin_id == plugin.manifest.id {
                        Some(plugin)
                    } else {
                        options
                            .installed
                            .iter()
                            .find(|other| other.manifest.id == plugin_id)
                    };
                    if let Some(source_plugin) = source_plugin
                        && source_plugin
                            .kinds
                            .get(kind_id)
                            .is_none_or(|kind| kind.hidden)
                    {
                        return Err(located(
                            collection
                                .site
                                .error(format!("The source kind {kind} is unavailable")),
                            &format!("collections/{}.md", collection.id),
                        ));
                    }
                } else if !plugin.kinds.contains_key(kind) {
                    return Err(located(
                        collection
                            .site
                            .error(format!("The source kind {kind} is missing")),
                        &format!("collections/{}.md", collection.id),
                    ));
                }
            }
        }
    }
    for sidebar in plugin.sidebars.values() {
        for entry in &sidebar.entries {
            if let Some((collection, None)) = &entry.collection_ref
                && !plugin.collections.contains_key(collection)
            {
                return Err(located(
                    entry
                        .site
                        .error(format!("The collection {collection} is missing")),
                    &format!("sidebars/{}.md", sidebar.id),
                ));
            }
        }
    }
    for program in plugin.manifest.programs.values() {
        if let Some(binding) = &program.path
            && !paths.contains(binding.as_str())
        {
            return Err(located(
                program
                    .site
                    .error(format!("Include the program file {binding}")),
                "plugin.md",
            ));
        }
    }
    verify_bindings(plugin, paths, options).map_err(|error| located(error, "plugin.md"))?;
    Ok(())
}

fn operation<'a>(plugin: &'a Plugin, target: &str) -> Option<(&'a Kind, &'a crate::Operation)> {
    if let Some((kind, operation)) = target.split_once('.') {
        let kind = plugin.kinds.get(kind)?;
        return kind.operations.get(operation).map(|op| (kind, op));
    }
    let mut matches = plugin
        .kinds
        .values()
        .filter_map(|kind| kind.operations.get(target).map(|op| (kind, op)));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn verify_bindings(
    plugin: &Plugin,
    paths: &BTreeSet<&str>,
    options: &InstallOptions<'_>,
) -> Result<(), Diagnostic> {
    for slot in plugin.manifest.slots.values() {
        let Some(target) = &slot.input_operation else {
            continue;
        };
        let (kind, operation) = operation(plugin, target)
            .filter(|(_, operation)| !operation.inputs.is_empty())
            .ok_or_else(|| {
                slot.input_site.error(format!(
                    "Declare typed inputs on operation {target} before using it for a slot"
                ))
            })?;
        if let Some(bindings) = &slot.composer {
            let text = operation
                .inputs
                .get(&bindings.text)
                .and_then(|field| field.field_type.as_ref());
            if !matches!(text, Some(FieldType::Text | FieldType::LongText)) {
                return Err(bindings
                    .text_site
                    .error("Bind composer text to a text or long text operation input"));
            }
            if let Some(draft) = &bindings.draft {
                let state = kind
                    .state
                    .get(draft)
                    .and_then(|field| field.field_type.as_ref());
                if !(matches!(state, Some(FieldType::LongText))
                    || matches!(
                        (text, state),
                        (Some(FieldType::Text), Some(FieldType::Text))
                    ))
                {
                    return Err(bindings.draft_site.error("Bind the draft to state on the operation's kind that can hold the composer text"));
                }
            }
            if let Some(quote) = &bindings.quote
                && !matches!(operation.inputs.get(quote).and_then(|field| field.field_type.as_ref()),
                    Some(FieldType::Link(target)) if target.ends_with(" records"))
            {
                return Err(bindings
                    .quote_site
                    .error("Bind the quote to a record-link operation input"));
            }
        }
    }
    for fill in plugin.manifest.fills() {
        match &fill.binding {
            FillBinding::Frame(frame) if !paths.contains(frame.as_str()) => {
                return Err(fill.site.error(format!("Include the frame file {frame}")));
            }
            FillBinding::Collection(collection) if !plugin.collections.contains_key(collection) => {
                return Err(fill.site.error(format!(
                    "Declare collection {collection} before filling a place with it"
                )));
            }
            FillBinding::Operation(target) => {
                verify_operation_fill(plugin, options, fill, target)?;
            }
            _ => {}
        }
    }
    for refusal in &plugin.manifest.refusals {
        let (owner, target) = refusal.target.split_once('.').unwrap();
        let Some(target_plugin) = options
            .installed
            .iter()
            .find(|target| target.manifest.id == owner && owner != plugin.manifest.id)
        else {
            return Err(refusal
                .site
                .error("Refuse only an installed operation belonging to another plugin"));
        };
        let (kind, operation) = operation(target_plugin, target).ok_or_else(|| {
            refusal
                .site
                .error("The refusal target must name one installed operation")
        })?;
        if kind.hidden {
            return Err(refusal
                .site
                .error("A refusal cannot inspect an operation on another plugin's hidden kind"));
        }
        if operation.is_coded() && operation.inputs.is_empty() {
            return Err(refusal
                .site
                .error("The refusal target has no typed operation inputs"));
        }
        crate::definition::formulas::refusal_condition(&refusal.condition.root, kind, operation)?;
    }
    Ok(())
}

fn verify_operation_fill(
    plugin: &Plugin,
    options: &InstallOptions<'_>,
    fill: &super::Fill,
    target: &str,
) -> Result<(), Diagnostic> {
    let (_, filler) = operation(plugin, target).ok_or_else(|| {
        fill.site.error(format!(
            "Declare operation {target} before using it as a filler"
        ))
    })?;
    let Some((owner, slot)) = &fill.slot else {
        return Ok(());
    };
    let owner = if *owner == plugin.manifest.id {
        Some(plugin)
    } else {
        options
            .installed
            .iter()
            .find(|other| other.manifest.id == *owner)
    };
    let template = owner.and_then(|owner| {
        let target = owner.manifest.slots.get(slot)?.input_operation.as_ref()?;
        operation(owner, target).map(|(_, operation)| operation)
    });
    if let Some(template) = template {
        let same = filler.inputs.len() == template.inputs.len()
            && template.inputs.iter().all(|(key, input)| {
                filler.inputs.get(key).is_some_and(|provided| {
                    input.field_type == provided.field_type && input.required == provided.required
                })
            });
        if !same {
            return Err(fill
                .site
                .error("The filler operation inputs must match the slot's declared inputs"));
        }
    }
    Ok(())
}

fn folder_overlap(a: &str, b: &str) -> bool {
    let mut a = a.split('/');
    let mut b = b.split('/');
    loop {
        let (Some(a), Some(b)) = (a.next(), b.next()) else {
            return true;
        };
        if !a.eq_ignore_ascii_case(b) && !a.starts_with('{') && !b.starts_with('{') {
            return false;
        }
    }
}
