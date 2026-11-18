//! Wire shapes and admission metadata for the palette and pickers.
use crate::{JsonObject, OperationDescription, OperationRisk, PeopleTarget};
use calternal_types::{Capability, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Search visible items by title and body. An omitted space searches all readable spaces.
#[derive(Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SearchInput {
    /// Text typed into the palette.
    pub query: String,
    /// Optional space UUID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub space_id: Option<String>,
    /// Kind filter. An empty list selects all opted-in kinds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub kinds: Vec<String>,
    /// Maximum rows to return, from 1 to 50. Defaults to 20.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<u32>,
    /// Opaque continuation from this operation and filter set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cursor: Option<String>,
}

impl SearchInput {
    /// Bound decoding and validate identifiers without consulting storage.
    pub fn validate(&self) -> Result<(), Error> {
        validate_text(&self.query)?;
        if let Some(space) = &self.space_id {
            crate::validate_resource_id(space)?;
        }
        if self.kinds.len() > 32 || self.limit.is_some_and(|n| !(1..=50).contains(&n)) {
            return Err(Error::invalid_input(
                "search accepts up to 32 kinds and a limit of 1 through 50",
            ));
        }
        for kind in &self.kinds {
            crate::validate_id(kind)?;
        }
        if let Some(cursor) = &self.cursor {
            crate::validate_page_cursor(cursor)?;
        }
        Ok(())
    }
}

/// Picker filter for cross-space item and thread lookups.
#[derive(Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct LookupInput {
    /// Literal title prefix.
    pub prefix: String,
    /// Optional space UUID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub space_id: Option<String>,
    /// Maximum rows, from 1 to 50. Defaults to 20.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<u32>,
    /// Opaque continuation from the same lookup and filters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cursor: Option<String>,
}

/// Picker filter within one space.
#[derive(Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceLookupInput {
    /// Selected space UUID.
    pub space_id: String,
    /// Literal display-name prefix. Omit to browse.
    #[serde(default)]
    #[ts(as = "Option<String>", optional)]
    pub prefix: String,
}

/// One plain-text excerpt segment. Render its text literally.
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct SearchSegment {
    /// Literal display text.
    pub text: String,
    /// Whether the segment matches the search.
    pub matched: bool,
}

/// One indexed item in its visible space.
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct SearchHit {
    /// Stable item address.
    pub address: String,
    /// Manifest kind identifier.
    pub kind: String,
    /// Owning space UUID.
    pub space_id: String,
    /// Display title.
    pub title: String,
    /// Short title or body excerpt with structured highlights.
    pub excerpt: Vec<SearchSegment>,
    /// Stable record UUID on a message hit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub message_id: Option<String>,
    /// Saved RFC 3339 message time, with its original offset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub message_at: Option<String>,
}

/// Visible results in deterministic order.
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct SearchResult {
    /// The current page.
    pub hits: Vec<SearchHit>,
    /// Opaque continuation, omitted at the end.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<String>,
    /// A readable selected index is catching up, rebuilding, or has reached its coverage limit.
    pub partial: bool,
}

/// A named account or group directly granted access to the selected space.
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct PersonMatch {
    /// Account or group UUID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Subject type, either account or group.
    pub kind: String,
}

/// The first page of matching mention subjects.
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct PeopleLookupResult {
    /// Matching direct accounts and groups.
    pub people: Vec<PersonMatch>,
}

/// One externally placed space skill.
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct SkillMatch {
    /// Skill directory name.
    pub name: String,
    /// One-line description declared in the file.
    pub description: String,
    /// Harness labels selecting the skill. Empty means all eligible agents.
    #[serde(default)]
    pub agents: Vec<String>,
    /// Path to SKILL.md relative to the space.
    #[serde(default)]
    pub path: String,
    /// Stable content item for an application-managed skill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub id: Option<String>,
}

/// Bounded skill matches for the selected space.
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct SkillsResult {
    /// Skill descriptors, ordered by name.
    pub skills: Vec<SkillMatch>,
}

fn validate_text(value: &str) -> Result<(), Error> {
    if value.len() > 512 || value.chars().any(char::is_control) {
        return Err(Error::invalid_input(
            "search text must be at most 512 bytes with no controls",
        ));
    }
    Ok(())
}

/// Host-owned read operations for the palette and pickers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SearchOperation {
    /// Full-text palette results.
    Query,
    /// Cross-kind title prefix.
    Items,
    /// Thread title prefix.
    Threads,
    /// Direct mention targets.
    People,
    /// Per-space skills.
    Skills,
    /// Enabled plugin palette declarations.
    Manifests,
}

macro_rules! operations {
    ($($variant:ident: $id:literal, $description:literal, $target:ident, $input:ty => $output:ty;)+) => {
        impl SearchOperation {
            /// Operations in catalogue order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Look up a wire operation without allocating.
            pub fn from_id(id: &str) -> Option<Self> {
                match id { $($id => Some(Self::$variant),)+ _ => None }
            }

            /// Stable operation identifier for dispatch and clients.
            pub fn id(self) -> &'static str {
                match self { $(Self::$variant => $id,)+ }
            }

            /// Minimum authority at the selected space or each result space.
            pub const CAPABILITY: Capability = Capability::SpaceRead;

            /// Declared action class used by agent admission.
            pub const RISK: OperationRisk = OperationRisk::Read;

            /// Authority target. Account-scoped queries check every returned space.
            pub fn target(self) -> PeopleTarget {
                match self { $(Self::$variant => PeopleTarget::$target),+ }
            }

            pub(crate) fn describe(self) -> OperationDescription {
                match self {
                    $(Self::$variant => OperationDescription {
                        required_capability: Some(Self::CAPABILITY.name().into()),
                        target: Some(self.target()),
                        risk: Self::RISK,
                        id: $id.into(),
                        description: $description.into(),
                        mutates: false,
                        input_schema: serde_json::from_value(serde_json::json!(schemars::schema_for!($input))).expect("object schema"),
                        output_schema: serde_json::from_value(serde_json::json!(schemars::schema_for!($output))).expect("object schema"),
                    }),+
                }
            }
        }
    }
}

operations! {
    Query: "search.query", "Search titles and bodies across readable spaces.", Account, SearchInput => SearchResult;
    Items: "items.lookup", "Find indexed items by title prefix across readable spaces.", Account, LookupInput => SearchResult;
    Threads: "threads.lookup", "Find threads by title prefix across readable spaces.", Account, LookupInput => SearchResult;
    People: "people.lookup", "Find mentionable accounts and groups in a space.", Space, SpaceLookupInput => PeopleLookupResult;
    Skills: "skills.list", "List visible space skills and their descriptions.", Space, SpaceLookupInput => SkillsResult;
    Manifests: "plugins.manifests", "List palette contributions from enabled plugins.", Space, crate::PluginsListInput => crate::PaletteResult;
}

impl SearchOperation {
    /// Validate bounded request values. Authority is checked by the host.
    pub(crate) fn validate(self, input: &JsonObject) -> Result<(), Error> {
        use crate::validation::decode;
        match self {
            Self::Query => decode::<SearchInput>(input)?.validate(),
            Self::Items | Self::Threads => {
                let i: LookupInput = decode(input)?;
                SearchInput {
                    query: i.prefix,
                    space_id: i.space_id,
                    kinds: Vec::new(),
                    limit: i.limit,
                    cursor: i.cursor,
                }
                .validate()
            }
            Self::People | Self::Skills => {
                let i: SpaceLookupInput = decode(input)?;
                crate::validate_resource_id(&i.space_id)?;
                validate_text(&i.prefix)
            }
            Self::Manifests => {
                crate::validate_resource_id(&decode::<crate::PluginsListInput>(input)?.space_id)
            }
        }
    }
}
