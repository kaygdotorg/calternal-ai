//! Read one harness contribution's placement pins from a checked manifest.
use super::{EntryNetwork, hostname, login_path};
use crate::{Error, ManifestV2, space_channel::SpaceMethod};

/// The pins a host needs to build an [`super::EntryProgram`] for one harness
/// contribution. Data only: approval, placement and login custody are the
/// caller's checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessDeclaration {
    /// Contributing operation ID that the owning job calls.
    pub operation: String,
    /// Normalized HTTPS OCI repository, or `None` for an operator-provisioned image.
    pub image_source: Option<String>,
    /// Pinned image digest, `sha256:<hex>`.
    pub image_digest: String,
    /// Package-relative entry asset path.
    pub entry_path: String,
    /// Pinned lowercase SHA-256 of the entry asset.
    pub entry_sha256: String,
    /// Declared login mount paths; empty when the harness declares none.
    pub login_directories: Vec<String>,
    /// Declared execution egress before person-scoped servers are resolved.
    pub network: DeclaredNetwork,
    /// Host methods the contributing operation declared.
    pub methods: Vec<SpaceMethod>,
}

/// `harness.execution_network` as declared. Missing means none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredNetwork {
    /// Exact HTTPS hosts approved at install.
    pub fixed_sites: Vec<String>,
    /// Whether the acting person's own added servers are requested.
    pub user_servers: bool,
    /// Whether any public HTTPS host is requested.
    pub any_website: bool,
}

impl DeclaredNetwork {
    /// The machine policy after the host resolved the acting person's
    /// servers. Any website subsumes the lists; empty means none. Hostnames
    /// that fail the syntax check are refused. Cost: linear in the hosts.
    pub fn resolve(&self, user_servers: &[String]) -> Result<EntryNetwork, Error> {
        if self.any_website {
            return Ok(EntryNetwork::AnyPublic);
        }
        let mut hosts = self.fixed_sites.clone();
        if self.user_servers {
            hosts.extend(user_servers.iter().cloned());
        }
        hosts.sort();
        hosts.dedup();
        if hosts.is_empty() {
            return Ok(EntryNetwork::None);
        }
        if !hosts.iter().all(|host| hostname(host)) {
            return Err(Error::Invalid);
        }
        Ok(EntryNetwork::Sites { hosts })
    }
}

/// Find the `harness` contribution `id` and its operation in a checked
/// manifest. Returns `Error::Invocation` when either is missing. Cost: one
/// scan of at most 32 contributions and 128 operations.
pub fn harness_declaration(manifest: &ManifestV2, id: &str) -> Result<HarnessDeclaration, Error> {
    let declaration = manifest.declaration();
    let contribution = declaration["contributions"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["id"] == id && c["kind"] == "harness")
        .ok_or(Error::Invocation)?;
    let harness = &contribution["harness"];
    let operation = contribution["operation"]
        .as_str()
        .ok_or(Error::Invalid)?
        .to_owned();
    let declared = declaration["operations"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|o| o["id"] == operation.as_str())
        .ok_or(Error::Invocation)?;
    let strings = |value: &serde_json::Value| -> Vec<String> {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect()
    };
    let methods = strings(&declared["methods"])
        .iter()
        .map(|m| SpaceMethod::new(m))
        .collect::<Result<_, _>>()?;
    let login_directories = strings(&harness["login_directories"]);
    if !login_directories.iter().all(|path| login_path(path)) {
        return Err(Error::Invalid);
    }
    let network = &harness["execution_network"];
    let network = DeclaredNetwork {
        fixed_sites: strings(&network["fixed_sites"]),
        user_servers: network["user_servers"] == true,
        any_website: network["any_website"] == true,
    };
    let text = |value: &serde_json::Value| value.as_str().map(str::to_owned).ok_or(Error::Invalid);
    Ok(HarnessDeclaration {
        operation,
        image_source: harness["image_source"].as_str().map(str::to_owned),
        image_digest: text(&harness["image"])?,
        entry_path: text(&harness["entry"]["path"])?,
        entry_sha256: text(&harness["entry"]["sha256"])?,
        login_directories,
        network,
        methods,
    })
}
