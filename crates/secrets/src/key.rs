//! The master key, ported from the existing app's invite key
//! (`crates/auth/src/invite_key.rs` on `origin/dev`): a 32-byte file in
//! `appdata/system/` with mode 0600, published atomically by hard link, refused
//! when missing while encrypted values exist. Encryption, handle MACs and the key
//! fingerprint each use their own BLAKE3-derived key.

use crate::{Error, Handle, HandleToken, Owner, Result};
use openssl::symm::{Cipher, decrypt_aead, encrypt_aead};
use rusqlite::{Connection, OptionalExtension, params};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use zeroize::Zeroizing;

const NONCE: usize = 12;
const TAG: usize = 16;

/// Derived keys; the raw file bytes are wiped as soon as these exist.
pub(crate) struct MasterKey {
    encrypt: Zeroizing<[u8; 32]>,
    handle: Zeroizing<[u8; 32]>,
}

/// Open `system/secrets.db`, the owner registry that lets a missing key be detected.
/// It holds no values: only a key fingerprint and which owners have secrets.
/// Opened at startup and when an owner writes its first secret, never kept open,
/// so no installation-wide connection or lock sits on the request path.
pub(crate) fn registry(appdata: &Path) -> Result<Connection> {
    let dir = appdata.join("system");
    calternal_fsio::private::directory(&dir).map_err(|_| Error::Internal)?;
    crate::local::open_db(
        &dir.join("secrets.db"),
        "CREATE TABLE IF NOT EXISTS meta(k TEXT PRIMARY KEY, v BLOB NOT NULL);
         CREATE TABLE IF NOT EXISTS owners(kind TEXT NOT NULL, id TEXT NOT NULL, PRIMARY KEY(kind,id));",
    )
}

impl MasterKey {
    /// Load or create `appdata/system/secret.key`.
    ///
    /// Refuses with [`Error::KeyMissing`] if the file is absent but any owner has
    /// ever stored a secret, and with [`Error::KeyUnusable`] if it is not a regular
    /// 0600 file of 32 bytes, or if secrets exist and its fingerprint differs from
    /// the recorded one.
    /// Cost: one small file read, one registry query; creation adds two fsyncs.
    pub(crate) fn open(appdata: &Path, registry: &Connection) -> Result<Self> {
        let dir = appdata.join("system");
        let path = dir.join("secret.key");
        let used: bool = registry
            .query_row("SELECT EXISTS(SELECT 1 FROM owners)", [], |r| r.get(0))
            .map_err(|_| Error::Internal)?;
        if !path.try_exists().map_err(|_| Error::Internal)? {
            if used {
                return Err(Error::KeyMissing);
            }
            create(&dir, &path).map_err(|_| Error::Internal)?;
        }
        let meta = std::fs::symlink_metadata(&path).map_err(|_| Error::Internal)?;
        if !meta.is_file() || meta.permissions().mode() & 0o777 != 0o600 {
            return Err(Error::KeyUnusable);
        }
        let raw = Zeroizing::new(std::fs::read(&path).map_err(|_| Error::Internal)?);
        let raw: &[u8; 32] = raw.as_slice().try_into().map_err(|_| Error::KeyUnusable)?;
        let id = blake3::derive_key("calternal secret key id v1", raw);
        let recorded: Option<Vec<u8>> = registry
            .query_row("SELECT v FROM meta WHERE k='key_id'", [], |r| r.get(0))
            .optional()
            .map_err(|_| Error::Internal)?;
        // The fingerprint only binds once some owner has stored a secret; before
        // that, any key reads everything (there is nothing) and is adopted.
        match recorded {
            Some(v) if v == id[..16] => {}
            Some(_) if used => return Err(Error::KeyUnusable),
            _ => {
                registry
                    .execute(
                        "INSERT INTO meta(k,v) VALUES('key_id',?1) ON CONFLICT(k) DO UPDATE SET v=excluded.v",
                        params![&id[..16]],
                    )
                    .map_err(|_| Error::Internal)?;
            }
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

/// Write a fresh key to a private temporary file, fsync it, then publish it by
/// hard link: atomic, and never replaces a key another process just published.
fn create(dir: &Path, path: &Path) -> std::io::Result<()> {
    let mut bytes = Zeroizing::new([0u8; 32]);
    getrandom::fill(&mut bytes[..]).map_err(std::io::Error::other)?;
    let tmp = dir.join(format!(".secret-key-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(&bytes[..])?;
        f.sync_all()?;
        match std::fs::hard_link(&tmp, path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        calternal_fsio::sync_dir(dir)
    })();
    let _ = std::fs::remove_file(&tmp);
    result
}
