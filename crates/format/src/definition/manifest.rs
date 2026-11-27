use super::{
    Directive, InstallOptions, PluginFile, definition, format, kind, list, name, path, required,
    sections, text,
};
use crate::{
    Diagnostic, Field,
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
    /// Other declaration lines in file order, used by host install and clients.
    pub directives: Vec<Directive>,
    /// Description text shown at install.
    pub body: String,
}

impl Manifest {
    pub(super) fn is_description_only(&self) -> bool {
        self.reads.is_empty()
            && self.calls.is_empty()
            && self.destinations.is_empty()
            && self.programs.is_empty()
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
                for place in sections(entry.children, entry.line.depth + 1)? {
                    place.alone()?;
                    if entry.key == "slots" {
                        lines::id(place.key, place.line.site)?;
                        let parts = list(place.value, place.site)?;
                        if parts.len() < 3
                            || !matches!(parts[0].as_str(), "single" | "many")
                            || !parts[1].starts_with("gives ")
                            || !matches!(parts[2].as_str(), "picked by person" | "picked by space")
                        {
                            return Err(place.site.error(
                                "Declare slot cardinality, what it gives, and who picks it",
                            ));
                        }
                        text(&parts[1][6..], place.site, 120, false)?;
                        for part in &parts[3..] {
                            let Some(id) = part.strip_prefix("recommended ") else {
                                return Err(place.site.error("Unknown slot modifier"));
                            };
                            lines::id(id, place.site)?;
                        }
                    } else if let Some(frame) = place.value.strip_prefix("frame ") {
                        let binding = lines::parts(frame, place.site)?
                            .first()
                            .map_or("", |(name, _)| *name);
                        path(binding, place.site)?;
                        if !binding.starts_with("frames/") {
                            return Err(place
                                .site
                                .error("A frame must be inside the frames folder"));
                        }
                    } else if !place.value.starts_with("operation ")
                        && !place.value.starts_with("collection ")
                        && !(place.key == "receive" && place.value.contains(" -> operation "))
                    {
                        return Err(place
                            .site
                            .error("Fill the place with a frame, operation or collection"));
                    }
                }
                result.directives.push(entry.directive()?);
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
                    for refusal in target.children {
                        kind::flat(refusal, &target)?;
                        let parts = lines::parts(refusal.content()?, refusal.site)?;
                        let [(condition, position), (message, message_site)] = parts.as_slice()
                        else {
                            return Err(refusal
                                .site
                                .error("Write one refusal condition and its message"));
                        };
                        parse::parse(condition, *position)?;
                        text(message, *message_site, 120, false)?;
                    }
                }
                return Err(site.error("Refusals need authorized target operation input schemas before they can be installed"));
            }
            "sign-ins" | "outside" | "listens" | "secrets" => {
                return Err(site.error(format!(
                    "Installing {key} needs the host capability implementation",
                    key = entry.key
                )));
            }
            "extends" => {
                return Err(
                    site.error("Plugin extensions are deferred beyond the Chat-first format slice")
                );
            }
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
            lines::operation_name(parts[parts.len() - 1], site)?;
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
