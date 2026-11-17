//! Per-owner keys, ported from the existing app's invite key
//! (`crates/auth/src/invite_key.rs` on `origin/dev`): a 32-byte file in
//! each owner's appdata folder with mode 0600, published atomically by exclusive rename, refused
//! when missing while encrypted values exist. Encryption, handle MACs and the key
//! fingerprint each use their own BLAKE3-derived key. No server-wide key exists.

use crate::{Error, Handle, HandleToken, Owner, Result};
use openssl::symm::{Cipher, decrypt_aead, encrypt_aead};
use rusqlite::{Connection, OptionalExtension, params};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use zeroize::Zeroizing;

const NONCE: usize = 12;
const TAG: usize = 16;

/// Derived keys; the raw file bytes are wiped as soon as these exist.
pub(crate) struct MasterKey {
    encrypt: Zeroizing<[u8; 32]>,
    handle: Zeroizing<[u8; 32]>,
}

impl MasterKey {
    /// Load one owner's key, validating its local fingerprint. Creation is allowed
    /// only inside the owner's IMMEDIATE transaction before any encrypted row.
    /// Publication precedes the recovery fingerprint; the caller commits local
    /// metadata only after that recovery barrier succeeds. A crash in between
    /// leaves an unused key that can be adopted, never encrypted orphan rows.
    pub(crate) fn open(
        dir: &Path,
        db: &Connection,
        create_key: bool,
        owner: &Owner,
        lifecycle: Option<&dyn crate::KeyLifecycle>,
    ) -> Result<Self> {
        let path = dir.join("secret.key");
        let state: Option<(Vec<u8>, bool)> = db
            .query_row(
                "SELECT fingerprint,erasing FROM key_state WHERE singleton=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|_| Error::Internal)?;
        if state.as_ref().is_some_and(|(_, erasing)| *erasing) {
            return Err(Error::NotFound);
        }
        let recorded = state.map(|(f, _)| f);
        let used: bool = if recorded.is_none() {
            db.query_row("SELECT EXISTS(SELECT 1 FROM secrets)", [], |r| r.get(0))
                .map_err(|_| Error::Internal)?
        } else {
            false
        };
        if !path.try_exists().map_err(|_| Error::Internal)? {
            if used || recorded.is_some() {
                return Err(Error::KeyMissing);
            }
            if !create_key {
                return Err(Error::KeyMissing);
            }
            create(dir, &path).map_err(|_| Error::Internal)?;
            crate::local::crash_point("key-published");
        }
        let meta = std::fs::symlink_metadata(&path).map_err(|_| Error::Internal)?;
        if !meta.is_file() || meta.len() != 32 || meta.permissions().mode() & 0o777 != 0o600 {
            return Err(Error::KeyUnusable);
        }
        let raw = Zeroizing::new(std::fs::read(&path).map_err(|_| Error::Internal)?);
        let raw: &[u8; 32] = raw.as_slice().try_into().map_err(|_| Error::KeyUnusable)?;
        let id = blake3::derive_key("calternal secret key id v1", raw);
        match recorded {
            Some(v) if v == id => {}
            Some(_) => return Err(Error::KeyUnusable),
            None if create_key => {
                if let Some(log) = lifecycle {
                    log.record(owner, &id, false)?;
                }
                crate::local::crash_point("key-recorded");
                db.execute("INSERT INTO key_state VALUES(1,?1,0)", params![&id[..]])
                    .map_err(|_| Error::Internal)?;
            }
            None => return Err(Error::KeyUnusable),
        }
        Ok(Self {
            encrypt: Zeroizing::new(blake3::derive_key("calternal secret encryption v1", raw)),
            handle: Zeroizing::new(blake3::derive_key("calternal secret handle v1", raw)),
        })
    }

    /// Opaque token for a handle. Cost: one BLAKE3 keyed hash over < 200 bytes.
    pub(crate) fn token(&self, handle: &Handle) -> HandleToken {
        let mut h = blake3::Hasher::new_keyed(&self.handle);
        h.update(&binding(
            &handle.owner,
            handle.name.as_str(),
            handle.version,
        ));
        let hex = h.finalize().to_hex();
        HandleToken(format!("sh-{}", &hex[..40]))
    }

    /// AES-256-GCM with a random nonce; owner, name and version are the associated
    /// data, so a row copied to another owner, name or version fails to decrypt.
    /// Output: nonce (12) | tag (16) | ciphertext. Cost: one AEAD pass.
    pub(crate) fn seal(
        &self,
        owner: &Owner,
        name: &str,
        version: u64,
        plain: &[u8],
    ) -> Result<Vec<u8>> {
        let mut nonce = [0u8; NONCE];
        getrandom::fill(&mut nonce).map_err(|_| Error::Internal)?;
        let mut tag = [0u8; TAG];
        let sealed = encrypt_aead(
            Cipher::aes_256_gcm(),
            &self.encrypt[..],
            Some(&nonce),
            &binding(owner, name, version),
            plain,
            &mut tag,
        )
        .map_err(|_| Error::Internal)?;
        Ok([&nonce[..], &tag, &sealed].concat())
    }

    /// Reverse of [`seal`](Self::seal). Any tampering or mismatch is [`Error::NotFound`],
    /// the same answer as a missing secret.
    pub(crate) fn open_value(
        &self,
        owner: &Owner,
        name: &str,
        version: u64,
        bytes: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>> {
        if bytes.len() < NONCE + TAG {
            return Err(Error::NotFound);
        }
        decrypt_aead(
            Cipher::aes_256_gcm(),
            &self.encrypt[..],
            Some(&bytes[..NONCE]),
            &binding(owner, name, version),
            &bytes[NONCE + TAG..],
            &bytes[NONCE..NONCE + TAG],
        )
        .map(Zeroizing::new)
        .map_err(|_| Error::NotFound)
    }
}

/// Unambiguous associated data: fixed prefix, NUL-separated fields (none of which
/// can contain NUL), version as 8 little-endian bytes.
fn binding(owner: &Owner, name: &str, version: u64) -> Vec<u8> {
    let mut b = Vec::with_capacity(96 + name.len());
    b.extend_from_slice(b"calternal secret v1\0");
    b.extend_from_slice(owner.kind().as_bytes());
    b.push(0);
    b.extend_from_slice(owner.uuid().as_hyphenated().to_string().as_bytes());
    b.push(0);
    b.extend_from_slice(name.as_bytes());
    b.push(0);
    b.extend_from_slice(&version.to_le_bytes());
    b
}

/// Publish a fresh owner key with the fsio private exclusive-rename helper.
/// No extra hard link to published key material can survive a crash.
fn create(_dir: &Path, path: &Path) -> std::io::Result<()> {
    let mut bytes = Zeroizing::new([0u8; 32]);
    getrandom::fill(&mut bytes[..]).map_err(std::io::Error::other)?;
    match calternal_fsio::private::publish(path, &bytes[..]) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        result => result,
    }
}
