//! Package-neutral broker and protected control adapters for execution v2.
use calternal_content::{Content, Request, execution};
use calternal_core::JsonObject;
use calternal_types::{
    AccountId, Error, HarnessRef, InstallationId, ItemRef, JobId, LeaseRef, RequestId, Result,
    SpaceId, Via,
};
use serde_json::{Value, json};

mod broker;
pub use broker::{CheckedAdmission, OwningJob, claim_job};
mod control;
pub use control::{ConfirmedStop, ControlContext, ControlScope, stop_controls};
mod recover;
pub use recover::{Recovery, ack_control, reattach, verify_attached};
mod update;
pub use update::{Admission, FenceState, MAX_FENCE_MS, Permit, UpdateGate};

fn invalid() -> Error {
    Error::invalid_input("invalid execution operands")
}

fn unavailable() -> Error {
    Error::not_found("Execution is unavailable")
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod update_tests;
