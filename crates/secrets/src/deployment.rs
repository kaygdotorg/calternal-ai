//! AEAD for host-only deployment credentials embedded in protected authority.

use crate::{Error, Result, SecretValue};
use openssl::symm::{Cipher, decrypt_aead, encrypt_aead};
use zeroize::Zeroizing;

/// Installation-scoped credential sealer for a trusted host service. The
/// caller retains the encrypted record and controls deployment authorization.
/// This type never exposes raw key bytes or plaintext through formatting.
pub struct DeploymentCipher(Zeroizing<[u8; 32]>);

impl DeploymentCipher {
    /// Derives an independent OIDC credential key from a private installation key.
    /// The caller owns persistence and backup of the installation key.
    pub fn oidc(installation_key: &[u8; 32]) -> Self {
        Self(Zeroizing::new(blake3::derive_key(
            "calternal deployment OIDC credential v1",
            installation_key,
        )))
    }

    /// Seals a credential for the supplied provider binding. Cost: one AEAD pass.
    pub fn seal(&self, binding: &[u8], secret: &SecretValue) -> Result<Vec<u8>> {
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).map_err(|_| Error::Internal)?;
        let mut tag = [0u8; 16];
        let body = encrypt_aead(
            Cipher::aes_256_gcm(),
            &self.0[..],
            Some(&nonce),
            binding,
            secret.expose(),
            &mut tag,
        )
        .map_err(|_| Error::Internal)?;
        Ok([&nonce[..], &tag, &body].concat())
    }

    /// Opens the same provider binding into a redacting, zeroized secret value.
    pub fn open(&self, binding: &[u8], sealed: &[u8]) -> Result<SecretValue> {
        if sealed.len() <= 28 {
            return Err(Error::NotFound);
        }
        let bytes = decrypt_aead(
            Cipher::aes_256_gcm(),
            &self.0[..],
            Some(&sealed[..12]),
            binding,
            &sealed[28..],
            &sealed[12..28],
        )
        .map_err(|_| Error::NotFound)?;
        SecretValue::new(bytes)
    }
}
