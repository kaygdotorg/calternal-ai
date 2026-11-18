//! Local upgrade snapshot and activation state, held under the installation locks.

mod intent;
mod inventory;
mod proof;
mod snapshot;

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

    fn publish(&self, intent: &Intent) -> io::Result<()> {
        intent.publish(&self.roots.appdata.join(INTENT))
    }

    fn copy(&self, role: Root, index: usize) -> io::Result<PathBuf> {
        Ok(self.root(role)?.join(COPY_DIR).join(index.to_string()))
    }

    fn discard_copies(&self) -> io::Result<()> {
        for role in ROLES {
            let Ok(root) = self.root(role) else { continue };
            let folder = calternal_fsio::beneath::Folder::open_existing(root)?;
            match folder.remove_tree(COPY_DIR) {
                Ok(()) => folder.sync()?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    // The published intent follows every synced backup. No mutation is allowed
    // while a fresh snapshot has only part of its manifest.
    fn prepare(
        &self,
        previous: String,
        candidate: String,
        paths: BTreeSet<(Root, String)>,
    ) -> io::Result<()> {
        if self
            .read()?
            .is_some_and(|intent| intent.phase != Phase::Committed)
        {
            return Err(io::Error::other(
                "finish recovering the previous upgrade first",
            ));
        }
        for (_, path) in &paths {
            let relative = snapshot::relative(path)?;
            if relative.starts_with(COPY_DIR) || path == INTENT || path == ".lock" {
                return Err(io::Error::other(
                    "an upgrade cannot replace its own guard state",
                ));
            }
        }
        self.discard_copies()?;
        let mut intent = Intent::copying(self.installation, previous, candidate);
        self.publish(&intent)?;
        let mut directories = BTreeSet::new();
        let mut files = Vec::with_capacity(paths.len());
        for (index, (role, path)) in paths.into_iter().enumerate() {
            let root = self.root(role)?;
            let directory = root.join(COPY_DIR);
            if directories.insert(directory.clone()) {
                calternal_fsio::private::directory(&directory)?;
            }
            let copy = snapshot::save(root, &path, &self.copy(role, index)?)?;
            files.push(SavedFile {
                root: role,
                path,
                copy,
            });
            hit(FaultPoint::UpgradeCopied);
        }
        for directory in directories {
            calternal_fsio::sync_dir(&directory)?;
        }
        intent.phase = Phase::Preparing;
        intent.files = files;
        self.publish(&intent)?;
        hit(FaultPoint::UpgradeIntent);
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
            self.publish(&intent)?;
            hit(FaultPoint::UpgradeCommitted);
        }
        self.finish_committed(&mut intent)
    }

    fn finish_committed(&self, intent: &mut Intent) -> io::Result<()> {
        if !intent.files.is_empty() {
            self.discard_copies()?;
            intent.files.clear();
            self.publish(intent)?;
        }
        Ok(())
    }

    fn restore(&self) -> io::Result<Option<String>> {
        let Some(mut intent) = self.read()? else {
            return Ok(None);
        };
        if intent.phase == Phase::Committed {
            return Err(io::Error::other(
                "the committed upgrade cannot restore old bytes",
            ));
        }
        // Refuse before replacing even one authority if any backup is missing or corrupt.
        for (index, saved) in intent.files.iter().enumerate() {
            snapshot::relative(&saved.path)?;
            if let Some(proof) = &saved.copy {
                snapshot::validate(&self.copy(saved.root, index)?, proof)?;
            }
        }
        for (index, saved) in intent.files.iter().enumerate() {
            snapshot::restore(
                self.root(saved.root)?,
                &saved.path,
                &self.copy(saved.root, index)?,
                saved.copy.as_ref(),
            )?;
            hit(FaultPoint::UpgradeRestoreFile);
        }
        hit(FaultPoint::UpgradeRestored);
        intent.phase = Phase::Copying;
        intent.files.clear();
        self.publish(&intent)?;
        self.discard_copies()?;
        let path = self.roots.appdata.join(INTENT);
        std::fs::remove_file(&path)?;
        calternal_fsio::sync_dir(path.parent().expect("intent has a parent"))?;
        Ok(Some(intent.previous))
    }
}

/// One privileged action run with all required data locks held. An upgrade
/// caller first stops the server and the co-located machine role.
pub enum Action {
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

/// Run one local deployment action. Takes installation and optional machine
/// locks for its whole update or restore. Snapshot and migration I/O cost
/// depends on the files selected. Unknown versions fail closed.
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
                    json!({"phase":intent.phase,"previous":intent.previous,"candidate":intent.candidate,"needs_recovery":intent.phase!=Phase::Committed || !intent.files.is_empty()})
                }
                None => json!({"phase":"none","needs_recovery":false}),
            },
        );
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
            proof::check_release_ids(&previous, &candidate)?;
            if !incompatible {
                proof::require(proof.as_deref(), &previous, &candidate)?;
            }
            let inventory = inventory::Inventory::plan(&deploy, incompatible)?;
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
        Action::Status => unreachable!("handled before taking data locks"),
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

/// Refuse server startup while a snapshot still owns the release decision. The
/// host holds its validated root locks. Reads one bounded owner-only file.
pub(crate) fn check_start(appdata: &Path, installation: Uuid) -> io::Result<()> {
    if Intent::load(&appdata.join(INTENT), installation)?
        .is_some_and(|intent| intent.phase != Phase::Committed || !intent.files.is_empty())
    {
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
    let path = path.canonicalize()?;
    for root in [&roots.appdata, &roots.userdata_ssd, &roots.userdata_hdd] {
        let root = root.canonicalize()?;
        if root.starts_with(&path) || path.starts_with(&root) {
            return Err(io::Error::other(
                "the machine root must be separate from the server roots",
            ));
        }
    }
    Ok(Some(path))
}
