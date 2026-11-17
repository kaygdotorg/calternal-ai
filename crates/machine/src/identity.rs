//! The machine's identity: an Ed25519 key pair made on the machine, and the
//! credentials file that records it.
//!
//! The file is `key: value` lines (human-readable, like other Calternal files),
//! written atomically with mode 0600 through `calternal-fsio`:
//!
//! ```text
//! server: https://cal.example.org
//! machine_id: 0193…
//! owner: account 0193…   (or: owner: pool)
//! private_key: <base64 PKCS#8>
//! ```
//!
//! The server only ever sees the public key. Losing this file means enrolling again.

use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair};
use uuid::Uuid;

use crate::protocol::{Owner, session_message};
use crate::{Error, Result};

/// File name of the credentials inside the machine directory.
pub const CREDENTIALS_FILE: &str = "credentials";

/// A machine key pair. Signing costs about 20 µs.
pub struct MachineKey {
    pkcs8: Vec<u8>,
    pair: Ed25519KeyPair,
}

impl MachineKey {
    /// Make a new key from the operating system's random source.
    pub fn generate() -> Result<Self> {
        let doc = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
            .map_err(|_| Error::Protocol("cannot generate a machine key".into()))?;
        Self::from_pkcs8(doc.as_ref().to_vec())
    }

    fn from_pkcs8(pkcs8: Vec<u8>) -> Result<Self> {
        let pair = Ed25519KeyPair::from_pkcs8(&pkcs8)
            .map_err(|_| Error::Protocol("unreadable machine key".into()))?;
        Ok(Self { pkcs8, pair })
    }

    /// Raw public key, base64, as sent at enrolment.
    pub fn public_key_b64(&self) -> String {
        B64.encode(self.pair.public_key().as_ref())
    }

    /// Sign a session challenge for `machine_id`; returns base64.
    pub fn sign_session(&self, machine_id: Uuid, challenge: &str) -> String {
        B64.encode(
            self.pair
                .sign(&session_message(machine_id, challenge))
                .as_ref(),
        )
    }
}

/// Everything the machine role needs to connect after enrolment.
pub struct Credentials {
    /// Base URL of the server, without a trailing slash.
    pub server: String,
    /// The machine identity.
    pub machine_id: Uuid,
    /// Whose machine it is.
    pub owner: Owner,
    /// The private key.
    pub key: MachineKey,
}

impl Credentials {
    /// Path of the credentials file in `dir`.
    pub fn path(dir: &Path) -> PathBuf {
        dir.join(CREDENTIALS_FILE)
    }

    /// Write atomically with mode 0600; the directory is created 0700.
    pub fn save(&self, dir: &Path) -> Result<()> {
        create_private_dir(dir)?;
        let owner = match self.owner {
            Owner::Account { id } => format!("account {id}"),
            Owner::Pool => "pool".to_owned(),
        };
        let text = format!(
            "server: {}\nmachine_id: {}\nowner: {}\nprivate_key: {}\n",
            self.server,
            self.machine_id,
            owner,
            B64.encode(&self.key.pkcs8)
        );
        calternal_fsio::write_atomic_mode(&Self::path(dir), text.as_bytes(), 0o600)?;
        Ok(())
    }

    /// Read the credentials file in `dir`. Refuses a file readable by others.
    pub fn load(dir: &Path) -> Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        let path = Self::path(dir);
        let meta = std::fs::symlink_metadata(&path)?;
        if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
            return Err(Error::Protocol(
                "credentials file must be a regular file with mode 0600".into(),
            ));
        }
        let text = std::fs::read_to_string(&path)?;
        let field = |name: &str| -> Result<&str> {
            text.lines()
                .find_map(|l| l.strip_prefix(name)?.strip_prefix(':').map(str::trim))
                .ok_or_else(|| Error::Protocol(format!("credentials file lacks `{name}`")))
        };
        let bad = |what: &str| Error::Protocol(format!("credentials file has a bad `{what}`"));
        let owner = match field("owner")? {
            "pool" => Owner::Pool,
            other => Owner::Account {
                id: other
                    .strip_prefix("account ")
                    .and_then(|s| Uuid::parse_str(s.trim()).ok())
                    .ok_or_else(|| bad("owner"))?,
            },
        };
        Ok(Self {
            server: field("server")?.to_owned(),
            machine_id: Uuid::parse_str(field("machine_id")?).map_err(|_| bad("machine_id"))?,
            owner,
            key: MachineKey::from_pkcs8(
                B64.decode(field("private_key")?)
                    .map_err(|_| bad("private_key"))?,
            )?,
        })
    }
}

/// Create `dir` (and parents) and set it to mode 0700.
///
/// Stand-in for `calternal_auth::private_fs::directory`, which this crate does
/// not depend on (that would pull the whole auth crate onto machines); replace
/// with `calternal_fsio`'s copy once `private_fs` moves there.
pub fn create_private_dir(dir: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_round_trip_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let c = Credentials {
            server: "http://127.0.0.1:1".into(),
            machine_id: Uuid::new_v4(),
            owner: Owner::Account { id: Uuid::new_v4() },
            key: MachineKey::generate().unwrap(),
        };
        c.save(dir.path()).unwrap();
        let mode = std::fs::metadata(Credentials::path(dir.path()))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let back = Credentials::load(dir.path()).unwrap();
        assert_eq!(back.machine_id, c.machine_id);
        assert_eq!(back.owner, c.owner);
        assert_eq!(back.key.public_key_b64(), c.key.public_key_b64());

        std::fs::set_permissions(
            Credentials::path(dir.path()),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(Credentials::load(dir.path()).is_err());
    }
}
