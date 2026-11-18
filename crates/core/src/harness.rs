//! Administrator-visible harness release pins and smoke-test inputs.

/// Immutable artifacts for an operator-reviewed provider runtime. The binaries
/// run on the enrolled machine, inside the thread's sandbox.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct HarnessPin {
    /// Exact official CLI version reported by the adapter.
    pub version: String,
    /// Integrity digest of the CLI artifact for this deployment.
    pub package_sha256: String,
    /// Full local OCI image ID, including its sha256 prefix.
    pub image: String,
    /// The adapter accepts stop, steer and approval responses during a turn.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub interactive: bool,
    /// Verified guest-image manifest digest, when a MicroVM image is staged.
    pub guest_image_digest: Option<String>,
}

/// Public authentication instructions and deployment artifacts approved by the
/// operator. The enrollment flow owns any credential bytes separately.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HarnessOffer {
    /// Exact candidate the smoke test will run.
    pub pin: HarnessPin,
    /// Display-only command for manual authentication in the sandbox.
    pub login_command: String,
    /// Exact HTTPS origins and paths emitted by the adapter, without queries.
    pub login_locations: Vec<String>,
}

/// Smoke-test a candidate package runtime on a selected enrolled machine.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HarnessStage {
    /// Installed provider package to load.
    pub id: String,
    /// Accessible space used to acquire a disposable lease.
    pub space_id: String,
    /// Enrolled machine authorized for private diagnostics.
    pub machine_id: String,
    /// MicroVM machine to qualify when the candidate includes a guest digest.
    #[serde(default)]
    pub guest_machine_id: Option<String>,
    /// Operator-approved candidate and public authentication information.
    pub offer: HarnessOffer,
}

/// Select a tested runtime or restore the previous active runtime.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HarnessSelect {
    /// Installed provider package identifier.
    pub id: String,
    /// Exact immutable image ID observed in the list.
    pub image: String,
}

/// One installation-level runtime and its available changes.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct HarnessStatus {
    /// Installed provider package identifier.
    pub id: String,
    /// Human-readable provider name.
    pub name: String,
    /// Whether the package has a bundled legacy runtime.
    pub bundled: bool,
    /// Current immutable runtime pin, if one was activated.
    pub active: Option<HarnessPin>,
    /// Prior pin offered for rollback.
    pub previous: Option<HarnessPin>,
    /// Candidate offered for activation after a successful smoke test.
    pub tested: Option<HarnessPin>,
}

/// Catalog of installed harnesses visible to the installation administrator.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct HarnessList {
    /// Installed provider packages and their runtime status.
    pub harnesses: Vec<HarnessStatus>,
}
