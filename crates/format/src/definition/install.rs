use std::collections::BTreeSet;

use super::{InstallOptions, Kind, Plugin, Source, located};
use crate::{Diagnostic, lines};

pub(super) fn verify(
    plugin: &Plugin,
    paths: &BTreeSet<&str>,
    options: &InstallOptions<'_>,
) -> Result<(), Diagnostic> {
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
            if let Some(collection) = &entry.collection
                && !collection.contains(':')
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
        if let Some(target) = &slot.input_operation
            && operation(plugin, target).is_none_or(|(_, operation)| operation.inputs.is_empty())
        {
            return Err(slot.site.error(format!(
                "Declare typed inputs on operation {target} before using it for a slot"
            )));
        }
    }
    for section in &plugin.manifest.directives {
        if section.key != "fills" {
            continue;
        }
        for fill in &section.children {
            let first = lines::parts(&fill.value, fill.site)?
                .first()
                .map_or("", |(part, _)| *part);
            if let Some(frame) = first.strip_prefix("frame ") {
                if !paths.contains(frame) {
                    return Err(fill.site.error(format!("Include the frame file {frame}")));
                }
            } else if let Some(collection) = first.strip_prefix("collection ") {
                if !plugin.collections.contains_key(collection) {
                    return Err(fill.site.error(format!(
                        "Declare collection {collection} before filling a place with it"
                    )));
                }
            } else if let Some(target) = first.strip_prefix("operation ") {
                let Some((_, filler)) = operation(plugin, target) else {
                    return Err(fill.site.error(format!(
                        "Declare operation {target} before using it as a filler"
                    )));
                };
                if let Some((owner, slot)) = fill.key.split_once(':') {
                    let slot_owner = if owner == plugin.manifest.id {
                        Some(plugin)
                    } else {
                        options
                            .installed
                            .iter()
                            .find(|other| other.manifest.id == owner)
                    };
                    if let Some((slot_owner, slot)) = slot_owner.and_then(|plugin| {
                        plugin.manifest.slots.get(slot).map(|slot| (plugin, slot))
                    }) && let Some((_, template)) = slot
                        .input_operation
                        .as_ref()
                        .and_then(|target| operation(slot_owner, target))
                    {
                        let same = filler.inputs.len() == template.inputs.len()
                            && template.inputs.iter().all(|(key, input)| {
                                filler.inputs.get(key).is_some_and(|provided| {
                                    input.field_type == provided.field_type
                                        && input.required == provided.required
                                })
                            });
                        if !same {
                            return Err(fill.site.error(
                                "The filler operation inputs must match the slot's declared inputs",
                            ));
                        }
                    }
                }
            }
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
