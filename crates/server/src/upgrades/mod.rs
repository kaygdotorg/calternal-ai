//! Local upgrade snapshot and activation state, held under the installation locks.

mod full;
mod intent;
mod inventory;
mod proof;
mod snapshot;
mod tree;

#[cfg(test)]
mod intent_tests;

use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
};

use calternal_roots::Roots;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::fault::{FaultPoint, hit};
use intent::{Intent, Phase, SavedFile};

const COPY_DIR: &str = ".calternal-upgrade";
const INTENT: &str = "system/upgrade-intent.json";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum Root {
    Appdata,
    Ssd,
    Hdd,
    Machine,
}

const ROLES: [Root; 4] = [Root::Appdata, Root::Ssd, Root::Hdd, Root::Machine];

/// All paths are pinned by the caller's root guards. The intent on appdata is
/// also the startup fence, even when a deployment wrapper has been killed.
struct Deployment {
    roots: Roots,
    machine: Option<PathBuf>,
    installation: Uuid,
}

impl Deployment {
    fn root(&self, role: Root) -> io::Result<&Path> {
        match role {
            Root::Appdata => Ok(&self.roots.appdata),
            Root::Ssd => Ok(&self.roots.userdata_ssd),
            Root::Hdd => Ok(&self.roots.userdata_hdd),
            Root::Machine => self.machine.as_deref().ok_or_else(|| {
                io::Error::other("a machine directory is needed to restore this upgrade")
            }),
        }
    }

    fn read(&self) -> io::Result<Option<Intent>> {
        Intent::load(&self.roots.appdata.join(INTENT), self.installation)
    }

    fn publish(&self, intent: &mut Intent) -> io::Result<()> {
        intent.publish(&self.roots.appdata.join(INTENT))
    }

    // The published intent follows every synced backup. No mutation is allowed
    // while a fresh snapshot has only part of its manifest.
    fn prepare(
        &self,
        previous: String,
        candidate: String,
        paths: BTreeSet<(Root, String)>,
    ) -> io::Result<()> {
        self.require_clean()?;
        for (_, path) in &paths {
            let relative = snapshot::relative(path)?;
            if relative.starts_with(COPY_DIR) || path == INTENT || path == ".lock" {
                return Err(io::Error::other(
                    "an upgrade cannot replace its own guard state",
                ));
            }
        }
        full::discard(self)?;
        let mut intent = Intent::copying(self.installation, previous, candidate);
        self.publish(&mut intent)?;
        let mut directories = std::collections::BTreeMap::new();
        let mut files = Vec::with_capacity(paths.len());
        for (index, (role, path)) in paths.into_iter().enumerate() {
            let root = self.root(role)?;
            if let std::collections::btree_map::Entry::Vacant(slot) = directories.entry(role) {
                let folder = tree::Tree::open(root)?;
                slot.insert(folder.mkdir(std::ffi::OsStr::new(COPY_DIR))?);
            }
            let copy = snapshot::save(root, &path, index)?;
            if let Some(proof) = &copy {
                snapshot::validate(root, index, proof)?;
            }
            files.push(SavedFile {
                root: role,
                path,
                copy,
            });
            hit(FaultPoint::UpgradeCopied);
        }
        for directory in directories.values() {
            directory.sync()?;
        }
        intent.phase = Phase::Preparing;
        intent.files = files;
        self.publish(&mut intent)?;
        hit(FaultPoint::UpgradeIntent);
        Ok(())
    }

    fn prepare_full(
        &self,
        previous: String,
        candidate: String,
        mut entries: Vec<full::Entry>,
    ) -> io::Result<()> {
        self.require_clean()?;
        full::discard(self)?;
        let mut intent = Intent::copying(self.installation, previous, candidate);
        intent.make_full();
        self.publish(&mut intent)?;
        full::capture(self, &mut entries)?;
        intent.entries = Some(entries);
        intent.phase = Phase::Preparing;
        self.publish(&mut intent)?;
        hit(FaultPoint::UpgradeIntent);
        Ok(())
    }

    fn require_clean(&self) -> io::Result<()> {
        if self.read()?.is_some_and(|intent| intent.needs_recovery()) {
            return Err(io::Error::other(
                "finish recovering the previous upgrade first",
            ));
        }
        Ok(())
    }

    fn commit(&self, candidate: &str) -> io::Result<()> {
        let mut intent = self
            .read()?
            .ok_or_else(|| io::Error::other("there is no pending upgrade"))?;
        if intent.candidate != candidate {
            return Err(io::Error::other(
                "upgrade is bound to a different candidate",
            ));
        }
        if intent.phase == Phase::Copying {
            return Err(io::Error::other("the restore point was not completed"));
        }
        if intent.phase == Phase::Preparing {
            intent.phase = Phase::Committed;
            self.publish(&mut intent)?;
            hit(FaultPoint::UpgradeCommitted);
        }
        self.finish_committed(&mut intent)
    }

    fn finish_committed(&self, intent: &mut Intent) -> io::Result<()> {
        if intent.needs_recovery() {
            full::discard(self)?;
            intent.retire();
            self.publish(intent)?;
        }
        Ok(())
    }

    fn restore(&self) -> io::Result<Option<String>> {
        let Some(mut intent) = self.read()? else {
            return Ok(None);
        };
        match intent.phase {
            Phase::Committed => {
                return Err(io::Error::other(
                    "the committed upgrade cannot restore old bytes",
                ));
            }
            Phase::Preparing => {
                if let Some(entries) = &intent.entries {
                    full::restore(self, entries)?;
                    intent.make_full();
                } else {
                    self.restore_files(&intent)?;
                    intent.files.clear();
                }
                hit(FaultPoint::UpgradeRestored);
                intent.phase = Phase::Copying;
                self.publish(&mut intent)?;
            }
            // Copying has no complete manifest. Only private backup bytes
            // can be discarded, including after an interrupted restore cleanup.
            Phase::Copying => {}
        }
        full::discard(self)?;
        let path = self.roots.appdata.join(INTENT);
        intent::remove(&path)?;
        Ok(Some(intent.previous))
    }

    fn restore_files(&self, intent: &Intent) -> io::Result<()> {
        // Refuse before replacing even one authority if any backup is missing or corrupt.
        for (index, saved) in intent.files.iter().enumerate() {
            snapshot::relative(&saved.path)?;
            if let Some(proof) = &saved.copy {
                snapshot::validate(self.root(saved.root)?, index, proof)?;
            }
        }
        for (index, saved) in intent.files.iter().enumerate() {
            snapshot::restore(
                self.root(saved.root)?,
                &saved.path,
                index,
                saved.copy.as_ref(),
            )?;
            hit(FaultPoint::UpgradeRestoreFile);
        }
        Ok(())
    }
}

/// A privileged deployment action. Preflight and status can run while both
/// services are up. The remaining actions require stopped roles and data locks.
pub enum Action {
    /// Check the evidence and restore-point footprint while both roles may serve.
    /// Acquires no writer locks and never publishes an intent.
    Preflight {
        /// Current release image digest.
        previous: String,
        /// Candidate release image digest.
        candidate: String,
        /// Explicit owner-approved full restore point and incompatible mode.
        incompatible: bool,
        /// Test-gate evidence required for unattended updates.
        proof: Option<PathBuf>,
    },
    /// Capture the named release boundary, migrate and verify changed stores.
    Prepare {
        /// Current release image digest.
        previous: String,
        /// Candidate release image digest.
        candidate: String,
        /// Explicit owner-approved full restore point and incompatible mode.
        incompatible: bool,
        /// Test-gate evidence required for unattended updates.
        proof: Option<PathBuf>,
    },
    /// Recheck the selected candidate and commit its durable activation marker.
    Commit {
        /// Candidate release image digest from the staged intent.
        candidate: String,
    },
    /// Read the atomically published decision without taking a writer lock.
    Status,
    /// Restore an incomplete deployment, replayably, before serving requests.
    Recover,
    /// Check that this release understands the current database and recovery formats.
    Verify,
}

/// Run one local deployment action. Updates and recovery hold the installation
/// and optional machine locks. A read-only preflight checks the live footprint
/// for refusals before the operator stops either role. I/O cost depends on the
/// selected files and stores. Unknown versions fail closed.
pub fn run(
    roots: Roots,
    machine: Option<PathBuf>,
    installation: Uuid,
    action: Action,
) -> io::Result<serde_json::Value> {
    use serde_json::json;
    if matches!(action, Action::Status) {
        return Ok(
            match Intent::load(&roots.appdata.join(INTENT), installation)? {
                Some(intent) => {
                    json!({"phase":intent.phase,"previous":intent.previous,"candidate":intent.candidate,"needs_recovery":intent.needs_recovery()})
                }
                None => json!({"phase":"none","needs_recovery":false}),
            },
        );
    }
    if let Action::Preflight {
        previous,
        candidate,
        incompatible,
        proof,
    } = action
    {
        check_approval(&previous, &candidate, incompatible, proof.as_deref())?;
        calternal_roots::check_pairing(&roots, installation)?;
        let machine = validate_machine_path(&roots, machine)?;
        let deploy = Deployment {
            roots,
            machine,
            installation,
        };
        deploy.require_clean()?;
        inventory::Inventory::plan(&deploy, incompatible, true)?;
        return Ok(json!({"verified":true}));
    }
    let machine = validate_machine_path(&roots, machine)?;
    let _roots = calternal_roots::prepare(&roots, installation, false)?;
    let _machine = machine
        .as_ref()
        .filter(|path| path.is_dir())
        .map(|path| calternal_machine::lock_for_upgrade(path).map_err(io::Error::other))
        .transpose()?;
    let deploy = Deployment {
        roots,
        machine,
        installation,
    };
    hit(FaultPoint::UpgradeLocked);
    match action {
        Action::Prepare {
            previous,
            candidate,
            incompatible,
            proof,
        } => {
            check_approval(&previous, &candidate, incompatible, proof.as_deref())?;
            let inventory = inventory::Inventory::plan(&deploy, incompatible, false)?;
            hit(FaultPoint::UpgradeVersionsChecked);
            let files = inventory.apply(&deploy, previous, candidate, !incompatible)?;
            hit(FaultPoint::UpgradeMigrated);
            inventory::verify(&deploy)?;
            hit(FaultPoint::UpgradeVerified);
            Ok(json!({"phase":"preparing","files":files}))
        }
        Action::Commit { candidate } => {
            inventory::verify(&deploy)?;
            deploy.commit(&candidate)?;
            Ok(json!({"phase":"committed"}))
        }
        Action::Status | Action::Preflight { .. } => {
            unreachable!("handled before taking data locks")
        }
        Action::Recover => {
            let restored = match deploy.read()? {
                Some(intent) if intent.phase != Phase::Committed => deploy.restore()?,
                Some(mut intent) => {
                    deploy.finish_committed(&mut intent)?;
                    None
                }
                None => None,
            };
            Ok(json!({"restored_previous":restored}))
        }
        Action::Verify => {
            inventory::verify(&deploy)?;
            Ok(json!({"verified":true}))
        }
    }
}

fn check_approval(
    previous: &str,
    candidate: &str,
    incompatible: bool,
    path: Option<&Path>,
) -> io::Result<()> {
    proof::check_release_ids(previous, candidate)?;
    if !incompatible {
        proof::require(path, previous, candidate)?;
    }
    Ok(())
}

/// Refuse server startup while a snapshot still owns the release decision. The
/// host holds its validated root locks. Reads one bounded owner-only file.
pub(crate) fn check_start(
    appdata: &Path,
    installation: Uuid,
    probe: Option<&str>,
) -> io::Result<()> {
    let intent = Intent::load(&appdata.join(INTENT), installation)?;
    if let Some(candidate) = probe {
        return match intent {
            Some(intent)
                if intent.phase == Phase::Preparing
                    && intent.entries.is_some()
                    && intent.candidate == candidate =>
            {
                Ok(())
            }
            _ => Err(io::Error::other(
                "a private probe requires a prepared full restore point for this image",
            )),
        };
    }
    if intent.is_some_and(|intent| intent.needs_recovery()) {
        return Err(io::Error::other(
            "An upgrade is incomplete. Recover it through the deployment command before starting the server.",
        ));
    }
    Ok(())
}

fn validate_machine_path(roots: &Roots, machine: Option<PathBuf>) -> io::Result<Option<PathBuf>> {
    let Some(path) = machine else { return Ok(None) };
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_dir() {
        return Err(io::Error::other("the machine root must be a directory"));
    }
    // Keep the configured spelling: canonicalizing a symlinked ancestor
    // would turn a redirected machine root into a valid snapshot source.
    tree::Tree::open(&path)?;
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };
    use std::os::unix::fs::MetadataExt;
    for root in [&roots.appdata, &roots.userdata_ssd, &roots.userdata_hdd] {
        let root = root.canonicalize()?;
        let other = std::fs::metadata(&root)?;
        if root.starts_with(&path)
            || path.starts_with(&root)
            || (other.dev(), other.ino()) == (metadata.dev(), metadata.ino())
        {
            return Err(io::Error::other(
                "the machine root must be separate from the server roots",
            ));
        }
    }
    Ok(Some(path))
}
