//! Read-only credential mounts for an operator-selected run identity.

use std::os::unix::fs::MetadataExt;
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
    ) -> SandboxResult<&Path> {
        if self.account.as_uuid() != spec.account_id
            || Some(self.space) != spec.space_id
            || self.program != program
            || spec.provider_login
        {
            return Err(SandboxError(
                "This account, space and program cannot use operator sign-in".into(),
            ));
        }
        let metadata = tokio::fs::symlink_metadata(&self.file)
            .await
            .map_err(|_| SandboxError("Operator credential file is unavailable".into()))?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.mode() & 0o077 != 0
            || metadata.len() > 1024 * 1024
        {
            return Err(SandboxError(
                "Operator credentials must be in a private regular file of at most 1 MiB".into(),
            ));
        }
        Ok(&self.file)
    }
}
