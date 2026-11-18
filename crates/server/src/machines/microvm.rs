//! Admission and lease handling for pool machines running account microVMs.

use calternal_machine::protocol::{Down, GUEST_LOST, GuestSize, Owner};
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

/// Validate the advertised guest memory using the same rule as the machine.
pub(super) fn guest_memory(backend: &str, memory_mib: Option<u32>) -> Result<Option<u32>, Error> {
    if is_requested(backend) {
        return memory_mib
            .and_then(GuestSize::from_mib)
            .map(|size| Some(size.memory_mib))
            .ok_or_else(|| {
                Error::invalid_input("microVM sessions require a supported guest size")
            });
    }
    if memory_mib.is_some() {
        return Err(Error::invalid_input(
            "only microVM sessions report a guest size",
        ));
    }
    Ok(None)
}

/// Validate resource reports against the negotiated guest profile. Duplicate
/// account observations and samples from other backends are refused.
pub(super) fn resources(
    memory_mib: Option<u32>,
    reports: &[calternal_machine::protocol::GuestResources],
) -> Result<
    std::collections::HashMap<
        calternal_types::AccountId,
        calternal_machine::protocol::GuestResources,
    >,
    Error,
> {
    let Some(memory_mib) = memory_mib else {
        if reports.is_empty() {
            return Ok(Default::default());
        }
        return Err(Error::invalid_input(
            "only a microVM session can report guest resources",
        ));
    };
    if reports.len() > calternal_machine::protocol::MAX_GUEST_RESOURCES {
        return Err(Error::invalid_input("too many guest resource observations"));
    }
    let mut samples = std::collections::HashMap::with_capacity(reports.len());
    for report in reports {
        if report.generation.is_nil()
            || report.memory_available_bytes > u64::from(memory_mib) * 1024 * 1024
            || samples.insert(report.account_id, *report).is_some()
        {
            return Err(Error::invalid_input("invalid guest resource observation"));
        }
    }
    Ok(samples)
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
