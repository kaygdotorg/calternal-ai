//! Wire shapes for the space carrier, independent of the child worker adapter.
use super::*;
use calternal_types::ErrorCode;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Package identity pinned by the authenticated assignment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackagePin {
    /// Package identifier in the approved registry.
    pub package: String,
    /// Lowercase SHA-256 digest of the assigned bundle.
    pub digest: String,
}

/// The methods admitted to the v2 callback wire.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceMethod {
    /// Return the head of one item in the ticket's space.
    ItemHead,
}

/// Direction-checked invocation, callback and transport messages.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpaceMessage {
    /// Host launches one checked entry on an execution carrier.
    StartEntry {
        /// Pinned image, executable asset and checked input.
        program: crate::entry::EntryProgram,
    },
    /// Host dispatches an operation within its invocation deadline.
    Invoke {
        /// Pinned operation name.
        operation: String,
        /// Checked operation input.
        input: Value,
        /// Relative deadline in milliseconds.
        deadline_ms: u32,
    },
    /// Supervisor forwards a package's broker callback.
    Call {
        /// Caller-local callback identifier.
        callback_id: Uuid,
        /// Requested v2 engine method.
        method: SpaceMethod,
        /// Method input, validated by the engine.
        input: Value,
    },
    /// Host answers the same callback.
    Reply {
        /// Identifier from the corresponding call.
        callback_id: Uuid,
        /// Typed result or caller-safe error code.
        result: Result<Value, ErrorCode>,
    },
    /// Supervisor forwards transient progress.
    Chunk {
        /// Bounded progress event.
        data: Value,
    },
    /// Supervisor returns the terminal outcome.
    Complete {
        /// Bounded terminal result or caller-safe error code.
        result: Result<Value, ErrorCode>,
    },
    /// Host cancels one invocation ticket.
    Cancel {},
    /// Supervisor confirms readiness for one pinned package.
    Ready {},
    /// Return encoded bytes released by the receiver for this package.
    Credit {
        /// Bytes released, inclusive of LF.
        bytes: usize,
        /// Number of application frames released.
        frames: usize,
    },
    /// Host extends the remote space lease by the negotiated interval.
    Renew {},
    /// Host drains an idle placement and asks its supervisor to save it.
    Sleep {},
    /// Host fences the entire space assignment.
    Fence {},
}

impl SpaceMessage {
    pub(super) fn control(&self) -> bool {
        matches!(
            self,
            Self::Cancel {}
                | Self::Ready {}
                | Self::Credit { .. }
                | Self::Renew {}
                | Self::Sleep {}
                | Self::Fence {}
        )
    }

    pub(super) fn permits(&self, host: bool) -> bool {
        match self {
            Self::StartEntry { .. } => host,
            Self::Credit { bytes, frames } => *bytes > 0 && *frames > 0,
            Self::Invoke {
                operation,
                deadline_ms,
                ..
            } => {
                host && crate::registry::identifier(operation) && (1..=60_000).contains(deadline_ms)
            }
            Self::Reply { callback_id, .. } => host && !callback_id.is_nil(),
            Self::Call { callback_id, .. } => !host && !callback_id.is_nil(),
            Self::Renew {} | Self::Fence {} | Self::Sleep {} | Self::Cancel {} => host,
            Self::Ready {} | Self::Chunk { .. } | Self::Complete { .. } => !host,
        }
    }
}

fn required_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(deserializer)
}

/// One sequenced envelope from an authenticated space endpoint.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceFrame {
    /// Negotiated space channel major.
    pub v: u8,
    /// Host-issued runtime epoch.
    pub epoch: Uuid,
    /// Consecutive direction-local ordinal, starting at one.
    pub seq: u64,
    /// Pinned package, or null for space-scoped control.
    #[serde(deserialize_with = "required_nullable")]
    pub package: Option<String>,
    /// Pinned bundle digest, or null for space-scoped control.
    #[serde(deserialize_with = "required_nullable")]
    pub digest: Option<String>,
    /// Invocation ticket, or null for channel and package control.
    #[serde(deserialize_with = "required_nullable")]
    pub ticket: Option<Uuid>,
    /// Payload interpreted according to direction and scope.
    pub message: SpaceMessage,
}

impl SpaceFrame {
    pub(super) fn encode(&self, limit: usize) -> Result<Vec<u8>, Error> {
        let mut bytes = crate::bounded::json(self, limit - 1)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub(super) fn route(&self, host: bool) -> Result<Option<&str>, Error> {
        if !self.message.permits(host) {
            return Err(Error::Invalid);
        }
        if matches!(
            self.message,
            SpaceMessage::Renew {} | SpaceMessage::Fence {} | SpaceMessage::Sleep {}
        ) {
            return if self.package.is_none() && self.digest.is_none() && self.ticket.is_none() {
                Ok(None)
            } else {
                Err(Error::Invalid)
            };
        }
        let unticketed = matches!(
            self.message,
            SpaceMessage::Ready {} | SpaceMessage::Credit { .. }
        );
        if unticketed != self.ticket.is_none() || self.ticket.is_some_and(|id| id.is_nil()) {
            return Err(Error::Invalid);
        }
        match (&self.package, &self.digest) {
            (Some(package), Some(_)) => Ok(Some(package)),
            _ => Err(Error::Invalid),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Offer {
    pub v: u8,
    pub space: SpaceId,
    pub assignment: Uuid,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Welcome {
    pub v: u8,
    pub space: SpaceId,
    pub assignment: Uuid,
    pub epoch: Uuid,
    pub lease_ms: u64,
    pub data_credit: usize,
    pub control_credit: usize,
    pub packages: Vec<PackagePin>,
}

pub(super) async fn read_line(
    reader: &mut (impl AsyncBufRead + Unpin),
    bytes: &mut Vec<u8>,
    max: usize,
) -> Result<(), Error> {
    bytes.clear();
    loop {
        let available = reader.fill_buf().await.map_err(|_| Error::Invocation)?;
        if available.is_empty() {
            return Err(Error::Invocation);
        }
        let end = available.iter().position(|&byte| byte == b'\n');
        let take = end.map_or(available.len(), |at| at + 1);
        if bytes.len() + take > max {
            return Err(Error::TooLarge);
        }
        bytes.extend_from_slice(&available[..take]);
        reader.consume(take);
        if end.is_some() {
            return Ok(());
        }
    }
}

pub(super) fn parse<T: serde::de::DeserializeOwned>(line: &[u8]) -> Result<T, Error> {
    let object = line.strip_suffix(b"\n").ok_or(Error::Invalid)?;
    if object.contains(&b'\r') {
        return Err(Error::Invalid);
    }
    crate::strict_json::parse(object)
}
