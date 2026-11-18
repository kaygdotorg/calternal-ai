//! Operator-only first-owner codes issued by the server that holds the root locks.
use calternal_auth::{Auth, FirstOwnerCode};
use calternal_types::Error;
use std::{
    io::Read,
    path::{Path, PathBuf},
};

/// Serializes the protected credential update and private file publication.
/// Only startup and the local setup command acquire this lock.
pub struct OwnerSetup {
    auth: Auth,
    path: PathBuf,
    url: String,
    publish: tokio::sync::Mutex<()>,
}

impl OwnerSetup {
    /// Bind first-owner setup to this server's authority and public origin. No I/O.
    pub fn new(auth: Auth, appdata: &Path, origin: &str) -> Self {
        Self {
            auth,
            path: appdata.join("setup.token"),
            url: format!("{}/", origin.trim_end_matches('/')),
            publish: tokio::sync::Mutex::new(()),
        }
    }

    /// Path of the private file published by this service. No I/O.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Address where the operator can use a code. The address carries no secret.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Return the current code at startup or replace it on explicit request.
    /// Issuance and file publication run in one order for concurrent operators.
    /// A valid startup code costs two indexed reads and no writes. Rotation
    /// costs one protected commit and an atomic mode-0600 file replacement.
    pub async fn issue(&self, rotate: bool) -> Result<Option<FirstOwnerCode>, Error> {
        let _publishing = self.publish.lock().await;
        let candidate = if rotate {
            None
        } else {
            let path = self.path.clone();
            io(move || read_candidate(&path)).await?
        };
        let grant = self
            .auth
            .first_owner_code(candidate.clone())
            .await
            .map_err(|_| Error::unavailable("could not issue a first-owner setup code"))?;
        if let Some(grant) = &grant
            && candidate.as_deref() != Some(grant.code.as_str())
        {
            let (path, code) = (self.path.clone(), grant.code.clone());
            io(move || calternal_fsio::write_atomic_mode(&path, code.as_bytes(), 0o600)).await?;
        }
        Ok(grant)
    }
}

fn read_candidate(path: &Path) -> std::io::Result<Option<String>> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => {
            let file = calternal_fsio::private::file(path)?;
            let mut bytes = Vec::new();
            file.take(256).read_to_end(&mut bytes)?;
            Ok(String::from_utf8(bytes).ok().filter(|s| s.len() < 256))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

async fn io<T: Send + 'static>(
    f: impl FnOnce() -> std::io::Result<T> + Send + 'static,
) -> Result<T, Error> {
    crate::blocking::spawn(f)
        .await
        .map_err(|_| Error::unavailable("the setup code file worker is unavailable"))?
        .map_err(|_| Error::unavailable("could not access the private setup code file"))
}
