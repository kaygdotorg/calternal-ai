//! Checked declarations for the host DAV adapters.

use std::collections::BTreeSet;

use super::{Kind, Section, kind};
use crate::{Diagnostic, Expression, FieldType};

/// The calendar or contact component represented by the kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DavComponent {
    /// Calendar events.
    Event,
    /// Calendar tasks.
    Todo,
    /// Contact cards.
    Card,
}

/// A checked bidirectional mapping for a single component type.
#[derive(Clone, Debug)]
pub struct DavMapping {
    /// The component type used by the host adapter.
    pub component: DavComponent,
    /// Field and property bindings in file order.
    pub properties: Vec<DavProperty>,
}

/// One host DAV property binding.
#[derive(Clone, Debug)]
pub struct DavProperty {
    /// The stored field name, or body for the Markdown body.
    pub field: String,
    /// The protocol property name.
    pub property: String,
    /// When true, the host suppresses this property in both directions.
    pub unless: Option<Expression>,
}

pub(super) fn parse(entry: &Section<'_>, kind: &Kind) -> Result<DavMapping, Diagnostic> {
    let component = match (entry.key, entry.value) {
        ("ical", "VEVENT") => DavComponent::Event,
        ("ical", "VTODO") => DavComponent::Todo,
        ("vcard", "") => DavComponent::Card,
        _ => {
            return Err(entry
                .site
                .error("Use ical: VEVENT, ical: VTODO, or a vcard block"));
        }
    };
    let mut properties = Vec::new();
    let mut seen_fields = BTreeSet::new();
    let mut seen_properties = BTreeSet::new();
    for line in entry.children {
        kind::flat(line, entry)?;
        let (name, text) = line
            .content()?
            .split_once(": ")
            .ok_or_else(|| line.site.error("Map a field to a DAV property"))?;
        let stored = kind
            .fields
            .get(name)
            .filter(|field| field.field_type.is_some());
        if name != "body" && stored.is_none() {
            return Err(line
                .site
                .error("A DAV mapping requires the body or a stored field"));
        }
        let parts = line.parts(text)?;
        let [(property, site), tail @ ..] = parts.as_slice() else {
            return Err(line.site.error("Name a DAV property"));
        };
        let allowed = match component {
            DavComponent::Event | DavComponent::Todo => matches!(
                *property,
                "SUMMARY"
                    | "DESCRIPTION"
                    | "DTSTART"
                    | "DTEND"
                    | "DUE"
                    | "DURATION"
                    | "LOCATION"
                    | "RRULE"
                    | "RDATE"
                    | "EXDATE"
                    | "VALARM"
                    | "STATUS"
                    | "CATEGORIES"
                    | "CLASS"
                    | "TRANSP"
                    | "PRIORITY"
                    | "COMPLETED"
                    | "PERCENT-COMPLETE"
                    | "ATTENDEE"
                    | "ORGANIZER"
            ),
            DavComponent::Card => matches!(
                *property,
                "FN" | "N"
                    | "NICKNAME"
                    | "EMAIL"
                    | "TEL"
                    | "ADR"
                    | "ORG"
                    | "TITLE"
                    | "ROLE"
                    | "NOTE"
                    | "BDAY"
                    | "ANNIVERSARY"
                    | "CATEGORIES"
                    | "KIND"
            ),
        };
        if !allowed || !seen_fields.insert(name) || !seen_properties.insert(*property) {
            return Err(site.error("Map each field and allowed DAV property once"));
        }
        if *property == "EXDATE"
            && stored
                .and_then(|field| field.field_type.as_ref())
                .is_none_or(
                    |ty| !matches!(ty, FieldType::List(inner) if **inner == FieldType::Date),
                )
        {
            return Err(site.error("EXDATE requires a dates field"));
        }
        let unless = match tail {
            [] => None,
            [(condition, site)] if component != DavComponent::Card => {
                let condition = condition
                    .strip_prefix("unless ")
                    .ok_or_else(|| site.error("Use unless followed by a condition"))?;
                let condition = crate::expr::parse::ordinary(condition, site.after("unless "))?;
                if super::formulas::viewer_dependent(
                    &condition.root,
                    &kind.fields,
                    &Default::default(),
                ) {
                    return Err(site.error("A DAV condition cannot use viewer-dependent fields"));
                }
                Some(condition)
            }
            _ => return Err(site.error("Use one calendar mapping condition")),
        };
        properties.push(DavProperty {
            field: name.into(),
            property: (*property).into(),
            unless,
        });
    }
    if properties.is_empty() || kind.hidden {
        return Err(entry
            .site
            .error("A DAV mapping requires a visible kind and at least one property"));
    }
    Ok(DavMapping {
        component,
        properties,
    })
}
