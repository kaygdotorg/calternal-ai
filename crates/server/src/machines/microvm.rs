//! Admission and lease handling for pool machines running account microVMs.

use calternal_machine::protocol::{Down, GUEST_LOST, Owner};
use calternal_types::Error;
use uuid::Uuid;

use super::{Settings, State};

pub(super) fn is_requested(backend: &str) -> bool {
    backend == "microvm"
}

pub(super) fn authorize(owner: Owner) -> Result<(), Error> {
    if owner == Owner::Pool {
        Ok(())
    } else {
        Err(Error::forbidden(
            "microVM execution is available on pool machines",
        ))
    }
}

pub(super) fn lost(
    state: &mut State,
    machine: Uuid,
    lease: Uuid,
    settings: &Settings,
    keep_files: bool,
) {
    let reason = GUEST_LOST;
    if keep_files {
        state.interrupt(lease, reason);
        state.push(
            machine,
            Down::Stop {
                lease_id: lease,
                reason: reason.into(),
                keep_files,
            },
        );
    } else {
        state.end_with_files(lease, reason, settings, keep_files);
    }
}
