//! Bounded, run-scoped interactive provider input, fenced by the admitted exec.
use calternal_core::{PROVIDER_COMMAND_BYTES, ProviderCommand, ProviderRequest};
use calternal_types::{AccountId, Error};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use uuid::Uuid;

use crate::machines::MachineService;

pub(crate) struct Control {
    pub run: u32,
    account: AccountId,
    exec: Uuid,
    seq: u64,
    requests: HashMap<String, (Instant, bool)>,
}

impl Control {
    pub(super) fn new(run: u32, account: AccountId, exec: Uuid) -> Self {
        // Start and send are the initial two commands carried with the exec.
        Self {
            run,
            account,
            exec,
            seq: 3,
            requests: HashMap::new(),
        }
    }

    pub(super) fn offer(&mut self, id: &str, can_approve: bool) -> Result<(), Error> {
        self.requests
            .retain(|_, (expiry, _)| *expiry > Instant::now());
        if self.requests.len() >= 16 || self.requests.contains_key(id) {
            return Err(Error::invalid_input(
                "too many or duplicate provider approvals",
            ));
        }
        self.requests.insert(
            id.to_owned(),
            (Instant::now() + Duration::from_secs(90), can_approve),
        );
        Ok(())
    }

    fn decision(&self, request_id: &str, approved: bool) -> Result<(), Error> {
        let (_, can_approve) = self
            .requests
            .get(request_id)
            .filter(|(expiry, _)| *expiry > Instant::now())
            .ok_or_else(|| Error::not_found("provider approval expired or already resolved"))?;
        if approved && !*can_approve {
            return Err(Error::forbidden(
                "the native request would exceed the sandbox boundary",
            ));
        }
        Ok(())
    }

    pub(super) fn send(
        &mut self,
        machines: &MachineService,
        command: ProviderCommand,
    ) -> Result<(), Error> {
        let approval = match &command {
            ProviderCommand::Approve {
                request_id,
                approved,
            } => {
                self.decision(request_id, *approved)?;
                Some(request_id.clone())
            }
            _ => None,
        };
        let mut line = serde_json::to_vec(&ProviderRequest {
            version: 1,
            seq: self.seq,
            command,
        })
        .map_err(|_| Error::internal("provider command encoding failed"))?;
        line.push(b'\n');
        if line.len() > PROVIDER_COMMAND_BYTES {
            return Err(Error::invalid_input("provider command exceeds budget"));
        }
        machines.interactive_input(self.account, self.exec, line)?;
        self.seq += 1;
        if let Some(id) = approval {
            self.requests.remove(&id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_confirmation_never_grants_a_sandbox_escalation() {
        let mut control = Control::new(1, AccountId::new(), Uuid::new_v4());
        control.offer("escalation", false).unwrap();
        assert!(control.decision("escalation", false).is_ok());
        assert_eq!(
            control.decision("escalation", true).unwrap_err().code,
            calternal_types::ErrorCode::Forbidden
        );
        assert!(control.decision("foreign", false).is_err());
    }
}
