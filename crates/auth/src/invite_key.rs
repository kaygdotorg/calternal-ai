//! The keyed lookup and recoverable ciphertext from the source invite key.
use crate::{Error, Result};
use openssl::symm::{Cipher, decrypt_aead, encrypt_aead};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub(crate) struct Key {
    encryption: [u8; 32],
    lookup: [u8; 32],
}

impl Key {
    pub(crate) fn open(path: &Path, invites_exist: impl FnOnce() -> Result<bool>) -> Result<Self> {
        if !path.try_exists().map_err(Error::internal)? {
            if invites_exist()? {
                return Err(Error::internal(
                    "restore the missing system/invite.key from backup",
                ));
            }
            match calternal_fsio::private::publish(path, &crate::token::random32()?) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(Error::internal(e)),
            }
        }
        let meta = std::fs::symlink_metadata(path).map_err(Error::internal)?;
        if !meta.is_file() || meta.permissions().mode() & 0o777 != 0o600 || meta.len() != 32 {
            return Err(Error::internal(
                "system/invite.key must be a 32-byte, mode 0600 regular file",
            ));
        }
        let encryption: [u8; 32] = std::fs::read(path)
            .map_err(Error::internal)?
            .try_into()
            .map_err(|_| Error::internal("invalid invite key length"))?;
        Ok(Self {
            lookup: blake3::derive_key("calternal invite lookup v1", &encryption),
            encryption,
        })
    }

    pub(crate) fn lookup(&self, secret: &str) -> blake3::Hash {
        blake3::keyed_hash(&self.lookup, secret.as_bytes())
    }

    pub(crate) fn encrypt(&self, secret: &str, id: &str) -> Result<Vec<u8>> {
        let random = crate::token::random32()?;
        let nonce = &random[..12];
        let mut tag = [0; 16];
        let encrypted = encrypt_aead(
            Cipher::aes_256_gcm(),
            &self.encryption,
            Some(nonce),
            id.as_bytes(),
            secret.as_bytes(),
            &mut tag,
        )
        .map_err(|_| Error::internal("could not encrypt invite"))?;
        Ok([nonce, &tag, &encrypted].concat())
    }

    pub(crate) fn decrypt(&self, bytes: &[u8], id: &str) -> Result<String> {
        if bytes.len() < 28 {
            return Err(Error::internal("invalid encrypted invite"));
        }
        let plain = decrypt_aead(
            Cipher::aes_256_gcm(),
            &self.encryption,
            Some(&bytes[..12]),
            id.as_bytes(),
            &bytes[28..],
            &bytes[12..28],
        )
        .map_err(|_| Error::internal("could not decrypt invite"))?;
        String::from_utf8(plain).map_err(|_| Error::internal("invalid invite encoding"))
    }
}
