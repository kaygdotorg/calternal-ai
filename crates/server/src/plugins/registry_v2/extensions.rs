//! Checked contributions derived from the installed catalogue.

use super::{Catalog, ManifestV2, declared};
use calternal_content::contribution_spool::{Ingest, Spool};
use calternal_plugin_host::ContributionInterface;
use calternal_types::{
    ContributionId, ContributionIdentity, Error, HarnessRef, PackageId, RequestId, Result,
};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

/// One archive's manifest and monotonic installation generation.
#[derive(Clone, Debug)]
pub(in crate::plugins) struct InstalledV2 {
    /// Manifest checked against the archive's exact asset pins.
    pub(in crate::plugins) manifest: Arc<ManifestV2>,
    /// Installed archive row ID, which changes even for a byte-identical reinstall.
    pub(in crate::plugins) revision: u32,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct PointKey(String, String, String, u16, String);

/// A package contribution proven to match an installed point and owned operation.
/// Callers must check their space, placement and attempt to use this binding.
#[derive(Debug)]
#[allow(dead_code, reason = "Slices 3, 4 and 6 consume the pinned binding")]
pub(crate) struct CheckedContribution {
    /// Logical identity for the checked choice or admitted job context.
    pub(crate) identity: ContributionIdentity,
    /// Installed generation of the package that owns the point.
    pub(crate) owner_revision: u32,
    /// Installed generation of the contributing package.
    pub(crate) contributor_revision: u32,
    /// Checked manifest declaration including operation and any executable asset pin.
    pub(crate) declaration: Value,
    /// Cached validator for the matching owner and contributor signatures.
    pub(crate) interface: ContributionInterface,
}

#[allow(
    dead_code,
    reason = "Slice 3's contribution broker and Slice 4's producer route consume these hooks"
)]
impl CheckedContribution {
    /// Reserve one call in the owning job's existing receipt transaction.
    /// The broker supplies the current contribution, checked attempt and
    /// exact admitted input before the spool may observe a fresh request.
    /// Cost: bounded preflight and the spool reservation.
    pub(crate) fn reserve_call(
        &self,
        spool: &Spool<'_, '_>,
        harness: &HarnessRef,
        admitted: &Value,
        offered: &Value,
        now: i64,
    ) -> Result<Uuid> {
        if admitted != offered {
            return Err(Error::request_reused());
        }
        self.interface.preflight(harness, admitted)?;
        spool.reserve(now)
    }

    /// Claim launch once and return the exact entry frame. Replay returns
    /// no entry bytes for dispatch. Start the carrier only after commit.
    /// Cost: one bounded validation, two bounded encodings and one indexed update.
    pub(crate) fn claim_entry(
        &self,
        spool: &Spool<'_, '_>,
        request: &RequestId,
        harness: &HarnessRef,
        call: Uuid,
        admitted: &Value,
    ) -> Result<Option<Vec<u8>>> {
        let wire = self.interface.encode_entry(
            request,
            spool.execution(),
            harness,
            &call.to_string(),
            admitted,
            admitted,
        )?;
        Ok(spool.claim_launch(call)?.then_some(wire))
    }

    /// Validate and persist one producer frame using the checked output
    /// signature and exact attempt. Acknowledge the result after commit.
    /// Cost: one bounded output validation for new output and one spool append.
    pub(crate) fn ingest_frame(
        &self,
        spool: &Spool<'_, '_>,
        call: Uuid,
        frame: &Value,
        now: i64,
    ) -> Result<Ingest> {
        spool.ingest(call, frame, now, |data| self.interface.accepts_output(data))
    }
}

/// Immutable catalogue index. Maximum entries are the installed package cap
/// times the manifest's contribution cap.
#[derive(Clone, Debug, Default)]
pub(in crate::plugins) struct ContributionIndex(BTreeMap<PointKey, PointChoices>);

type PointChoices = BTreeMap<(String, String), Arc<CheckedContribution>>;

pub(super) type PendingContribution = (PointKey, Arc<CheckedContribution>);

impl ContributionIndex {
    pub(super) fn extend(&mut self, choices: impl IntoIterator<Item = PendingContribution>) {
        for (point, contribution) in choices {
            let identity = &contribution.identity;
            let key = (identity.package.0.clone(), identity.contribution.0.clone());
            self.0.entry(point).or_default().insert(key, contribution);
        }
    }

    pub(super) fn withdraw(&mut self, package: &str) {
        self.0.retain(|point, contributions| {
            if point.0 == package {
                return false;
            }
            contributions.retain(|(contributor, _), _| contributor != package);
            !contributions.is_empty()
        });
    }
}

fn id<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .expect("checked extension declaration")
}

impl PointKey {
    fn from_contribution(contribution: &Value) -> Self {
        let app = &contribution["app"];
        Self(
            id(app, "package").into(),
            id(app, "app").into(),
            id(contribution, "point").into(),
            contribution["version"]
                .as_u64()
                .expect("checked point version") as u16,
            id(contribution, "kind").into(),
        )
    }
}

impl Catalog {
    /// Check the current binding and encoded input before the existing job or
    /// alarm admission writes its durable pin. Callers resolve the harness
    /// for this actor, space and target and supply live package enablement.
    /// A replaced or disabled package cannot invoke the admission callback.
    /// Cost: two in-memory lookups and bounded preflight, with no storage I/O.
    #[allow(dead_code, reason = "Slices 3 and 7 own the host admission routes")]
    pub(crate) fn preflight_admission<T>(
        &self,
        owner: &PackageId,
        selected: &CheckedContribution,
        harness: &HarnessRef,
        input: &Value,
        enabled: impl Fn(&PackageId) -> bool,
        admit: impl FnOnce(&ContributionIdentity, &Value) -> Result<T>,
    ) -> Result<T> {
        if selected.declaration["app"]["package"] != owner.0 {
            return Err(Error::not_found("The selected contribution is unavailable"));
        }
        let current = self.current_contribution(selected, enabled)?;
        current.interface.preflight(harness, input)?;
        admit(&current.identity, input)
    }

    /// Recover a selected contribution only while both original package
    /// generations are installed and enabled for the checked space.
    /// Cost: two in-memory lookups and at most two enablement checks.
    pub(crate) fn current_contribution(
        &self,
        selected: &CheckedContribution,
        enabled: impl Fn(&PackageId) -> bool,
    ) -> Result<&CheckedContribution> {
        let point = PointKey::from_contribution(&selected.declaration);
        let key = (
            selected.identity.package.0.clone(),
            selected.identity.contribution.0.clone(),
        );
        self.contributions
            .0
            .get(&point)
            .and_then(|choices| choices.get(&key))
            .filter(|current| {
                selected.owner_revision == current.owner_revision
                    && selected.contributor_revision == current.contributor_revision
                    && enabled(&PackageId(point.0))
                    && enabled(&current.identity.package)
            })
            .map(Arc::as_ref)
            .ok_or_else(|| Error::not_found("The selected contribution is unavailable"))
    }

    /// List checked contributions for an exact qualified interface. Both
    /// packages must be enabled by the caller's live space predicate.
    /// Cost: one in-memory lookup and one check per candidate, no wake or I/O.
    /// Slice 6 binds renderer choices to its checked activation and placement.
    #[allow(dead_code, reason = "Slice 6 consumes the checked inventory")]
    pub(crate) fn checked_contributions<'a>(
        &'a self,
        owner: &PackageId,
        app: &str,
        point: &str,
        version: u16,
        kind: &str,
        enabled: impl Fn(&PackageId) -> bool + 'a,
    ) -> impl Iterator<Item = &'a CheckedContribution> + 'a {
        let key = PointKey(
            owner.0.clone(),
            app.into(),
            point.into(),
            version,
            kind.into(),
        );
        self.contributions
            .0
            .get(&key)
            .filter(|_| enabled(owner))
            .into_iter()
            .flat_map(BTreeMap::values)
            .filter(move |choice| enabled(&choice.identity.package))
            .map(Arc::as_ref)
    }

    fn check_contribution(
        &self,
        installed: &InstalledV2,
        contribution: &Value,
    ) -> std::result::Result<PendingContribution, String> {
        let target = &contribution["app"];
        let app_package = id(target, "package");
        let app_id = id(target, "app");
        let point_id = id(contribution, "point");
        let kind = id(contribution, "kind");
        let version = contribution["version"]
            .as_u64()
            .expect("checked point version") as u16;
        let owner = if app_package == installed.manifest.id() {
            Some(installed)
        } else {
            self.v2.get(app_package)
        };
        let owner =
            owner.ok_or_else(|| format!("Extension owner {app_package} is not installed"))?;
        let point = declared(&owner.manifest, "apps")
            .find(|app| app["id"] == app_id)
            .into_iter()
            .flat_map(|app| app["extension_points"].as_array().into_iter().flatten())
            .find(|point| point["id"] == point_id && point["version"].as_u64() == Some(u64::from(version)))
            .ok_or_else(|| format!("Extension point {app_package}/{app_id}/{point_id} version {version} is not installed"))?;
        if point["kind"] != kind {
            return Err(format!(
                "Extension point {point_id} does not declare contribution kind {kind}"
            ));
        }
        let operation = declared(&installed.manifest, "operations")
            .find(|operation| operation["id"] == contribution["operation"])
            .expect("manifest verifies contribution operation ownership");
        let interface = ContributionInterface::check(point, operation)?;
        Ok((
            PointKey::from_contribution(contribution),
            Arc::new(CheckedContribution {
                identity: ContributionIdentity {
                    package: PackageId(installed.manifest.id().into()),
                    contribution: ContributionId(id(contribution, "id").into()),
                },
                owner_revision: owner.revision,
                contributor_revision: installed.revision,
                declaration: contribution.clone(),
                interface,
            }),
        ))
    }

    pub(super) fn check_installed_contributions(
        &self,
        installed: &InstalledV2,
    ) -> std::result::Result<Vec<PendingContribution>, String> {
        declared(&installed.manifest, "contributions")
            .map(|contribution| self.check_contribution(installed, contribution))
            .collect()
    }

    fn index_stored(&self, owner: Option<&str>) -> Vec<PendingContribution> {
        self.v2.values().flat_map(|installed| {
            declared(&installed.manifest, "contributions").filter_map(move |entry| {
                if owner.is_some_and(|owner| installed.manifest.id() == owner || entry["app"]["package"] != owner) {
                    return None;
                }
                match self.check_contribution(installed, entry) {
                    Ok(contribution) => Some(contribution),
                    Err(reason) => {
                        tracing::warn!(package = installed.manifest.id(), contribution = id(entry, "id"), %reason, "installed contribution unavailable");
                        None
                    }
                }
            })
        }).collect()
    }

    /// Rebuild eligibility after every installed package and its dependencies
    /// have been loaded. Missing or invalid relationships remain unavailable.
    pub(in crate::plugins) fn index_stored_contributions(&mut self) {
        self.contributions = ContributionIndex::default();
        let checked = self.index_stored(None);
        self.contributions.extend(checked);
    }

    pub(super) fn reindex_owner(&mut self, owner: &str) {
        let checked = self.index_stored(Some(owner));
        self.contributions.extend(checked);
    }
}
