//! Wire shapes and admission metadata for the palette and pickers.
use crate::{JsonObject, OperationDescription, OperationRisk, PeopleTarget};
use calternal_types::{Capability, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
            "lookup text must be at most 512 bytes with no controls",
        ));
    }
    Ok(())
}

/// Host-owned read operations for the palette and pickers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SearchOperation {
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

            /// Minimum authority at the selected space.
            pub const CAPABILITY: Capability = Capability::SpaceRead;

            /// Declared action class used by agent admission.
            pub const RISK: OperationRisk = OperationRisk::Read;

            /// Authority target for the selected space.
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
    People: "people.lookup", "Find mentionable accounts and groups in a space.", Space, SpaceLookupInput => PeopleLookupResult;
    Skills: "skills.list", "List visible space skills and their descriptions.", Space, SpaceLookupInput => SkillsResult;
    Manifests: "plugins.manifests", "List palette contributions from enabled plugins.", Space, crate::PluginsListInput => crate::PaletteResult;
}

impl SearchOperation {
    /// Validate bounded request values. Authority is checked by the host.
    pub(crate) fn validate(self, input: &JsonObject) -> Result<(), Error> {
        use crate::validation::decode;
        match self {
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
