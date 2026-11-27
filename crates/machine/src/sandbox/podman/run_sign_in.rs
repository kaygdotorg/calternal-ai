//! Read-only credential mounts for an operator-selected run identity.

use rustix::fs::OFlags;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

use calternal_types::{AccountId, SpaceId};

use crate::protocol::SandboxSpec;
use crate::sandbox::{SandboxError, SandboxResult};

pub(super) struct RunSignIn {
    account: AccountId,
    space: SpaceId,
    program: String,
    file: PathBuf,
}

impl RunSignIn {
    pub(super) fn new(
        account: AccountId,
        space: SpaceId,
        program: String,
        file: PathBuf,
    ) -> SandboxResult<Self> {
        let valid_id = |id: &str| {
            id.len() <= 80
                && id.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        };
        if !program
            .split_once('.')
            .is_some_and(|(plugin, program)| valid_id(plugin) && valid_id(program))
        {
            return Err(SandboxError("Invalid operator sign-in program ID".into()));
        }
        if !file.is_absolute()
            || file.components().any(|part| part == Component::ParentDir)
            || !file
                .to_str()
                .is_some_and(|value| !value.contains(':') && !value.chars().any(char::is_control))
            || !file
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')
                    })
                })
        {
            return Err(SandboxError("Invalid operator credential file path".into()));
        }
        Ok(Self {
            account,
            space,
            program,
            file,
        })
    }

    pub(super) async fn credential(
        &self,
        spec: &SandboxSpec,
        program: &str,
        run_directory: &Path,
    ) -> SandboxResult<PathBuf> {
        if self.account.as_uuid() != spec.account_id
            || Some(self.space) != spec.space_id
            || self.program != program
            || spec.provider_login
        {
            return Err(SandboxError(
                "This account, space and program cannot use operator sign-in".into(),
            ));
        }
        let source = self.file.clone();
        let directory = run_directory.join("credential");
        tokio::task::spawn_blocking(move || {
            let file = open_checked(&source)?;
            let name = source.file_name().expect("validated credential name");
            stage_checked(file, &directory.join(name))
                .map_err(|_| SandboxError("Cannot stage operator credentials".into()))?;
            Ok(directory.join(name))
        })
        .await
        .map_err(|_| SandboxError("Cannot stage operator credentials".into()))?
    }
}

const MAX_CREDENTIAL_BYTES: u64 = 1024 * 1024;

fn open_checked(path: &Path) -> SandboxResult<File> {
    let unavailable = |_| SandboxError("Operator credential file is unavailable".into());
    let file = OpenOptions::new()
        .read(true)
        .custom_flags((OFlags::NOFOLLOW | OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .map_err(unavailable)?;
    let metadata = file.metadata().map_err(unavailable)?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.mode() & 0o077 != 0
        || metadata.len() > MAX_CREDENTIAL_BYTES
    {
        return Err(SandboxError(
            "Operator credentials must be in a private regular file of at most 1 MiB".into(),
        ));
    }
    Ok(file)
}

fn stage_checked(source: File, destination: &Path) -> io::Result<()> {
    DirBuilder::new()
        .mode(0o700)
        .create(destination.parent().expect("private credential directory"))?;
    let mut staged = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)?;
    if io::copy(&mut source.take(MAX_CREDENTIAL_BYTES + 1), &mut staged)? > MAX_CREDENTIAL_BYTES {
        return Err(io::Error::other(
            "operator credential file grew beyond its limit",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_staging_uses_the_checked_inode_after_rotation() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("credential.json");
        let replacement = root.path().join("replacement");
        for (path, content) in [
            (&original, "synthetic original"),
            (&replacement, "synthetic replacement"),
        ] {
            std::io::Write::write_all(
                &mut OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(path)
                    .unwrap(),
                content.as_bytes(),
            )
            .unwrap();
        }
        let file = open_checked(&original).unwrap();
        std::fs::rename(&replacement, &original).unwrap();
        let staged = root.path().join("private/credential.json");
        stage_checked(file, &staged).unwrap();
        assert_eq!(
            std::fs::read_to_string(&staged).unwrap(),
            "synthetic original"
        );
        assert_eq!(staged.metadata().unwrap().mode() & 0o777, 0o600);
    }
}
