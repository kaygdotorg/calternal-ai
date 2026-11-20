//! A plugin declaration as data, and the one validator for it.
//!
//! A declaration names kinds (a schema over frontmatter), a grant, forms,
//! collections and a sidebar. The host never runs code for a plugin like
//! Reading List: the kind's write grant is what lets `files.create` and
//! `files.set` run for it. [`validate`] is the only place a declaration is
//! checked; field values written later are checked by [`Kind::check_fields`]
//! against the same parsed schema, so there is one schema and one reading of it.
use calternal_types::Error;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A parsed, validated plugin declaration. Cost: parsed once at install and
/// once per space at server start; held in memory after that.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    /// The plugin's ID, `calternal-<name>` for official plugins.
    pub id: String,
    /// The name people see.
    pub name: String,
    /// The kinds this plugin owns; a kind name belongs to one plugin per space.
    #[serde(default)]
    pub kinds: BTreeMap<String, Kind>,
    /// What the plugin may do beyond reading its own kinds.
    #[serde(default)]
    pub grant: Grant,
    /// Forms the client draws (create and set); the host does not interpret them
    /// beyond checking that they name the plugin's own kinds and fields.
    #[serde(default)]
    pub forms: BTreeMap<String, Value>,
    /// Saved queries the plugin ships, by name.
    #[serde(default)]
    pub collections: BTreeMap<String, Query>,
    /// The default sidebar: collection names, in order.
    #[serde(default)]
    pub sidebar: Vec<String>,
}

/// A kind: a schema over frontmatter, the folder its files live in, and
/// whether its body is a sequence of host-written records.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kind {
    /// The folder (relative to the space root) new files of this kind go in.
    pub folder: String,
    /// Typed frontmatter fields.
    pub fields: BTreeMap<String, Field>,
    /// True for record kinds (chats): the body is host-written records.
    #[serde(default)]
    pub records: bool,
    /// How a file of the kind looks as a row, card, cell; opaque to the host (clients read it).
    #[serde(default)]
    #[allow(dead_code)]
    pub looks: Value,
}

/// One typed frontmatter field.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    /// The value type.
    #[serde(rename = "type")]
    pub ty: FieldType,
    /// Whether `files.create` must be given a value.
    #[serde(default)]
    pub required: bool,
    /// The value written when none is given.
    #[serde(default)]
    pub default: Option<Value>,
    /// Which index the field is kept in (`filter`, `sort`); a query may use only indexed fields.
    #[serde(default)]
    pub index: Option<String>,
    /// The label a form shows; opaque to the host (clients read it).
    #[serde(default)]
    #[allow(dead_code)]
    pub label: Option<String>,
}

/// The field types the spike checks.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    /// One line of text.
    Text,
    /// An `http` or `https` URL.
    Url,
    /// `true` or `false`.
    Bool,
    /// A JSON number.
    Number,
}

/// A plugin's rights. Reading its own kinds is implied.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    /// Kinds the plugin may create and write files of.
    #[serde(default)]
    pub write: Vec<String>,
}

/// The one query form (design §4.2): one kind, a `where` on fields, a sort.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// The kind asked about.
    pub kind: String,
    /// Field to value, `null`, `{gt: v}` or `{in: [..]}`; all must hold.
    #[serde(default, rename = "where")]
    pub filter: Map<String, Value>,
    /// A field name or a common field (`created`), `-` first for descending.
    #[serde(default)]
    pub sort: Option<String>,
    /// At most this many results (default and cap: 500).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Fields every file has, besides its kind's own (design files §1.6).
pub const COMMON_FIELDS: &[&str] = &["id", "kind", "created"];

/// Parse and validate a declaration. This is the one validator: install and
/// server start both go through it.
///
/// # Errors
/// `invalid_input` naming the first problem: unknown keys, a folder that is not
/// a plain relative path, a grant naming a kind the plugin does not own, a
/// default of the wrong type, a form, collection or sidebar entry naming
/// something undeclared, or a query on an unindexed field. Cost: linear in the
/// declaration's size.
pub fn validate(raw: &Value) -> Result<Declaration, Error> {
    let decl: Declaration = serde_json::from_value(raw.clone())
        .map_err(|e| Error::invalid_input(format!("declaration: {e}")))?;
    let bad = |m: String| {
        Err(Error::invalid_input(format!(
            "declaration {}: {m}",
            decl.id
        )))
    };
    if decl.id.is_empty()
        || !decl
            .id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return bad("id must be lowercase letters, digits and dashes".into());
    }
    for (name, kind) in &decl.kinds {
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_lowercase()) {
            return bad(format!("kind {name}: name must be lowercase letters"));
        }
        if calternal_fsio::beneath::plain_parts(&kind.folder).is_err()
            || kind.folder.starts_with('.')
        {
            return bad(format!("kind {name}: folder must be a plain relative path"));
        }
        for (field, spec) in &kind.fields {
            if COMMON_FIELDS.contains(&field.as_str()) {
                return bad(format!("kind {name}: field {field} is a common field"));
            }
            if let Some(d) = &spec.default
                && let Err(e) = spec.check(d)
            {
                return bad(format!("kind {name}: default of {field}: {e}"));
            }
        }
    }
    for kind in &decl.grant.write {
        if !decl.kinds.contains_key(kind) {
            return bad(format!(
                "grant names kind {kind}, which it does not declare"
            ));
        }
    }
    for (name, q) in &decl.collections {
        let Some(kind) = decl.kinds.get(&q.kind) else {
            return bad(format!(
                "collection {name} asks about undeclared kind {}",
                q.kind
            ));
        };
        if let Err(e) = kind.check_query(q) {
            return bad(format!("collection {name}: {}", e.message));
        }
    }
    for entry in &decl.sidebar {
        if !decl.collections.contains_key(entry) {
            return bad(format!("sidebar names undeclared collection {entry}"));
        }
    }
    for (name, form) in &decl.forms {
        let kind = form.get("create").and_then(Value::as_str);
        if let Some(kind) = kind
            && !decl.grant.write.iter().any(|k| k == kind)
        {
            return bad(format!(
                "form {name} creates {kind} without a write grant for it"
            ));
        }
    }
    Ok(decl)
}

impl Field {
    /// Check one value against the field's type. Cost: O(value length).
    fn check(&self, value: &Value) -> Result<(), String> {
        match (self.ty, value) {
            (_, Value::Null) if !self.required => Ok(()),
            (FieldType::Text, Value::String(s)) if !s.contains('\n') && s.len() <= 1024 => Ok(()),
            (FieldType::Url, Value::String(s))
                if (s.starts_with("https://") || s.starts_with("http://"))
                    && !s.contains(char::is_whitespace)
                    && s.len() <= 2048 =>
            {
                Ok(())
            }
            (FieldType::Bool, Value::Bool(_)) | (FieldType::Number, Value::Number(_)) => Ok(()),
            (ty, _) => Err(format!("expected {ty:?}").to_lowercase()),
        }
    }
}

impl Kind {
    /// Check values for `files.create` (`complete`: required fields present,
    /// defaults filled in) or `files.set` (only the given fields). Returns the
    /// values to write, in the kind's field order.
    ///
    /// # Errors
    /// `invalid_input` naming the field (never echoing the value).
    pub fn check_fields(
        &self,
        given: &Map<String, Value>,
        complete: bool,
    ) -> Result<Vec<(String, Value)>, Error> {
        for key in given.keys() {
            if !self.fields.contains_key(key) {
                return Err(Error::input_field(
                    &format!("/fields/{key}"),
                    "a field the kind declares",
                ));
            }
        }
        let mut out = Vec::new();
        for (name, spec) in &self.fields {
            let value = match given.get(name) {
                Some(v) => v.clone(),
                None if !complete => continue,
                None if spec.required => {
                    return Err(Error::input_field(
                        &format!("/fields/{name}"),
                        "a value (required)",
                    ));
                }
                None => spec.default.clone().unwrap_or(Value::Null),
            };
            spec.check(&value)
                .map_err(|e| Error::input_field(&format!("/fields/{name}"), &e))?;
            out.push((name.clone(), value));
        }
        Ok(out)
    }

    /// Check that a query only filters and sorts on indexed or common fields.
    ///
    /// # Errors
    /// `invalid_input` naming the field.
    pub fn check_query(&self, q: &Query) -> Result<(), Error> {
        let indexed = |f: &str| {
            COMMON_FIELDS.contains(&f) || self.fields.get(f).is_some_and(|s| s.index.is_some())
        };
        for f in q.filter.keys() {
            if !indexed(f) {
                return Err(Error::invalid_input(format!(
                    "where uses unindexed field {f}"
                )));
            }
        }
        if let Some(s) = &q.sort
            && !indexed(s.trim_start_matches('-'))
        {
            return Err(Error::invalid_input(format!(
                "sort uses unindexed field {s}"
            )));
        }
        Ok(())
    }
}
