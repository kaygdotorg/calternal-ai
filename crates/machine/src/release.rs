//! Machine release identity, shared by the installer endpoint and self-updater.
use base64::Engine as _;

/// Root of the server's public, read-only machine release endpoints.
pub const PATH: &str = "/api/v1/machine/release";
/// Largest machine executable accepted for installation.
pub const MAX_BINARY: u64 = 256 * 1024 * 1024;

/// Release identifier embedded at build time. Missing or invalid IDs disable distribution.
pub fn build_id() -> Option<&'static str> {
    option_env!("CALTERNAL_MACHINE_RELEASE_ID").filter(|id| valid_id(id))
}

/// The owner's SSH Ed25519 public key embedded at build time.
/// Accept only the OpenSSH type and data fields, so the same pin is safe in shell and SSH.
pub fn public_key() -> Option<&'static str> {
    option_env!("CALTERNAL_MACHINE_RELEASE_PUBLIC_KEY").filter(|key| {
        let Some(data) = key.strip_prefix("ssh-ed25519 ") else {
            return false;
        };
        let decoded = base64::engine::general_purpose::STANDARD.decode(data).ok();
        decoded.is_some_and(|bytes| {
            bytes.len() == 51 && bytes.starts_with(b"\0\0\0\x0bssh-ed25519\0\0\0\x20")
        })
    })
}

/// Validate a compact version used as an individual URL and on-disk path component.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
