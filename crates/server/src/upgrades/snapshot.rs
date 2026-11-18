//! Copies and validates the regular files named by an additive manifest.

use std::{
    ffi::OsStr,
    fs::{File, Permissions},
    io::{self, Read},
    os::unix::fs::PermissionsExt,
    path::{Component, Path},
};

use ring::digest::{Context, SHA256};
use rustix::fs::AtFlags;

use super::{intent::CopyProof, tree::Tree};

pub(super) fn relative(path: &str) -> io::Result<&Path> {
    let relative = Path::new(path);
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part.contains('\0'))
    {
        return Err(io::Error::other("invalid upgrade snapshot path"));
    }
    Ok(relative)
}

fn name(path: &Path) -> &OsStr {
    path.file_name().expect("validated snapshot path")
}

fn parent(root: &Path, path: &Path, create: bool) -> io::Result<Tree> {
    let mut folder = Tree::open(root)?;
    for part in path
        .parent()
        .expect("relative path has a parent")
        .components()
    {
        let name = part.as_os_str();
        folder = match folder.directory(name) {
            Err(error) if create && error.kind() == io::ErrorKind::NotFound => {
                folder.mkdir(name)?
            }
            result => result?,
        };
    }
    Ok(folder)
}

fn source(root: &Path, path: &str) -> io::Result<Option<File>> {
    let relative = relative(path)?;
    match parent(root, relative, false).and_then(|parent| parent.file(name(relative))) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io::Error::other(format!("cannot snapshot {path}: {error}"))),
    }
}

pub(super) fn check_source(root: &Path, path: &str) -> io::Result<bool> {
    Ok(source(root, path)?.is_some())
}

fn copies(root: &Path) -> io::Result<Tree> {
    Tree::open(root)?.directory(OsStr::new(super::COPY_DIR))
}

/// Capture raw bytes, including any live database WAL named by the inventory.
pub(super) fn save(root: &Path, path: &str, index: usize) -> io::Result<Option<CopyProof>> {
    let Some(mut source) = source(root, path)? else {
        return Ok(None);
    };
    let mode = source.metadata()?.permissions().mode() & 0o777;
    let mut file = copies(root)?.create(OsStr::new(&index.to_string()))?;
    let (bytes, sha256) = hash_copy(&mut source, &mut file)?;
    file.sync_all()?;
    Ok(Some(CopyProof {
        mode,
        bytes,
        sha256,
    }))
}

/// Refuse a missing or corrupt copy before the first data write of recovery.
pub(super) fn validate(root: &Path, index: usize, proof: &CopyProof) -> io::Result<()> {
    let mut file = copies(root)?.file(OsStr::new(&index.to_string()))?;
    check(&mut file, proof)
}

pub(super) fn check(file: &mut File, proof: &CopyProof) -> io::Result<()> {
    let (bytes, sha256) = hash_copy(file, &mut io::sink())?;
    if proof.bytes != bytes || proof.sha256 != sha256 {
        return Err(io::Error::other("upgrade snapshot failed its checksum"));
    }
    Ok(())
}

pub(super) fn hash_copy(
    input: &mut File,
    output: &mut impl io::Write,
) -> io::Result<(u64, String)> {
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

/// Restore one prevalidated entry while the root locks exclude data writers.
pub(super) fn restore(
    root: &Path,
    path: &str,
    index: usize,
    proof: Option<&CopyProof>,
) -> io::Result<()> {
    let relative = relative(path)?;
    let folder = match parent(root, relative, proof.is_some()) {
        Err(error) if error.kind() == io::ErrorKind::NotFound && proof.is_none() => return Ok(()),
        result => result?,
    };
    let dest = name(relative);
    let remove = |tree: &Tree, name| match tree.remove(name, AtFlags::empty()) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    };
    match proof {
        Some(proof) => {
            let copies = copies(root)?;
            let stage = OsStr::new("restore-staging");
            remove(&copies, stage)?;
            let mut previous = copies.file(OsStr::new(&index.to_string()))?;
            let mut output = copies.create(stage)?;
            output.set_permissions(Permissions::from_mode(proof.mode))?;
            io::copy(&mut previous, &mut output)?;
            output.sync_all()?;
            copies.replace(stage, &folder, dest)
        }
        None => remove(&folder, dest),
    }
}
