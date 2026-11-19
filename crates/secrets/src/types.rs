//! Value types shared by every backend. None of them can carry or print a value
//! except [`SecretValue`], which only exposes its bytes through an explicit call.

use crate::{Error, Result};
use calternal_types::Owner;
use zeroize::Zeroizing;

/// Validated secret name: 1–96 bytes of `a-z 0-9 . _ - :`, starting with a letter
/// or digit. Plugin secrets are named `plugin:<plugin-id>:<name>` by the host so
/// one plugin cannot name another's.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SecretName(String);

impl SecretName {
    /// Validate a name. Cost: one pass over at most 96 bytes.
    pub fn new(name: &str) -> Result<Self> {
        let ok = name.len() <= 96
            && name
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-:".contains(&b));
        if ok {
            Ok(Self(name.to_owned()))
        } else {
            Err(Error::InvalidInput)
        }
    }
    /// The name of a plugin's declared secret: `plugin:<plugin-id>:<name>`.
    pub fn for_plugin(plugin: &str, name: &str) -> Result<Self> {
        if plugin.contains(':') || name.contains(':') {
            return Err(Error::InvalidInput);
        }
        Self::new(&format!("plugin:{plugin}:{name}"))
    }
    /// The validated text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A secret value. It has no `Display` or `Serialize`, its `Debug` prints
/// `SecretValue(..)`, and its buffer is zeroed on drop. Values are at most 64 KiB.
pub struct SecretValue(Zeroizing<Vec<u8>>);

impl SecretValue {
    /// Largest accepted value in bytes.
    pub const MAX: usize = 64 * 1024;
    /// Take ownership of value bytes; empty and oversized values are refused.
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        let bytes = Zeroizing::new(bytes);
        if bytes.is_empty() || bytes.len() > Self::MAX {
            return Err(Error::InvalidInput);
        }
        Ok(Self(bytes))
    }
    /// Wrap freshly decrypted bytes without copying them.
    pub(crate) fn from_plain(bytes: Zeroizing<Vec<u8>>) -> Self {
        Self(bytes)
    }
    /// The plaintext. Call only at the consumer that [`crate::SecretStore::use_value`]
    /// named, and do not copy it into logs, errors or stored records.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretValue(..)")
    }
}

/// Host-side identity of one stored version. Never sent to plugins as is; they
/// receive its [`HandleToken`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Handle {
    /// Owner the value is bound to.
    pub owner: Owner,
    /// Secret name within the owner.
    pub name: SecretName,
    /// Version, starting at 1 and increased by every rotation.
    pub version: u64,
}

/// Opaque wire handle: `sh-` followed by 40 lowercase hex digits (a keyed BLAKE3
/// MAC of owner, name and version). Reveals nothing and cannot be forged without
/// the owner’s key. Fits the plugin host's identifier rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HandleToken(pub(crate) String);

impl HandleToken {
    /// Parse a presented token; anything not in the exact format is refused.
    pub fn parse(token: &str) -> Result<Self> {
        let hex = token.strip_prefix("sh-").ok_or(Error::NotFound)?;
        if hex.len() == 40
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(token.to_owned()))
        } else {
            Err(Error::NotFound)
        }
    }
    /// The wire text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The one place a released value may go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Consumer {
    /// Host login to an account's exact configured IMAPS destination.
    MailLogin {
        /// Verified TLS name, as configured by the person.
        host: String,
        /// Configured IMAPS port.
        port: u16,
    },
    /// Host verification of one manifest-declared incoming webhook.
    WebhookSignature {
        /// Package declaring the space-local route.
        package: String,
        /// Declared route name, with no query or delivery identifiers.
        route: String,
    },
    /// One environment variable in the sandbox of one agent thread.
    SandboxEnv {
        /// Thread whose sandbox receives the variable.
        thread: uuid::Uuid,
        /// Variable name: `A-Z 0-9 _`, at most 64 bytes, not starting with a digit.
        variable: String,
    },
    /// One header on one outbound request to one origin.
    RequestHeader {
        /// `https://host[:port]` the request is sent to.
        origin: String,
        /// Header name: token characters only, at most 64 bytes.
        header: String,
    },
}

impl Consumer {
    /// Check the consumer's shape so audit rows stay small and unambiguous.
    pub(crate) fn validate(&self) -> Result<()> {
        let ok = match self {
            Self::MailLogin { host, port } => {
                *port != 0
                    && !host.is_empty()
                    && host.len() <= 253
                    && host
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
            }
            Self::WebhookSignature { package, route } => {
                SecretName::for_plugin(package, route).is_ok()
            }
            Self::SandboxEnv { variable, .. } => {
                !variable.is_empty()
                    && variable.len() <= 64
                    && !variable.as_bytes()[0].is_ascii_digit()
                    && variable
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            }
            Self::RequestHeader { origin, header } => {
                origin.len() <= 256
                    && origin.strip_prefix("https://").is_some_and(|h| {
                        !h.is_empty()
                            && h.bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b))
                    })
                    && !header.is_empty()
                    && header.len() <= 64
                    && header
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
            }
        };
        if ok { Ok(()) } else { Err(Error::InvalidInput) }
    }
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::MailLogin { host, port } => format!("IMAPS login to {host}:{port}"),
            Self::WebhookSignature { package, route } => {
                format!("verify webhook {package}/{route}")
            }
            Self::SandboxEnv { thread, variable } => format!("env {variable} in thread {thread}"),
            Self::RequestHeader { origin, header } => format!("header {header} to {origin}"),
        }
    }
}

/// Identifier of a pending request (UUIDv7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(pub uuid::Uuid);

/// What an audit entry records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Version 1 stored.
    Put,
    /// New version stored, old one destroyed.
    Rotate,
    /// Value released to a consumer.
    Use,
    /// Secret destroyed.
    Delete,
    /// Value asked for.
    Request,
    /// Request answered.
    Provide,
    /// Request withdrawn.
    Cancel,
}

impl Action {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Put => "put",
            Self::Rotate => "rotate",
            Self::Use => "use",
            Self::Delete => "delete",
            Self::Request => "request",
            Self::Provide => "provide",
            Self::Cancel => "cancel",
        }
    }
    pub(crate) fn parse(s: &str) -> Option<Self> {
        [
            Self::Put,
            Self::Rotate,
            Self::Use,
            Self::Delete,
            Self::Request,
            Self::Provide,
            Self::Cancel,
        ]
        .into_iter()
        .find(|a| a.as_str() == s)
    }
}

/// Listing row: everything about a secret except its value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Metadata {
    /// Secret name.
    pub name: String,
    /// Human label shown in the UI.
    pub label: String,
    /// Current version.
    pub version: u64,
    /// Creation time, Unix milliseconds.
    pub created_at: i64,
    /// Last put or rotation, Unix milliseconds.
    pub updated_at: i64,
}

/// A request waiting for a trusted surface to supply its value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PendingRequest {
    /// Request identifier.
    pub id: uuid::Uuid,
    /// Secret name the value will be stored under.
    pub name: String,
    /// Short label shown in the prompt.
    pub label: String,
    /// Why the requester needs it, shown in the prompt.
    pub reason: String,
    /// Who asked (account and route).
    pub requested_by: String,
    /// Request time, Unix milliseconds.
    pub created_at: i64,
}

/// One audit row. It never contains a value or a handle token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    /// Time, Unix milliseconds.
    pub at: i64,
    /// Who acted (account and route).
    pub actor: String,
    /// What happened.
    pub action: Action,
    /// Secret name.
    pub name: String,
    /// Version affected, if any.
    pub version: Option<u64>,
    /// For [`Action::Use`], where the value went.
    pub consumer: Option<String>,
}

/// Bounded printable text for labels (≤120 chars) and reasons (≤500 chars).
pub(crate) fn text(value: &str, max: usize) -> Result<&str> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(Error::InvalidInput);
    }
    Ok(value)
}
