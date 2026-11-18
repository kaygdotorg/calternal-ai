//! Copies and validates the files named by a pending deployment manifest.
use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Component, Path},
};

use calternal_fsio::beneath::Folder;
use ring::digest::{Context, SHA256};

use super::intent::CopyProof;

pub(super) fn relative(path: &str) -> io::Result<&Path> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(io::Error::other("invalid upgrade snapshot path"));
    }
    Ok(path)
}

/// The caller owns the selected roots and the private snapshot directory.
/// Capture raw bytes, including any live database WAL named by the inventory.
pub(super) fn save(root: &Path, path: &str, destination: &Path) -> io::Result<Option<CopyProof>> {
    relative(path)?;
    let mut source = match Folder::open_existing(root)?.open_file(path) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // The descriptor-relative reader also refuses nonregular entries.
            // Only a genuinely absent pre-upgrade path is a deletion on restore.
            match std::fs::symlink_metadata(root.join(path)) {
                Err(missing) if missing.kind() == io::ErrorKind::NotFound => return Ok(None),
                _ => return Err(error),
            }
        }
        Err(error) => return Err(error),
    };
    let mode = source.metadata()?.permissions().mode() & 0o777;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)?;
    let (bytes, sha256) = hash_copy(&mut source, &mut file)?;
    file.sync_all()?;
    Ok(Some(CopyProof {
        mode,
        bytes,
        sha256,
    }))
}

/// Check a snapshot before the first restore write. The backup stays immutable
/// across retries, so an interrupted restoration can repeat from the same bytes.
pub(super) fn validate(path: &Path, proof: &CopyProof) -> io::Result<()> {
    let mut file = File::open(path)?;
    let (bytes, sha256) = hash_copy(&mut file, &mut io::sink())?;
    if proof.bytes != bytes || proof.sha256 != sha256 {
        return Err(io::Error::other("upgrade snapshot failed its checksum"));
    }
    Ok(())
}

fn hash_copy(input: &mut File, output: &mut impl io::Write) -> io::Result<(u64, String)> {
    let mut block = [0; 64 * 1024];
    let mut digest = Context::new(&SHA256);
    let mut bytes = 0;
    loop {
        let got = input.read(&mut block)?;
        if got == 0 {
            break;
        }
        output.write_all(&block[..got])?;
        digest.update(&block[..got]);
        bytes += got as u64;
    }
    Ok((bytes, hex::encode(digest.finish().as_ref())))
}

/// Restore a prevalidated file from its immutable copy. The destination was
/// selected by the coordinator under the same root lock as preparation.
pub(super) fn restore(
    root: &Path,
    path: &str,
    source: &Path,
    proof: Option<&CopyProof>,
) -> io::Result<()> {
    let relative = relative(path)?;
    let root = Folder::open_existing(root)?;
    let parent = relative.parent().and_then(Path::to_str).unwrap_or_default();
    let folder = if parent.is_empty() {
        root
    } else {
        match root.open_sub(parent, proof.is_some()) {
            Err(error) if error.kind() == io::ErrorKind::NotFound && proof.is_none() => {
                return Ok(());
            }
            result => result?,
        }
    };
    let name = relative
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::other("invalid upgrade file name"))?;
    match proof {
        Some(proof) => {
            let mut previous = File::open(source)?;
            let mut staged = folder.create_temp(&format!(
                "{}upgrade-{}",
                calternal_fsio::TEMP_PREFIX,
                uuid::Uuid::now_v7(),
            ))?;
            staged
                .file()
                .set_permissions(std::fs::Permissions::from_mode(proof.mode))?;
            std::io::copy(&mut previous, staged.file())?;
            staged.replace(name)
        }
        None => match folder.remove(name) {
            Ok(()) => folder.sync(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
    }
}
