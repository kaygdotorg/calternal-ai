//! Narrowing declarations shared by kind and folder authorization.

use std::collections::BTreeSet;

use super::{Section, kind, list};
use crate::{
    Diagnostic,
    lines::{self, Site},
};

/// The restrictions declared for the host's authorization check.
#[derive(Clone, Debug, Default)]
pub struct Rights {
    /// The minimum role to change the kind, when narrower than the default.
    pub change: Option<String>,
    /// An allow-list of minimum roles and group IDs for reading.
    pub read: Vec<String>,
}

/// A narrowing choice for item and record changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeRight {
    /// Require at least manager access.
    Manager,
    /// Require owner access.
    Owner,
}

/// One alternative in a read audience.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadAudience<'a> {
    /// A minimum role, identified by its shared storage spelling.
    Role(&'a str),
    /// An ID to check for group membership.
    Group(&'a str),
}

impl Rights {
    /// The checked change restriction, if a declaration is present.
    pub fn change_right(&self) -> Option<ChangeRight> {
        match self.change.as_deref() {
            Some("manager") => Some(ChangeRight::Manager),
            Some("owner") => Some(ChangeRight::Owner),
            _ => None,
        }
    }

    /// Return any role or group that can satisfy this declaration.
    /// The host intersects it with applicable ancestor and kind rights.
    pub fn read_audience(&self) -> impl Iterator<Item = ReadAudience<'_>> {
        self.read.iter().map(|name| match name.as_str() {
            "reader" | "editor" | "manager" | "owner" => ReadAudience::Role(name),
            _ => ReadAudience::Group(name),
        })
    }
}

pub(super) fn parse(entry: &Section<'_>, rights: &mut Rights) -> Result<(), Diagnostic> {
    if entry.children.is_empty() {
        return right(entry.value, entry.site, rights);
    }
    entry.block()?;
    for line in entry.children {
        kind::flat(line, entry)?;
        right(line.content()?, line.site, rights)?;
    }
    Ok(())
}

fn right(text: &str, site: Site, rights: &mut Rights) -> Result<(), Diagnostic> {
    if let Some(role) = text
        .strip_prefix("change ")
        .filter(|role| matches!(*role, "manager" | "owner"))
        && rights.change.is_none()
    {
        rights.change = Some(role.into());
        return Ok(());
    }
    if let Some(read) = text
        .strip_prefix("read ")
        .filter(|_| rights.read.is_empty())
    {
        let values = list(read, site)?;
        if !values.is_empty() {
            let mut seen = BTreeSet::new();
            for audience in &values {
                lines::id(audience, site)?;
                if !seen.insert(audience) {
                    return Err(site.error("List each read role or group once"));
                }
            }
            rights.read = values;
            return Ok(());
        }
    }
    Err(site.error("Declare each right once as change manager, change owner, or read followed by roles or groups"))
}
