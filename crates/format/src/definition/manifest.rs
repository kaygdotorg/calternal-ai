use super::places::{self, Fill, FillBinding, Slot};
use super::{
    Directive, InstallOptions, PluginFile, definition, format, kind, list, name, path, required,
    sections, text,
};
use crate::{
    Diagnostic, Expression, Field,
    expr::parse,
    lines::{self, Site},
};
use std::collections::BTreeMap;

/// Executable or image declared by a plugin.
#[derive(Clone, Debug)]
pub struct Program {
    /// Plugin-relative file to execute or load as an image.
    /// The legacy Chat fixture declares an ID with no runnable binding.
    pub path: Option<String>,
    /// True when the bind names an image instead of an executable.
    pub image: bool,
    pub(crate) site: Site,
}

/// A compiled before-operation refusal over checked target fields and inputs.
#[derive(Clone, Debug)]
pub struct Refusal {
    /// Qualified operation in another installed plugin.
    pub target: String,
    /// Pure condition over the authorized fields and input values.
    pub condition: Expression,
    /// Message shown with the declaring plugin's name when the condition holds.
    pub message: String,
    pub(super) resolved: String,
    pub(crate) site: Site,
}

/// A checked plugin identity and its install declarations.
#[derive(Clone, Debug)]
pub struct Manifest {
    /// The plugin's ID in the space registry.
    pub id: String,
    /// The display name, shown with its source.
    pub name: String,
    /// Semantic version of the plugin.
    pub version: String,
    /// Declared icon.
    pub icon: Option<String>,
    /// Qualified kinds this plugin's code may attempt to read.
    pub reads: Vec<String>,
    /// Qualified operations this plugin may attempt to call.
    pub calls: Vec<String>,
    /// Declared outbound destinations.
    pub destinations: Vec<String>,
    /// Program bindings keyed by declaration ID.
    pub programs: BTreeMap<String, Program>,
    /// Typed settings keyed by declaration name.
    pub settings: BTreeMap<String, Field>,
    /// Declared places keyed by their plugin-local slot ID.
    pub slots: BTreeMap<String, Slot>,
    /// Qualified refusal rules in source order.
    pub refusals: Vec<Refusal>,
    /// Kind, sidebar and Home contributions in manifest order.
    pub extensions: Vec<super::Extension>,
    /// Other declaration lines in file order, used by host install and clients.
    pub directives: Vec<Directive>,
    /// Description text shown at install.
    pub body: String,
}

impl Manifest {
    /// Read checked fillers in declaration order from the installed manifest.
    pub fn fills(&self) -> impl Iterator<Item = &Fill> {
        self.directives
            .iter()
            .filter(|section| section.key == "fills")
            .flat_map(|section| &section.children)
            .filter_map(|entry| entry.fill.as_deref())
    }

    /// Find the operation named by this plugin's one fill for the specified
    /// slot. Missing, ambiguous and non-operation fills grant nothing. This
    /// reads the checked declaration in memory and performs no I/O.
    pub fn operation_fill(&self, owner: &str, slot: &str) -> Option<&str> {
        let mut fills = self.fills().filter(|fill| {
            fill.slot
                .as_ref()
                .is_some_and(|(plugin, name)| plugin == owner && name == slot)
        });
        let fill = fills.next()?;
        match &fill.binding {
            FillBinding::Operation(target) if fills.next().is_none() => Some(target),
            _ => None,
        }
    }

    pub(super) fn is_description_only(&self) -> bool {
        self.reads.is_empty()
            && self.calls.is_empty()
            && self.destinations.is_empty()
            && self.programs.is_empty()
            && self.refusals.is_empty()
            && self.directives.iter().all(|entry| {
                !matches!(
                    entry.key.as_str(),
                    "sign-ins" | "secrets" | "outside" | "listens" | "refuses" | "notifications"
                )
            })
    }
}

pub(super) fn parse(
    file: &PluginFile<'_>,
    options: &InstallOptions<'_>,
) -> Result<Manifest, Diagnostic> {
    let source = file
        .text
        .ok_or_else(|| Site::START.error("Supply plugin.md as UTF-8 text"))?;
    let document = definition(source)?;
    let entries = sections(&document.header, 0)?;
    required(&entries, &["plugin", "name", "version", "format"])?;
    let body = text(document.body, document.body_site, 4096, true)?;
    let mut result = Manifest {
        id: String::new(),
        name: String::new(),
        version: String::new(),
        icon: None,
        reads: Vec::new(),
        calls: Vec::new(),
        destinations: Vec::new(),
        programs: BTreeMap::new(),
        settings: BTreeMap::new(),
        slots: BTreeMap::new(),
        refusals: Vec::new(),
        extensions: Vec::new(),
        directives: Vec::new(),
        body,
    };
    for entry in &entries {
        let site = entry.site;
        let value = entry.value;
        if !matches!(
            entry.key,
            "programs"
                | "settings"
                | "sign-ins"
                | "slots"
                | "fills"
                | "outside"
                | "listens"
                | "notifications"
                | "refuses"
                | "extends"
        ) {
            entry.alone()?;
        }
        match entry.key {
            "plugin" => {
                lines::id(value, site)?;
                if ["space", "home", "files", "lines", "calternal", "outside"].contains(&value) {
                    return Err(site.error("This plugin ID is reserved"));
                }
                if options
                    .expected_id
                    .is_some_and(|expected| value != expected)
                {
                    return Err(
                        site.error("The declared plugin ID differs from the source's expected ID")
                    );
                }
                if value.starts_with("calternal-")
                    && !options.official.iter().any(|(id, _)| value == *id)
                {
                    return Err(
                        site.error("Only operator-approved sources may use a calternal- plugin ID")
                    );
                }
                result.id = value.into();
            }
            "name" => result.name = name(value, site)?,
            "version" => {
                value
                    .parse::<semver::Version>()
                    .map_err(|_| site.error("Use a semantic version such as 1.0.0"))?;
                result.version = value.into();
            }
            "icon" => result.icon = Some(text(value, site, 256, false)?),
            "format" => format(value, site)?,
            "needs" => result.directives.push(entry.directive()?),
            "reads" => result.reads = references(value, 2, 2, site)?,
            "calls" => result.calls = references(value, 2, 3, site)?,
            "sends out" => {
                if value != "nothing" {
                    result.destinations = list(value, site)?;
                    for host in &result.destinations {
                        if !matches!(host.as_str(), "a server you name" | "the public web") {
                            hostname(host, site)?
                        }
                    }
                }
            }
            "programs" => {
                if !value.is_empty() {
                    entry.alone()?;
                    for id in list(value, site)? {
                        lines::id(&id, site)?;
                        if result
                            .programs
                            .insert(
                                id,
                                Program {
                                    path: None,
                                    image: false,
                                    site,
                                },
                            )
                            .is_some()
                        {
                            return Err(site.error("Declare a program ID only once"));
                        }
                    }
                }
                for program in sections(entry.children, entry.line.depth + 1)? {
                    program.alone()?;
                    lines::id(program.key, program.line.site)?;
                    let (image, binding) = program.value.strip_prefix("image ").map_or_else(
                        || {
                            (
                                false,
                                program
                                    .value
                                    .strip_prefix("executable ")
                                    .unwrap_or(program.value),
                            )
                        },
                        |path| (true, path),
                    );
                    path(binding, program.site)?;
                    result.programs.insert(
                        program.key.into(),
                        Program {
                            path: Some(binding.into()),
                            image,
                            site: program.site,
                        },
                    );
                }
            }
            "settings" => {
                entry.block()?;
                result.settings = kind::fields(entry.children, entry.line.depth + 1, "settings")?;
            }
            "slots" | "fills" => {
                entry.block()?;
                let places = sections(entry.children, entry.line.depth + 1)?;
                let mut directive = entry.directive()?;
                for (place, child) in places.iter().zip(&mut directive.children) {
                    place.alone()?;
                    if entry.key == "slots" {
                        result.slots.insert(place.key.into(), places::slot(place)?);
                    } else {
                        child.fill = Some(Box::new(places::fill(place)?));
                    }
                }
                result.directives.push(directive);
            }
            "notifications" => {
                entry.block()?;
                for notice in sections(entry.children, entry.line.depth + 1)? {
                    notice.alone()?;
                    lines::operation_name(notice.key, notice.line.site)?;
                    let parts = lines::parts(notice.value, notice.site)?;
                    let Some((message, site)) = parts.first() else {
                        return Err(notice.site.error("Give the notification a template"));
                    };
                    let message = text(message, *site, 120, false)?;
                    let mut placeholder = false;
                    let fixed: String = message
                        .chars()
                        .filter(|ch| {
                            if *ch == '{' {
                                placeholder = true;
                            }
                            if *ch == '}' {
                                placeholder = false;
                                return false;
                            }
                            !placeholder
                        })
                        .collect();
                    if fixed
                        .split_whitespace()
                        .filter(|word| word.chars().any(char::is_alphabetic))
                        .count()
                        < 3
                        || placeholder
                    {
                        return Err(site.error(
                            "Include at least three fixed words in the notification template",
                        ));
                    }
                }
                result.directives.push(entry.directive()?);
            }
            "refuses" => {
                entry.block()?;
                for target in sections(entry.children, entry.line.depth + 1)? {
                    let key = target
                        .key
                        .strip_prefix('[')
                        .and_then(|key| key.strip_suffix(']'))
                        .ok_or_else(|| {
                            target
                                .line
                                .site
                                .error("Put the qualified refusal target in square brackets")
                        })?;
                    references(key, 2, 3, target.line.site)?;
                    target.block()?;
                    if target.children.is_empty() {
                        return Err(target
                            .site
                            .error("Declare at least one condition for this refusal target"));
                    }
                    for refusal in target.children {
                        kind::flat(refusal, &target)?;
                        let parts = lines::parts(refusal.content()?, refusal.site)?;
                        let [(condition, position), (message, message_site)] = parts.as_slice()
                        else {
                            return Err(refusal
                                .site
                                .error("Write one refusal condition and its message"));
                        };
                        if !message.starts_with('"') || !message.ends_with('"') {
                            return Err(
                                message_site.error("Put the refusal sentence in double quotes")
                            );
                        }
                        let sentence = text(message, *message_site, 120, false)?;
                        if sentence.trim().is_empty() {
                            return Err(message_site.error("Write a nonempty refusal sentence"));
                        }
                        result.refusals.push(Refusal {
                            target: key.into(),
                            condition: parse::ordinary(condition, *position)?,
                            message: sentence,
                            resolved: String::new(),
                            site: refusal.site,
                        });
                    }
                }
            }
            "sign-ins" | "outside" | "listens" | "secrets" => {
                return Err(site.error(format!(
                    "Installing {key} needs the host capability implementation",
                    key = entry.key
                )));
            }
            "extends" => result.extensions = super::extensions::parse(entry)?,
            key => return Err(entry.line.site.error(format!("Unknown plugin key {key}"))),
        }
    }
    #[cfg(feature = "unicode")]
    {
        let skeleton = |n: &str| {
            unicode_security::skeleton(n)
                .collect::<String>()
                .to_lowercase()
        };
        if options
            .official
            .iter()
            .any(|(id, name)| *id != result.id && skeleton(name) == skeleton(&result.name))
        {
            return Err(entries
                .iter()
                .find(|entry| entry.key == "name")
                .unwrap()
                .site
                .error("Use a name distinct from official plugins"));
        }
    }
    Ok(result)
}

fn references(value: &str, min: usize, max: usize, site: Site) -> Result<Vec<String>, Diagnostic> {
    let values = list(value, site)?;
    for value in &values {
        let parts: Vec<_> = value.split('.').collect();
        if !(min..=max).contains(&parts.len()) {
            return Err(site.error("Use a qualified plugin reference"));
        }
        lines::id(parts[0], site)?;
        if parts.len() == 3 {
            lines::kind_id(parts[1], site)?
        }
        if parts[parts.len() - 1].contains(' ') {
            if max == 2 {
                return Err(site.error("A referenced kind uses an ID"));
            }
            let operation = parts[parts.len() - 1];
            if !matches!(
                operation,
                "append record" | "change record" | "remove record"
            ) {
                lines::operation_name(operation, site)?;
            }
        } else {
            lines::id(parts[parts.len() - 1], site)?;
        }
    }
    Ok(values)
}

pub(super) fn hostname(host: &str, site: Site) -> Result<(), Diagnostic> {
    if host.len() > 253
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == b'-')
        })
    {
        return Err(site.error("Use an ASCII hostname without a URL path"));
    }
    Ok(())
}
