//! Secret storage: values encrypted at rest, released only through audited use.
//!
//! A secret belongs to exactly one [`Owner`]: an account (provider logins, personal
//! plugin secrets) or a space. Plugins and agents never receive a value; they get
//! an opaque [`HandleToken`] that names (owner, name, version). The value leaves the
//! store only through [`SecretStore::use_value`], which names the [`Consumer`] (one
//! sandbox environment variable for one thread, or one outbound request header) and
//! writes an audit row first. Values never appear in errors, logs, audit rows,
//! metadata or handles; [`SecretValue`] has no `Display`, `Serialize` or revealing
//! `Debug`, and wipes its buffer on drop.
//!
//! Authorization (who may put, list, use or revoke for an owner) is the host's job.
//! The store enforces only that a value decrypts for the owner, name and version it
//! was written for. [`LocalStore`] is the on-disk backend; the trait is shaped so an
//! OpenBao backend can replace it (see `README.md`).
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

// The only import of private_fs: a one-line change when it moves to calternal-fsio.
pub(crate) use calternal_auth::private_fs;

mod key;
mod local;
mod types;

pub use local::LocalStore;
pub use types::{
    Action, Actor, AuditEntry, Consumer, Handle, HandleToken, Metadata, Owner, PendingRequest,
    RequestId, SecretName, SecretValue, Via,
};

/// Stable refusal categories. Messages are fixed text and never carry a value,
/// a ciphertext or a handle token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// No secret, request or handle matches for this owner. Also returned for a
    /// stale handle, a handle from another owner and a value that fails to
    /// authenticate, so a caller cannot tell these apart.
    #[error("secret not available")]
    NotFound,
    /// A name, label, reason, consumer or value is malformed or too large.
    #[error("invalid secret input")]
    InvalidInput,
    /// `put` found a secret of that name already; use `rotate`.
    #[error("secret already exists")]
    Conflict,
    /// The owner already holds [`LIMITS`]' maximum of secrets or pending requests.
    #[error("too many secrets or pending requests for this owner")]
    Limit,
    /// `system/secret.key` is absent while encrypted secrets exist. Restore it from
    /// backup; a new key would make every stored value unreadable.
    #[error("secret key is missing; restore appdata/system/secret.key from backup")]
    KeyMissing,
    /// `system/secret.key` is not the key the stored secrets were written with,
    /// or it is not a regular 32-byte file with mode 0600.
    #[error("secret key is unusable; restore the matching appdata/system/secret.key")]
    KeyUnusable,
    /// Storage, entropy or cipher failure. Details are deliberately dropped.
    #[error("secret storage failed")]
    Internal,
}

/// Storage bounds per owner (review gate item 7). Values are bounded by
/// [`SecretValue::MAX`], labels and reasons by their character limits.
pub struct Limits {
    /// Stored secrets per owner.
    pub secrets: u32,
    /// Pending requests per owner.
    pub pending: u32,
    /// Audit rows kept per owner; older rows are pruned on each audited write.
    pub audit: u32,
}
/// The bounds the local backend enforces.
pub const LIMITS: Limits = Limits {
    secrets: 1000,
    pending: 64,
    audit: 10_000,
};

/// Result type of every secret operation.
pub type Result<T> = std::result::Result<T, Error>;

/// Backend-neutral secret store. Implementations are synchronous and may block on
/// disk or network; async hosts call them from a blocking task.
///
/// Every mutating call and every [`use_value`](Self::use_value) records an
/// [`AuditEntry`] in the same atomic step as its effect.
pub trait SecretStore: Send + Sync {
    /// Store a new secret at version 1. Fails with [`Error::Conflict`] if the name
    /// exists for this owner.
    fn put(
        &self,
        owner: &Owner,
        name: &SecretName,
        label: &str,
        value: SecretValue,
        actor: &Actor,
    ) -> Result<Handle>;

    /// Replace an existing secret's value with a new version. The old ciphertext is
    /// destroyed in the same transaction, so handles to older versions stop working.
    fn rotate(
        &self,
        owner: &Owner,
        name: &SecretName,
        value: SecretValue,
        actor: &Actor,
    ) -> Result<Handle>;

    /// Current handle for a stored secret, without its value.
    fn handle(&self, owner: &Owner, name: &SecretName) -> Result<Handle>;

    /// Opaque wire form of a handle, safe to give to a plugin or agent.
    fn token(&self, handle: &Handle) -> HandleToken;

    /// Resolve a wire token presented in `owner`'s context back to its handle.
    /// A token minted for another owner, or for a destroyed version, is not found.
    fn resolve(&self, owner: &Owner, token: &HandleToken) -> Result<Handle>;

    /// Release a value to exactly one consumer after recording who, what and when.
    /// The audit row is durable before the value is returned; if it cannot be
    /// written, nothing is released.
    fn use_value(&self, handle: &Handle, actor: &Actor, consumer: &Consumer)
    -> Result<SecretValue>;

    /// Metadata of every stored secret of this owner; never values.
    fn list(&self, owner: &Owner) -> Result<Vec<Metadata>>;

    /// Destroy a secret and every version of it.
    fn delete(&self, owner: &Owner, name: &SecretName, actor: &Actor) -> Result<()>;

    /// Record that `actor` asks for a secret. The value is supplied later, through
    /// [`provide`](Self::provide), by a trusted surface.
    fn request(
        &self,
        owner: &Owner,
        name: &SecretName,
        label: &str,
        reason: &str,
        actor: &Actor,
    ) -> Result<RequestId>;

    /// Pending requests of this owner, oldest first.
    fn pending(&self, owner: &Owner) -> Result<Vec<PendingRequest>>;

    /// Answer a pending request: store (or rotate) the named secret and remove the
    /// request in one transaction. The host must have verified a fresh interactive
    /// session for `actor` first.
    fn provide(
        &self,
        owner: &Owner,
        request: &RequestId,
        value: SecretValue,
        actor: &Actor,
    ) -> Result<Handle>;

    /// Withdraw a pending request without supplying a value.
    fn cancel(&self, owner: &Owner, request: &RequestId, actor: &Actor) -> Result<()>;

    /// Most recent audit entries of this owner, newest first, at most `limit`.
    fn audit(&self, owner: &Owner, limit: u32) -> Result<Vec<AuditEntry>>;
}

#[cfg(test)]
mod tests;
