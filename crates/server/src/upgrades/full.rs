//! Inventory and replay a complete restore point without resolving workspace links.

use rustix::fs::{AtFlags, FileType};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs::Permissions,
    io,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::Path,
};

use super::{COPY_DIR, Deployment, INTENT, ROLES, Root, intent::CopyProof, snapshot, tree::Tree};
use crate::fault::{FaultPoint, hit};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Entry {
    role: Root,
    // Hex preserves arbitrary Unix filename bytes in the versioned JSON.
    path: String,
    node: Node,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Node {
    Directory { mode: u32 },
    File { mode: u32, copy: Option<CopyProof> },
    Symlink { target: String },
    HardLink { anchor: usize },
    Absent,
}

struct Links {
    anchor: usize,
    expected: u64,
    found: u64,
}

type Inodes = BTreeMap<(u64, u64), Links>;

fn copy_dir() -> &'static OsStr {
    OsStr::new(COPY_DIR)
}

fn excluded(path: &Path) -> bool {
    path.starts_with(COPY_DIR)
        || [
            INTENT,
            "system/.calternal-tmp-upgrade-intent",
            ".lock",
            ".machine-role.lock",
        ]
        .iter()
        .any(|name| path == Path::new(name))
}

fn entry_error(role: Root, path: &Path, error: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("{role:?} root entry {}: {error}", path.display()))
}

fn path_bytes(path: &Path) -> &[u8] {
    path.as_os_str().as_bytes()
}

fn walk(
    dir: &Tree,
    prefix: &Path,
    role: Root,
    entries: &mut Vec<Entry>,
    inodes: &mut Inodes,
    live: bool,
) -> io::Result<()> {
    if path_bytes(prefix).len() >= 4096 {
        return Err(entry_error(
            role,
            prefix,
            "snapshot path must be shorter than 4096 bytes",
        ));
    }
    for name in dir.names()? {
        let path = prefix.join(&name);
        if excluded(&path) {
            continue;
        }
        let stat = match dir.inspect(&name) {
            Ok(stat) => stat,
            Err(error) if live && error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(entry_error(role, &path, error)),
        };
        let mode = stat.st_mode as u32 & 0o7777;
        let kind = FileType::from_raw_mode(stat.st_mode);
        if live && kind == FileType::RegularFile {
            match dir.file(&name) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(entry_error(role, &path, error)),
            }
        }
        let node = match kind {
            FileType::Directory => Node::Directory { mode },
            FileType::RegularFile if stat.st_nlink <= 1 => Node::File { mode, copy: None },
            FileType::RegularFile => {
                let links = inodes
                    .entry((stat.st_dev as u64, stat.st_ino as u64))
                    .or_insert(Links {
                        anchor: entries.len(),
                        expected: stat.st_nlink as u64,
                        found: 0,
                    });
                links.found += 1;
                if links.anchor == entries.len() {
                    Node::File { mode, copy: None }
                } else {
                    Node::HardLink {
                        anchor: links.anchor,
                    }
                }
            }
            FileType::Symlink if stat.st_nlink == 1 => Node::Symlink {
                target: match dir.target(&name) {
                    Ok(target) => hex::encode(target),
                    Err(error) if live && error.kind() == io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(entry_error(role, &path, error)),
                },
            },
            FileType::Socket => continue,
            _ => return Err(entry_error(role, &path, "unsupported snapshot entry type")),
        };
        if path_bytes(&path).len() >= 4096 {
            return Err(entry_error(
                role,
                &path,
                "snapshot path must be shorter than 4096 bytes",
            ));
        }
        entries.push(Entry {
            role,
            path: hex::encode(path_bytes(&path)),
            node,
        });
        if kind == FileType::Directory {
            match dir.directory(&name) {
                Ok(child) => walk(&child, &path, role, entries, inodes, live)?,
                Err(error) if live && error.kind() == io::ErrorKind::NotFound => {
                    entries.pop();
                }
                Err(error) => return Err(entry_error(role, &path, error)),
            }
        }
    }
    Ok(())
}

pub(super) fn plan(
    deploy: &Deployment,
    extras: BTreeSet<(Root, String)>,
    live: bool,
) -> io::Result<Vec<Entry>> {
    if !cfg!(target_os = "linux") {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "full restore points require Linux no-cross-mount lookup",
        ));
    }
    let mut entries = Vec::new();
    for role in ROLES {
        let Ok(path) = deploy.root(role) else {
            continue;
        };
        let mut inodes = BTreeMap::new();
        walk(
            &Tree::open(path)?,
            Path::new(""),
            role,
            &mut entries,
            &mut inodes,
            live,
        )?;
        for links in inodes.values() {
            if links.found != links.expected {
                let anchor = entries[links.anchor].decode()?;
                return Err(entry_error(
                    role,
                    Path::new(OsStr::from_bytes(&anchor)),
                    "hard-link names extend outside the captured root",
                ));
            }
        }
    }
    let present: BTreeSet<_> = entries
        .iter()
        .map(|entry| (entry.role, entry.path.clone()))
        .collect();
    for (role, path) in extras {
        let encoded = hex::encode(path.as_bytes());
        if !present.contains(&(role, encoded.clone())) {
            entries.push(Entry {
                role,
                path: encoded,
                node: Node::Absent,
            });
        }
    }
    validate_entries(deploy, &entries, false)?;
    // Bound even live inventory serialization before stopping services. The
    // reserved allowance covers checksums and the rest of the intent envelope.
    let listed = serde_json::to_vec(&entries)?.len();
    let files = entries
        .iter()
        .filter(|entry| matches!(entry.node, Node::File { .. }))
        .count();
    if listed
        .saturating_add(files.saturating_mul(128))
        .saturating_add(4096)
        > super::intent::LIMIT as usize
    {
        return Err(io::Error::other(
            "full restore point exceeds the upgrade intent size limit",
        ));
    }
    Ok(entries)
}

impl Entry {
    fn decode(&self) -> io::Result<Vec<u8>> {
        let raw = decode(&self.path)?;
        if raw.len() >= 4096
            || raw
                .split(|b| *b == b'/')
                .any(|part| part.is_empty() || part == b"." || part == b".." || part.contains(&0))
            || excluded(Path::new(OsStr::from_bytes(&raw)))
        {
            return Err(io::Error::other("invalid full snapshot path"));
        }
        Ok(raw)
    }
}

fn decode(value: &str) -> io::Result<Vec<u8>> {
    hex::decode(value).map_err(io::Error::other)
}

fn validate_entries(deploy: &Deployment, entries: &[Entry], copied: bool) -> io::Result<()> {
    let mut paths = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        deploy.root(entry.role)?;
        let raw = entry.decode()?;
        let path = Path::new(OsStr::from_bytes(&raw));
        let host = (entry.role, raw.clone());
        if paths.contains_key(&host) {
            return Err(entry_error(entry.role, path, "duplicate manifest entry"));
        }
        if !matches!(entry.node, Node::Absent) {
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                if paths.get(&(entry.role, path_bytes(parent).to_vec())) != Some(&true) {
                    return Err(entry_error(
                        entry.role,
                        path,
                        "missing manifest parent directory",
                    ));
                }
            }
        }
        match &entry.node {
            Node::Directory { mode } | Node::File { mode, .. } if *mode > 0o7777 => {
                return Err(entry_error(entry.role, path, "invalid snapshot mode"));
            }
            Node::File { copy, .. } if copied && copy.is_none() => {
                return Err(entry_error(entry.role, path, "missing payload proof"));
            }
            Node::Symlink { target } => {
                let target = decode(target)?;
                if target.is_empty() || target.contains(&0) {
                    return Err(entry_error(
                        entry.role,
                        path,
                        "invalid symlink target bytes",
                    ));
                }
            }
            Node::HardLink { anchor } => {
                if *anchor >= index
                    || entries[*anchor].role != entry.role
                    || !matches!(entries[*anchor].node, Node::File { .. })
                {
                    return Err(entry_error(entry.role, path, "invalid hard link anchor"));
                }
            }
            _ => {}
        }
        paths.insert(host, matches!(entry.node, Node::Directory { .. }));
    }
    Ok(())
}

fn parent(root: &Tree, path: &Path) -> io::Result<Option<Tree>> {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| root.directory(p.as_os_str()))
        .transpose()
}

fn name(path: &Path) -> &OsStr {
    path.file_name().expect("validated manifest path")
}

fn open_roots(deploy: &Deployment) -> io::Result<BTreeMap<Root, Tree>> {
    ROLES
        .into_iter()
        .filter_map(|role| deploy.root(role).ok().map(|path| (role, path)))
        .map(|(role, path)| Ok((role, Tree::open(path)?)))
        .collect()
}

pub(super) fn capture(deploy: &Deployment, entries: &mut [Entry]) -> io::Result<()> {
    let roots = open_roots(deploy)?;
    let mut dirs = BTreeMap::new();
    for (index, entry) in entries.iter_mut().enumerate() {
        let Node::File { mode, copy } = &mut entry.node else {
            continue;
        };
        let root = &roots[&entry.role];
        let destination = match dirs.entry(entry.role) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(root.mkdir(copy_dir())?)
            }
            std::collections::btree_map::Entry::Occupied(slot) => slot.into_mut(),
        };
        let raw = decode(&entry.path)?;
        let path = Path::new(OsStr::from_bytes(&raw));
        let folder = parent(root, path)?;
        let mut source = folder.as_ref().unwrap_or(root).file(name(path))?;
        let mut payload = destination.create(OsStr::new(&index.to_string()))?;
        let (bytes, sha256) = snapshot::hash_copy(&mut source, &mut payload)?;
        payload.sync_all()?;
        *copy = Some(CopyProof {
            mode: *mode,
            bytes,
            sha256,
        });
        hit(FaultPoint::UpgradeCopied);
    }
    for dir in dirs.values() {
        dir.sync()?;
    }
    validate(deploy, entries)
}

pub(super) fn validate(deploy: &Deployment, entries: &[Entry]) -> io::Result<()> {
    validate_entries(deploy, entries, true)?;
    let roots = open_roots(deploy)?;
    for (index, entry) in entries.iter().enumerate() {
        if let Node::File {
            copy: Some(proof), ..
        } = &entry.node
        {
            let root = &roots[&entry.role];
            let copies = root.directory(copy_dir())?;
            let mut payload = copies.file(OsStr::new(&index.to_string()))?;
            snapshot::check(&mut payload, proof)?;
        }
    }
    Ok(())
}

pub(super) fn restore(deploy: &Deployment, entries: &[Entry]) -> io::Result<()> {
    validate(deploy, entries)?;
    let roots = open_roots(deploy)?;
    // Children cannot be redirected through a planted symlink, including
    // symlinks that point to a different directory inside this same root.
    for entry in entries {
        let root = &roots[&entry.role];
        let raw = entry.decode()?;
        let path = Path::new(OsStr::from_bytes(&raw));
        if matches!(entry.node, Node::Directory { .. }) {
            let dir = match root.directory(path.as_os_str()) {
                Ok(dir) => dir,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    let folder = parent(root, path)?;
                    folder.as_ref().unwrap_or(root).mkdir(name(path))?
                }
                Err(error) => return Err(entry_error(entry.role, path, error)),
            };
            // Populate children before restoring restrictive directory modes.
            dir.chmod(0o700)?;
        }
    }
    let mut saved = BTreeMap::<Root, BTreeSet<Vec<u8>>>::new();
    for entry in entries {
        if !matches!(entry.node, Node::Absent) {
            saved.entry(entry.role).or_default().insert(entry.decode()?);
        }
    }
    for (role, root) in &roots {
        remove_new(root, Path::new(""), saved.get(role), true)?;
    }
    for (index, entry) in entries.iter().enumerate() {
        let root = &roots[&entry.role];
        let raw = entry.decode()?;
        let path = Path::new(OsStr::from_bytes(&raw));
        let folder = match parent(root, path) {
            Err(error)
                if matches!(entry.node, Node::Absent)
                    && error.kind() == io::ErrorKind::NotFound =>
            {
                continue;
            }
            result => result?,
        };
        let folder = folder.as_ref().unwrap_or(root);
        if matches!(entry.node, Node::Directory { .. }) {
            continue;
        }
        if matches!(entry.node, Node::Absent) {
            remove_if_present(folder, name(path))?;
            hit(FaultPoint::UpgradeRestoreFile);
            continue;
        }
        let stage = OsStr::new("restore-staging");
        let copies = match root.directory(copy_dir()) {
            Ok(copies) => copies,
            Err(error) if error.kind() == io::ErrorKind::NotFound => root.mkdir(copy_dir())?,
            Err(error) => return Err(error),
        };
        remove_if_present(&copies, stage)?;
        match &entry.node {
            Node::File { mode, .. } => {
                let mut source = copies.file(OsStr::new(&index.to_string()))?;
                let mut output = copies.create(stage)?;
                io::copy(&mut source, &mut output)?;
                output.set_permissions(Permissions::from_mode(*mode))?;
                output.sync_all()?;
            }
            Node::HardLink { anchor } => {
                let raw = entries[*anchor].decode()?;
                let anchor = Path::new(OsStr::from_bytes(&raw));
                let from = parent(root, anchor)?;
                from.as_ref()
                    .unwrap_or(root)
                    .link(name(anchor), &copies, stage)?;
            }
            Node::Symlink { target } => copies.symlink(&decode(target)?, stage)?,
            Node::Directory { .. } | Node::Absent => unreachable!("handled above"),
        }
        copies.replace(stage, folder, name(path))?;
        hit(FaultPoint::UpgradeRestoreFile);
    }
    for entry in entries.iter().rev() {
        if let Node::Directory { mode } = entry.node {
            let root = &roots[&entry.role];
            let raw = entry.decode()?;
            root.directory(OsStr::from_bytes(&raw))?.chmod(mode)?;
        }
    }
    Ok(())
}

// A private startup can create authority (for example a new journal sidecar).
// Drop additions under the locked snapshot roots while keeping runtime exclusions.
fn remove_new(
    dir: &Tree,
    prefix: &Path,
    saved: Option<&BTreeSet<Vec<u8>>>,
    kept: bool,
) -> io::Result<()> {
    for name in dir.names()? {
        let path = prefix.join(&name);
        if excluded(&path) {
            continue;
        }
        let stat = dir.inspect(&name)?;
        let present = saved.is_some_and(|saved| saved.contains(path_bytes(&path)));
        match FileType::from_raw_mode(stat.st_mode) {
            FileType::Directory if present => {
                remove_new(&dir.directory(&name)?, &path, saved, true)?
            }
            FileType::Directory => {
                let nested = dir.directory(&name)?;
                nested.chmod(0o700)?;
                remove_new(&nested, &path, None, false)?;
                dir.remove(&name, AtFlags::REMOVEDIR)?;
            }
            FileType::Socket if kept => {}
            FileType::Socket | FileType::RegularFile | FileType::Symlink if !present => {
                dir.remove(&name, AtFlags::empty())?;
            }
            FileType::RegularFile | FileType::Symlink => {}
            _ => {
                return Err(io::Error::other(format!(
                    "unsupported entry during restore: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn remove_if_present(folder: &Tree, name: &OsStr) -> io::Result<()> {
    match folder.remove(name, AtFlags::empty()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(super) fn discard(deploy: &Deployment) -> io::Result<()> {
    for role in ROLES {
        let Ok(path) = deploy.root(role) else {
            continue;
        };
        let root = Tree::open(path)?;
        let copies = match root.directory(copy_dir()) {
            Ok(copies) => copies,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for name in copies.names()? {
            // The private directory contains only flat payloads and one stage.
            // A directory or mount here is unexpected and must stay untouched.
            copies.inspect(&name)?;
            remove_if_present(&copies, &name)?;
        }
        root.remove(copy_dir(), AtFlags::REMOVEDIR)?;
    }
    Ok(())
}
