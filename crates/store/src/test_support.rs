//! Disposable fixtures for the inherited store tests.
use crate::{AccountId, SpaceId};
pub(super) struct TempDir(tempfile::TempDir);
impl TempDir {
    pub(super) fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }
    pub(super) fn path(&self) -> &std::path::Path {
        self.0.path()
    }
}
pub(super) fn test_space() -> SpaceId {
    "00000000-0000-7000-8000-000000000001".parse().unwrap()
}
pub(super) fn test_account() -> AccountId {
    "00000000-0000-7000-8000-000000000002".parse().unwrap()
}
