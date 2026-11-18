//! Shared storage cleanup after protected owner authority has been revoked.
use crate::{
    fault::{self, FaultPoint},
    plugins::Jobs,
    store::Store,
};
use calternal_auth::spaces::Spaces;
use calternal_types::{Error, Owner};
use std::{io::ErrorKind, path::Path};

fn auth_error(e: calternal_auth::Error) -> Error {
    tracing::warn!(error = %e, "owner erasure recovery failed");
    Error::unavailable("Owner erasure is pending. Check recovery storage before retrying.")
}

/// Restore closed owners before any owner store opens at startup. One
/// protected inventory read. Return the UUIDs still awaiting physical cleanup.
pub fn restore_fence(store: &Store, spaces: &Spaces) -> Result<Vec<Owner>, Error> {
    let lifecycle = store.content().lifecycle();
    let mut pending = vec![];
    for (owner, unfinished) in spaces.erasure_records().map_err(auth_error)? {
        lifecycle.close(owner.uuid());
        if unfinished {
            pending.push(owner);
        }
    }
    Ok(pending)
}

/// Finish startup cleanup before job recovery or HTTP admission. Reuses all
/// live cleanup steps and the existing recovery coordinator.
pub fn replay(
    store: &Store,
    jobs: &Jobs,
    spaces: &Spaces,
    pending: Vec<Owner>,
) -> Result<(), Error> {
    finish(store, Some(jobs), spaces, pending)
}

/// Drain closed stores, then erase their keys and trees. Protected pending
/// records remain until all checked unlinks and directory syncs succeed.
pub(crate) fn finish(
    store: &Store,
    jobs: Option<&Jobs>,
    spaces: &Spaces,
    owners: Vec<Owner>,
) -> Result<(), Error> {
    if owners.is_empty() {
        return Ok(());
    }
    fault::hit(FaultPoint::ErasureRecorded);
    for owner in &owners {
        store.content().lifecycle().drain(owner.uuid());
        if let Some(machines) = store.machines() {
            machines.retire_owner(*owner)?;
        }
        if let Owner::Space(id) = owner {
            store.content().retire_space(*id);
            if let Some(jobs) = jobs {
                jobs.retire_space(&id.to_string());
            }
        }
    }
    fault::hit(FaultPoint::ErasureMachinesQueued);
    for owner in &owners {
        if let (Owner::Account(account), Some(jobs)) = (owner, jobs) {
            jobs.retire_account(*account)?;
        }
        store.secrets().erase_owner(owner).map_err(|error| {
            tracing::warn!(%error, "owner key erasure failed");
            Error::unavailable("Owner key erasure is pending. Check storage before retrying.")
        })?;
        fault::hit(FaultPoint::ErasureKeyRemoved);
        store
            .content()
            .unroute_owner(owner.uuid(), matches!(owner, Owner::Space(_)))?;
        let roots = store.roots();
        for root in [&roots.appdata, &roots.userdata_ssd, &roots.userdata_hdd] {
            remove_owner_tree(root, owner.directory(), &owner.uuid().to_string())?;
        }
        if matches!(owner, Owner::Space(_)) {
            remove_owner_tree(
                &roots.appdata.join("secrets"),
                "spaces",
                &owner.uuid().to_string(),
            )?;
        }
        fault::hit(FaultPoint::ErasureStorageRemoved);
    }
    spaces.finish_erasure(owners).map_err(auth_error)
}

fn remove_owner_tree(root: &Path, kind: &str, id: &str) -> Result<(), Error> {
    let failure = |error: std::io::Error| {
        tracing::warn!(%error, "owner tree erasure failed");
        Error::unavailable("Owner storage cleanup is pending. Check storage before retrying.")
    };
    let parent = root.join(kind);
    match parent.symlink_metadata() {
        Ok(meta) if meta.is_dir() => (),
        Ok(_) => {
            return Err(Error::unavailable(
                "Owner storage directory is not a real directory.",
            ));
        }
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(failure(error)),
    }
    match std::fs::remove_dir_all(parent.join(id)) {
        Ok(()) => (),
        Err(error) if error.kind() == ErrorKind::NotFound => (),
        Err(error) => return Err(failure(error)),
    }
    calternal_fsio::sync_dir(&parent).map_err(failure)
}

pub(crate) fn replay_space_delete<D: crate::auth::Directory>(
    state: &crate::api::Shared<D>,
    actor: calternal_types::AccountId,
    space: calternal_types::SpaceId,
    request_id: &str,
    input: &calternal_core::JsonObject,
) -> Result<Option<serde_json::Value>, Error> {
    let spaces = state
        .directory
        .protected_spaces()
        .ok_or_else(|| Error::unavailable("stored space directory unavailable"))?;
    calternal_core::validate_operation_input("spaces.delete", input)?;
    let receipt = spaces
        .replay_space_delete(
            actor,
            space,
            request_id.to_owned(),
            crate::api::fingerprint("spaces.delete", input),
        )
        .map_err(crate::auth::auth_error)?;
    let Some((value, pending)) = receipt else {
        return Ok(None);
    };
    if pending {
        finish(
            &state.store,
            Some(&state.jobs),
            spaces,
            vec![Owner::Space(space)],
        )?;
    }
    Ok(Some(value))
}

pub(crate) fn space<D: crate::auth::Directory>(
    state: &crate::api::Shared<D>,
    actor: calternal_types::Actor,
    space: calternal_types::SpaceId,
    request_id: &str,
    input: &calternal_core::JsonObject,
    removed: Vec<calternal_types::AccountId>,
) -> Result<serde_json::Value, Error> {
    let spaces = state
        .directory
        .protected_spaces()
        .ok_or_else(|| Error::unavailable("stored space directory unavailable"))?;
    calternal_core::validate_operation_input("spaces.delete", input)?;
    let lifecycle = state.store.content().lifecycle();
    let fingerprint = crate::api::fingerprint("spaces.delete", input);
    let result = state.store.content().admit_erasure(
        space,
        calternal_content::Request {
            account: actor.account,
            via: actor.via.as_ref(),
            id: request_id,
            operation: "spaces.delete",
            fingerprint: &fingerprint,
        },
        || {
            spaces
                .delete_fenced(
                    actor.account,
                    space,
                    (request_id, "spaces.delete", &fingerprint),
                    move |()| {
                        lifecycle.close(space.as_uuid());
                        fault::hit(FaultPoint::ErasureAuthorityCommitted);
                    },
                )
                .map_err(crate::auth::auth_error)
        },
    )?;
    finish(
        &state.store,
        Some(&state.jobs),
        spaces,
        vec![Owner::Space(space)],
    )?;
    for account in removed {
        if let Err(error) = state.inputs.erase_space(account, space) {
            tracing::warn!(
                ?error,
                "removed space's account inputs will clean on access"
            );
        }
    }
    Ok(result)
}
