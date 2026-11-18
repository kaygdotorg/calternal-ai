//! Resolve a rootless deployment's private, local machine handoff.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_auth::machines::MachineRecord;
use calternal_machine::protocol::LocalEnrollment;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// Poll a preconfigured handoff directory on a blocking worker until the
/// machine's public-key request can be committed. No network listener is added.
/// A failed read or write is retried. Log messages carry no file contents.
pub fn start(svc: Arc<MachineService>, dir: PathBuf) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut reported = false;
        loop {
            let (svc, dir) = (svc.clone(), dir.clone());
            match crate::blocking::spawn(move || advance(&svc, &dir)).await {
                Ok(Ok(true)) => return,
                Ok(Ok(false)) => reported = false,
                _ if !reported => {
                    tracing::warn!(
                        "Co-located enrollment handoff failed. Check the owner-only handoff directory."
                    );
                    reported = true;
                }
                _ => {}
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    })
}

fn advance(svc: &MachineService, dir: &Path) -> std::io::Result<bool> {
    let private = |meta: &std::fs::Metadata| meta.permissions().mode() & 0o077 == 0;
    let meta = std::fs::symlink_metadata(dir)?;
    if !meta.is_dir() || !private(&meta) {
        return Err(std::io::Error::other("handoff directory must be private"));
    }
    let path = dir.join("request.json");
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !meta.is_file() || !private(&meta) || meta.len() > 4096 {
        return Err(std::io::Error::other("invalid enrollment handoff file"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&path)?
        .take(4097)
        .read_to_end(&mut bytes)?;
    let request: LocalEnrollment = serde_json::from_slice(&bytes)?;
    let public_key = STANDARD
        .decode(request.public_key)
        .ok()
        .filter(|key| key.len() == 32)
        .ok_or_else(|| std::io::Error::other("invalid enrollment key"))?;
    let clean = |value: &str, max| {
        value
            .chars()
            .filter(|c| !c.is_control())
            .take(max)
            .collect()
    };
    let row = MachineRecord {
        id: request.machine_id,
        owner: None,
        public_key,
        name: clean(&request.name, 64),
        os: clean(&request.os, 32),
        arch: clean(&request.arch, 32),
        revoked: false,
        direct: false,
    };
    let id = row.id;
    if !svc
        .enroll_colocated(row)
        .map_err(|error| std::io::Error::other(error.message))?
    {
        return Ok(false);
    }
    calternal_fsio::write_atomic_mode(&dir.join("enrolled"), id.to_string().as_bytes(), 0o600)?;
    Ok(true)
}

impl MachineService {
    // Wait for the protected commit under the same registry lock as network
    // enrollment so an acknowledged machine can authenticate immediately.
    fn enroll_colocated(&self, row: MachineRecord) -> Result<bool, Error> {
        let mut st = self.lock()?;
        let Some((record, grant)) = st
            .authority
            .enroll_colocated(row)
            .map_err(authority_error)?
        else {
            return Ok(false);
        };
        if !st.machines.contains_key(&record.id) {
            let row = db::MachineRow::new(record, false);
            let machine = Machine::new(row, Instant::now());
            st.readiness.insert(&machine);
            st.machines.insert(machine.row.id, machine);
        }
        if let Some(account) = grant {
            st.pool_grants.insert(account);
        }
        st.readiness_changed(Owner::Pool);
        Ok(true)
    }
}

/// Select an active machine for the first owner's setup. Requires the server
/// settings capability before revealing pool identity. Cost: one authority
/// read and one in-memory registry snapshot, no machine round trip.
pub(crate) async fn onboarding<D: crate::auth::Directory>(
    state: crate::api::Shared<D>,
    machines: Arc<MachineService>,
    account: AccountId,
) -> Result<serde_json::Value, Error> {
    blocking(move || {
        let auth = state
            .directory
            .http_auth()
            .ok_or_else(|| Error::forbidden("server settings authority is required"))?;
        if !auth
            .auth
            .has_server_capability(account, calternal_types::Capability::ServerSettings)
            .map_err(authority_error)?
        {
            return Err(Error::forbidden("server settings authority is required"));
        }
        let st = machines.lock()?;
        let candidate = st
            .machines
            .values()
            .filter(|m| {
                !m.row.revoked
                    && (m.row.owner == Owner::Pool || m.row.owner == Owner::Account { id: account })
            })
            .map(|m| {
                let ready = !m.row.direct
                    && !m.row.draining
                    && readiness::connected(m, &machines.settings)
                    && st.free(m) > 0;
                (ready, m.row.owner == Owner::Pool, m.row.id)
            })
            .min_by_key(|(ready, pool, id)| (!ready, !pool, *id));
        Ok(serde_json::json!(calternal_core::MachineOnboardingResult {
            attached: candidate.is_some(),
            ready: candidate.is_some_and(|(ready, _, _)| ready),
            machine_id: candidate.map(|(_, _, id)| id.to_string()),
        }))
    })
    .await
}
