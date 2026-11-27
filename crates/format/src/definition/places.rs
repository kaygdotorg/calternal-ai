use super::{Section, path, text};
use crate::{
    Diagnostic,
    lines::{self, Site},
};

/// A place declared by a plugin, with its typed input schema reference.
#[derive(Clone, Debug)]
pub struct Slot {
    /// Whether the person or space can choose several fillers.
    pub multiple: bool,
    /// Human-facing explanation of the data, or the leading typed gives clause without "gives".
    pub gives: String,
    /// Whether the space chooses the filler.
    pub picked_by_space: bool,
    /// A suggested filler plugin.
    pub recommended: Option<String>,
    /// An operation in this plugin declaring the input schema for the slot.
    pub input_operation: Option<String>,
    pub(crate) input_site: Site,
    /// Display label when declared.
    pub label: Option<String>,
    /// Checked bindings when this slot gives composer inputs.
    pub composer: Option<ComposerBindings>,
}

/// Names of the text, draft state, and quote bindings checked against the owning kind.
#[derive(Clone, Debug)]
pub struct ComposerBindings {
    /// Text input in the slot operation's schema.
    pub text: String,
    /// Per-person state key on the operation's kind, without the state prefix.
    pub draft: Option<String>,
    /// Record-link input in the same operation's schema.
    pub quote: Option<String>,
    pub(crate) text_site: Site,
    pub(crate) draft_site: Site,
    pub(crate) quote_site: Site,
}

/// A declared filler, including its target and optional display label.
#[derive(Clone, Debug)]
pub struct Fill {
    /// Place as written in the manifest.
    pub place: String,
    /// Plugin and local slot IDs, when this fill targets a plugin-defined slot.
    pub slot: Option<(String, String)>,
    /// Typed binding in the supplying plugin.
    pub binding: FillBinding,
    /// Display label. When absent, clients use the supplying plugin name.
    pub label: Option<String>,
    pub(crate) site: Site,
}

/// The target a filler supplies at its declared place.
#[derive(Clone, Debug)]
pub enum FillBinding {
    /// A plugin-relative file inside frames/.
    Frame(String),
    /// A local operation, optionally qualified with its kind.
    Operation(String),
    /// A local collection ID.
    Collection(String),
    /// A receive operation with its declared input media kinds.
    Receive {
        /// Accepted media names in declaration order.
        media: Vec<String>,
        /// Local operation to receive the media.
        operation: String,
    },
}

pub(super) fn slot(place: &Section<'_>) -> Result<Slot, Diagnostic> {
    lines::id(place.key, place.line.site)?;
    let parts = place.line.parts(place.value)?;
    let [
        (cardinality @ ("single" | "many"), _),
        (gives, gives_site),
        rest @ ..,
    ] = parts.as_slice()
    else {
        return Err(place
            .site
            .error("Declare single or many before what the slot gives"));
    };
    let gives_site = gives_site.after("gives ");
    let gives = gives
        .strip_prefix("gives ")
        .ok_or_else(|| place.site.error("Declare what the slot gives"))?;
    let (gives, composer, rest) = if let Some(input) = gives.strip_prefix("text ") {
        let (composer, rest) = composer(input, gives_site.after("text "), rest)?;
        (gives.to_owned(), Some(composer), rest)
    } else {
        (quoted(gives, gives_site)?, None, rest)
    };
    let [
        (picker @ ("picked by person" | "picked by space"), _),
        clauses @ ..,
    ] = rest
    else {
        return Err(rest
            .first()
            .map_or(place.site, |part| part.1)
            .error("Declare who picks the slot's filler"));
    };
    let mut slot = Slot {
        multiple: *cardinality == "many",
        gives,
        picked_by_space: *picker == "picked by space",
        recommended: None,
        input_operation: None,
        input_site: place.site,
        label: None,
        composer,
    };
    for &(clause, site) in clauses {
        if let Some(id) = clause
            .strip_prefix("recommended ")
            .filter(|_| slot.recommended.is_none())
        {
            lines::id(id, site.after("recommended "))?;
            slot.recommended = Some(id.into());
        } else if let Some(operation) = clause
            .strip_prefix("inputs ")
            .filter(|_| slot.input_operation.is_none())
        {
            slot.input_site = site.after("inputs ");
            operation_ref(operation, slot.input_site)?;
            slot.input_operation = Some(operation.into());
        } else if let Some(label) = clause
            .strip_prefix("label ")
            .filter(|_| slot.label.is_none())
        {
            slot.label = Some(quoted(label, site.after("label "))?);
        } else {
            return Err(
                site.error("Use at most one recommended plugin, inputs operation, and label")
            );
        }
    }
    if slot.composer.is_some() && slot.input_operation.is_none() {
        return Err(gives_site.error("A composer must name its typed inputs operation"));
    }
    Ok(slot)
}

type Clauses<'a, 'b> = &'a [(&'b str, Site)];

fn composer<'a, 'b>(
    input: &str,
    text_site: Site,
    parts: Clauses<'a, 'b>,
) -> Result<(ComposerBindings, Clauses<'a, 'b>), Diagnostic> {
    lines::field_name(input, text_site)?;
    let mut bindings = ComposerBindings {
        text: input.into(),
        draft: None,
        quote: None,
        text_site,
        draft_site: text_site,
        quote_site: text_site,
    };
    let mut rest = parts;
    if let Some(&(clause, site)) = rest.first()
        && let Some(key) = clause.strip_prefix("draft state.")
    {
        bindings.draft_site = site.after("draft state.");
        lines::field_name(key, bindings.draft_site)?;
        bindings.draft = Some(key.into());
        rest = &rest[1..];
    }
    if let Some(&(clause, site)) = rest.first()
        && let Some(input) = clause.strip_prefix("quote ")
    {
        bindings.quote_site = site.after("quote ");
        lines::field_name(input, bindings.quote_site)?;
        bindings.quote = Some(input.into());
        rest = &rest[1..];
    }
    Ok((bindings, rest))
}

pub(super) fn fill(place: &Section<'_>) -> Result<Fill, Diagnostic> {
    let slot = place
        .key
        .split_once(':')
        .map(|(owner, slot)| {
            lines::id(owner, place.line.site)?;
            lines::id(slot, place.line.site.after(owner).after(":"))?;
            Ok((owner.to_owned(), slot.to_owned()))
        })
        .transpose()?;

    let (binding, label) = if place.key == "receive"
        && let Some((media, operation)) = place.value.split_once(" -> operation ")
    {
        let operation_site = place.site.after(media).after(" -> operation ");
        operation_ref(operation, operation_site)?;
        let media = lines::parts(media, place.site)?;
        if media.is_empty()
            || media
                .iter()
                .any(|(kind, _)| !matches!(*kind, "url" | "text" | "file"))
        {
            return Err(place
                .site
                .error("Name the receive media as url, text or file"));
        }
        (
            FillBinding::Receive {
                media: media.into_iter().map(|(kind, _)| kind.to_owned()).collect(),
                operation: operation.into(),
            },
            None,
        )
    } else {
        let parts = place.line.parts(place.value)?;
        let [(binding, site), rest @ ..] = parts.as_slice() else {
            return Err(place
                .site
                .error("Fill the place with a frame, operation or collection"));
        };
        let binding = if let Some(frame) = binding.strip_prefix("frame ") {
            path(frame, site.after("frame "))?;
            if !frame.starts_with("frames/") {
                return Err(site.error("A frame must be inside the frames folder"));
            }
            FillBinding::Frame(frame.into())
        } else if let Some(operation) = binding.strip_prefix("operation ") {
            operation_ref(operation, site.after("operation "))?;
            FillBinding::Operation(operation.into())
        } else if let Some(collection) = binding.strip_prefix("collection ") {
            lines::id(collection, site.after("collection "))?;
            FillBinding::Collection(collection.into())
        } else {
            return Err(site.error("Fill the place with a frame, operation or collection"));
        };
        let label = match rest {
            [] => None,
            [(clause, site)] if clause.starts_with("as ") => {
                Some(quoted(&clause[3..], site.after("as "))?)
            }
            [(_, site), ..] => {
                return Err(site.error("Give the filler at most one quoted as label"));
            }
        };
        (binding, label)
    };
    Ok(Fill {
        place: place.key.into(),
        slot,
        binding,
        label,
        site: place.site,
    })
}

fn quoted(value: &str, site: Site) -> Result<String, Diagnostic> {
    if !value.starts_with('"') {
        return Err(site.error("Put the display text in double quotes"));
    }
    let label = text(value, site, 120, false)?;
    if label.trim().is_empty() {
        return Err(site.error("Supply nonempty display text"));
    }
    Ok(label)
}

fn operation_ref(value: &str, site: Site) -> Result<(), Diagnostic> {
    if let Some((kind, name)) = value.split_once('.') {
        lines::kind_id(kind, site)?;
        lines::operation_name(name, site.after(kind).after("."))
    } else {
        lines::operation_name(value, site)
    }
}
