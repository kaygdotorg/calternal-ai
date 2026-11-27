//! Refuse declarations whose runtime data or authority this host cannot supply.
use super::{Field, OperationForm, Plugin};
use calternal_format::{self as format, FillBinding};
use calternal_types::{Error, Result};

pub(super) fn check(plugin: &Plugin) -> Result<()> {
    let manifest = &plugin.manifest;
    for directive in &manifest.directives {
        let reason = match directive.key.as_str() {
            "extends" => "This host cannot apply plugin extensions.",
            "refuses" => "This host cannot enforce plugin refusal rules.",
            "outside" => "This host cannot grant outside access requested by the plugin.",
            "notifications" => "This host cannot deliver plugin notifications.",
            "sign-ins" | "secrets" => "This host cannot supply sign-ins or secrets to the plugin.",
            "listens" => "This host cannot start the plugin's declared listeners.",
            "needs" => "This host cannot enforce the plugin's declared prerequisites.",
            _ => continue,
        };
        return Err(Error::forbidden(reason));
    }
    if !manifest.settings.is_empty() {
        return Err(Error::forbidden(
            "This host cannot supply plugin settings to formulas.",
        ));
    }
    if manifest.fills().any(|fill| {
        matches!(
            fill.binding,
            FillBinding::Receive { .. } | FillBinding::Frame(_)
        )
    }) {
        return Err(Error::forbidden(
            "This host cannot open plugin frames or receive shared inputs.",
        ));
    }
    if plugin
        .sidebars
        .values()
        .flat_map(|sidebar| &sidebar.entries)
        .any(|entry| entry.title.is_none())
    {
        return Err(Error::forbidden(
            "This host cannot populate dynamic or folder sidebar entries.",
        ));
    }

    for kind in plugin.kinds.values() {
        if kind.rights.change.is_some() || !kind.rights.read.is_empty() {
            return Err(Error::forbidden(
                "This host cannot enforce kind-specific read or change rights.",
            ));
        }
        if kind
            .default_where
            .as_ref()
            .is_some_and(format::Expression::requires_query_lookups)
        {
            return Err(unsupported_lookup());
        }
        if kind.line_claim.is_some() {
            return Err(Error::forbidden(
                "This host cannot synchronize items with lines in other notes.",
            ));
        }
        if !kind.dav.is_empty() {
            return Err(Error::forbidden(
                "This host cannot serve calendar or contact mappings.",
            ));
        }
        let fields = kind
            .fields
            .values()
            .chain(kind.records.values())
            .chain(kind.state.values());
        for field in fields {
            check_field(field)?;
        }
        let local = |name: &str| kind.fields.contains_key(name);
        for operation in kind.operations.values() {
            if operation
                .condition
                .as_ref()
                .is_some_and(format::Expression::requires_query_lookups)
            {
                return Err(unsupported_lookup());
            }
            match &operation.form {
                OperationForm::Open(_) => {
                    return Err(Error::forbidden(
                        "This host cannot open or create an item from an operation filter.",
                    ));
                }
                OperationForm::Duplicate => {
                    return Err(Error::forbidden(
                        "This host cannot duplicate an item and its records.",
                    ));
                }
                OperationForm::Timer { .. } => {
                    return Err(Error::forbidden(
                        "This host cannot schedule a named operation.",
                    ));
                }
                OperationForm::Add { phrases: true, .. } => {
                    return Err(Error::forbidden(
                        "This host cannot extract form inputs from a phrase.",
                    ));
                }
                OperationForm::Set(sets) => {
                    if let [set] = sets.as_slice()
                        && set.state
                        && set
                            .value
                            .as_ref()
                            .is_none_or(|value| value.literal().is_some())
                    {
                        // The existing viewer-state surface accepts a single supplied or literal value.
                        continue;
                    }
                    if sets.iter().any(|set| {
                        set.state
                            || set.target == "body"
                            || set
                                .value
                                .as_ref()
                                .is_some_and(|expr| !expr.uses_only_row_and_clock(local))
                    }) || operation
                        .condition
                        .as_ref()
                        .is_some_and(|expr| !expr.uses_only_row_and_clock(local))
                        || kind
                            .fields
                            .values()
                            .filter_map(|field| field.expression.as_ref())
                            .any(|expr| !expr.uses_only_row_and_clock(local))
                    {
                        return Err(Error::forbidden(
                            "An item assignment must use stored fields and the clock. This host cannot combine it with body or viewer state changes.",
                        ));
                    }
                }
                _ if operation.condition.is_some() => {
                    return Err(Error::forbidden(
                        "Only item assignments and single viewer state actions can declare a condition on this host.",
                    ));
                }
                _ => {}
            }
        }
    }

    for collection in plugin.collections.values() {
        if !matches!(collection.sources.as_slice(), [format::Source::Kind(_)])
            || collection.share_source == format::ShareSource::Shared
        {
            return Err(Error::forbidden(
                "This host can serve a collection from one local kind.",
            ));
        }
        if !collection.body.trim().is_empty() {
            return Err(Error::forbidden(
                "This host cannot include hand-added collection entries.",
            ));
        }
        if collection
            .condition
            .iter()
            .chain(&collection.group)
            .any(format::Expression::requires_query_lookups)
        {
            return Err(unsupported_lookup());
        }
        if collection.expand_repeat {
            return Err(Error::forbidden(
                "This host cannot expand repeats into a collection date window.",
            ));
        }
        if collection.drop.is_some() {
            return Err(Error::forbidden(
                "This host cannot run collection drop actions.",
            ));
        }
        for field in collection
            .fields
            .values()
            .chain(collection.named_totals.values())
        {
            check_field(field)?;
        }
        if collection
            .named_totals
            .values()
            .filter_map(|field| field.expression.as_ref())
            .any(|expr| {
                !expr.uses_only_row_and_clock(|name| collection.named_totals.contains_key(name))
            })
        {
            return Err(Error::forbidden(
                "This host cannot evaluate a total that reads another row or a historical period.",
            ));
        }
    }
    Ok(())
}

fn check_field(field: &Field) -> Result<()> {
    if field.period.is_some() {
        return Err(Error::forbidden(
            "This host cannot supply historical rows for a per-period field.",
        ));
    }
    if field.heading
        || field.from_metadata
        || field.sidecar.is_some()
        || !field.previous_names.is_empty()
    {
        return Err(Error::forbidden(
            "This host cannot sync heading, metadata or sidecar fields, or rewrite former field names.",
        ));
    }
    if field
        .expression
        .as_ref()
        .is_some_and(format::Expression::requires_query_lookups)
    {
        return Err(unsupported_lookup());
    }
    Ok(())
}

fn unsupported_lookup() -> Error {
    Error::forbidden(
        "This host cannot evaluate a formula that needs other rows, settings or a selected item.",
    )
}
