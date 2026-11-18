//! Format marker for per-account microVM data disks.
//!
//! The ext4 volume label carries the disk layout version, so a newer layout is
//! recognized without mounting. The guest stores the ordinary sandbox tree
//! under `/data/sandboxes`; provider files inside it remain opaque.

use std::{fs::File, io, os::unix::fs::FileExt, path::Path};

/// Label written by `mkfs.ext4 -L`. Bump the suffix when the in-disk layout changes.
pub(super) const LABEL: &str = "calternal-v1";
/// The primary superblock starts at byte 1024 and stores its 16-byte volume name at 0x78.
const NAME_OFFSET: u64 = 1024 + 0x78;

/// Refuse an existing disk whose label is not this release's layout, leaving it
/// untouched. Cost: one open and one 16-byte positioned read; blocking.
pub(super) fn check(path: &Path) -> io::Result<()> {
    let mut name = [0; 16];
    File::open(path)?.read_exact_at(&mut name, NAME_OFFSET)?;
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    if &name[..end] == LABEL.as_bytes() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        format!(
            "microVM account disk has unsupported format {:?}; expected {LABEL}",
            String::from_utf8_lossy(&name[..end])
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_current_label() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("disk.ext4");
        let mut bytes = vec![0; 4096];
        bytes[NAME_OFFSET as usize..][..LABEL.len()].copy_from_slice(LABEL.as_bytes());
        std::fs::write(&path, &bytes).unwrap();
        check(&path).unwrap();
        bytes[NAME_OFFSET as usize..][..LABEL.len()].copy_from_slice(b"calternal-v2");
        std::fs::write(&path, &bytes).unwrap();
        let error = check(&path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
