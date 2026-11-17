//! Writing a file without losing what another app wrote a moment before.
//!
//! A save checks the file, then writes. Another app can rename its own
//! version over the file in between, and a plain rename would then throw
//! that version away without anyone noticing (#222).
//!
//! So the check happens last, right before the new file goes in, and the
//! new file is swapped in (one atomic exchange) instead of renamed over.
//! The file it replaced is then still there under the temporary name: if
//! it is not the very file that was checked (another app renamed a new
//! one in since), it is swapped straight back, and the caller merges it
//! and tries again. Between the two swaps a reader could see our version.
//! Settings use [`Prepared::place_exact_recoverable`] and [`recover_exact`]
//! to recover this interval after process death before reading the file.

use super::{parent, sync_dir, temp_path};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

/// Writes `bytes` to `path` like [`crate::write_atomic`], unless the file
/// holds something the caller didn't expect: `expected` says whether the
/// content about to be replaced is the one the caller last saw.
///
/// Returns `None` when written, or `Some(found)` when `path` held
/// something else; then nothing was written and `path` holds `found`.
///
/// Missing destinations are placed exclusively, so a racing create is
/// preserved and returned to the caller. Existing files require an atomic
/// exchange; unsupported filesystems fail without replacing their content.
pub fn write_unless_changed(
    path: &Path,
    bytes: &[u8],
    expected: impl Fn(&[u8]) -> bool,
) -> io::Result<Option<Vec<u8>>> {
    Prepared::new(path, bytes)?.place(expected)
}

/// [`write_unless_changed`] in two halves, so a caller can do something
/// else while the new content goes to the disk: [`Prepared::new`] writes
/// it under a temporary name and flushes it; [`Prepared::place`] checks
/// the file and puts the new content in. A note save records its pending
/// write in history at the same time as the first half (#298).
pub struct Prepared {
    path: std::path::PathBuf,
    tmp: std::path::PathBuf,
    bytes: Vec<u8>,
    metadata: Option<std::fs::Metadata>,
    placed: bool,
    retain: Option<std::path::PathBuf>,
}

impl Prepared {
    /// Writes `bytes` next to `path` under a temporary name and flushes
    /// it. `path` itself is untouched.
    pub fn new(path: &Path, bytes: &[u8]) -> io::Result<Self> {
        let tmp = temp_path(path)?;
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        let mut me = Self {
            path: path.to_owned(),
            tmp,
            bytes: bytes.to_vec(),
            metadata: None,
            placed: false,
            retain: None,
        };
        // From here on, dropping `me` removes the temporary file.
        f.write_all(bytes).and_then(|()| f.sync_all())?;
        me.metadata = f.metadata().ok();
        drop(f);
        Ok(me)
    }

    /// Metadata of the bytes flushed under the temporary name. This describes
    /// our write image, not whatever may later occupy the destination path.
    pub fn metadata(&self) -> Option<&std::fs::Metadata> {
        self.metadata.as_ref()
    }

    /// Retains the replaced inode at this exclusive destination instead of
    /// unlinking it. Undo uses a Trash entry so already-open writers survive.
    pub fn retain_replaced_at(mut self, path: std::path::PathBuf) -> Self {
        self.retain = Some(path);
        self
    }

    /// Checks `path` and puts the new content in, unless it holds
    /// something `expected` doesn't accept. Answers like
    /// [`write_unless_changed`].
    pub fn place(self, expected: impl Fn(&[u8]) -> bool) -> io::Result<Option<Vec<u8>>> {
        self.place_checked(expected, true)
    }

    /// Places only over the exact bytes supplied, or exclusively into a missing
    /// destination when `expected` is `None`. Returns true on a conflict,
    /// including an outside deletion of a previously existing destination.
    pub fn place_exact(self, expected: Option<&[u8]>) -> io::Result<bool> {
        self.place_checked(|bytes| expected == Some(bytes), expected.is_none())
            .map(|found| found.is_some())
    }

    /// Like [`Self::place_exact`], with a durable intent for restart recovery.
    /// The caller must serialize saves and call [`recover_exact`] before each
    /// read. An interrupted conflicting save is dropped and the displaced file
    /// is restored before the caller may trust the destination.
    pub fn place_exact_recoverable(mut self, expected: Option<&[u8]>) -> io::Result<bool> {
        let Some(base) = expected else {
            return self.place_exact(None);
        };
        publish_intent(&self.path, &self.tmp, base, &self.bytes)?;
        // From intent publication onward even an early error leaves recovery
        // in charge of the temporary file. Drop must not discard outside data.
        self.placed = true;
        let path = self.path.clone();
        let result = self.place_exact(Some(base));
        if result.is_ok() {
            recover_exact(&path)?;
        }
        result
    }

    fn place_checked(
        mut self,
        expected: impl Fn(&[u8]) -> bool,
        missing_allowed: bool,
    ) -> io::Result<Option<Vec<u8>>> {
        let (path, tmp) = (self.path.clone(), self.tmp.clone());
        let dir = parent(&path)?;
        let result = match self.retain.as_deref() {
            Some(retain) => put_retaining(&path, &tmp, expected, retain),
            None => put(&path, &tmp, &self.bytes, expected, missing_allowed),
        };
        let found = match result {
            Ok(Placed::Written) => {
                self.placed = true;
                None
            }
            Ok(Placed::Found(b)) => Some(b),
            // From the swap on, the temporary name may hold someone else's
            // file: it is never removed on an error.
            Ok(Placed::Swapped(r)) => {
                self.placed = true;
                r?
            }
            Err(e) => return Err(e),
        };
        sync_dir(dir)?;
        if found.is_none() {
            crate::usage::changed(&path);
        }
        Ok(found)
    }
}

impl Drop for Prepared {
    fn drop(&mut self) {
        if !self.placed {
            let _ = std::fs::remove_file(&self.tmp);
        }
    }
}

fn intent_path(path: &Path) -> io::Result<std::path::PathBuf> {
    Ok(parent(path)?.join(format!(
        "{}intent-{}",
        crate::TEMP_PREFIX,
        crate::file_name(path)?
    )))
}

fn publish_intent(path: &Path, tmp: &Path, base: &[u8], save: &[u8]) -> io::Result<()> {
    let intent = intent_path(path)?;
    let mut record = crate::file_name(tmp)?.into_bytes();
    if record.contains(&b'\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "newline in exchange filename",
        ));
    }
    record.push(b'\n');
    record.extend_from_slice(&(base.len() as u64).to_le_bytes());
    record.extend_from_slice(base);
    record.extend_from_slice(save);
    // Exclusive publication prevents overwriting an unresolved intent. This
    // also flushes the folder containing both the intent and the save image.
    if Prepared::new(&intent, &record)?.place_exact(None)? {
        return Err(io::Error::other("an earlier exchange needs recovery"));
    }
    Ok(())
}

/// Recovers an interrupted [`Prepared::place_exact_recoverable`] before a read.
/// The intent holds exact base and save bytes, not metadata or a weak hash.
/// A leftover save image means the exchange did not happen or was undone; a
/// leftover base means it completed. Any other displaced image must go back.
/// Recovery itself can be interrupted: after rollback the save image is under
/// the temporary name, so the next recovery only has to finish cleanup.
/// Errors refuse access rather than let a caller trust an unresolved save.
/// Like placement, this requires a single application writer per destination.
pub fn recover_exact(path: &Path) -> io::Result<()> {
    let intent = intent_path(path)?;
    let record = match std::fs::read(&intent) {
        Ok(record) => record,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "invalid exchange intent");
    let newline = record
        .iter()
        .position(|b| *b == b'\n')
        .ok_or_else(invalid)?;
    let name = std::str::from_utf8(&record[..newline]).map_err(|_| invalid())?;
    if !name.starts_with(crate::TEMP_PREFIX)
        || name.contains(['/', '\\'])
        || name == crate::file_name(&intent)?
    {
        return Err(invalid());
    }
    let payload = record.get(newline + 1..).ok_or_else(invalid)?;
    let size = u64::from_le_bytes(payload.get(..8).ok_or_else(invalid)?.try_into().unwrap());
    let size = usize::try_from(size).map_err(|_| invalid())?;
    let payload = &payload[8..];
    let base = payload.get(..size).ok_or_else(invalid)?;
    let save = &payload[size..];
    let tmp = parent(path)?.join(name);
    match std::fs::symlink_metadata(&tmp) {
        Ok(metadata) => {
            let known_image = if metadata.is_file() {
                let displaced = std::fs::read(&tmp)?;
                displaced == base || displaced == save
            } else {
                false
            };
            if !known_image {
                // Only replace our save. An outside edit during recovery must
                // remain visible, with the intent retained and reads refused.
                if !std::fs::symlink_metadata(path)?.is_file() || std::fs::read(path)? != save {
                    return Err(io::Error::other(
                        "settings changed during exchange recovery",
                    ));
                }
                swap(&tmp, path)?;
                if !std::fs::symlink_metadata(&tmp)?.is_file() || std::fs::read(&tmp)? != save {
                    swap(&tmp, path)?;
                    return Err(io::Error::other(
                        "settings changed during exchange recovery",
                    ));
                }
                sync_dir(parent(path)?)?;
                crate::usage::changed(path);
            }
            std::fs::remove_file(&tmp)?;
        }
        // Cleanup can have removed the temporary name before process death.
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    // Persist removal of displaced data only after placement/rollback, then
    // remove the intent. Repeating either cleanup step is safe.
    sync_dir(parent(path)?)?;
    std::fs::remove_file(intent)?;
    sync_dir(parent(path)?)
}

#[cfg(feature = "test-hooks")]
type ExchangeHooks = [Option<Box<dyn FnOnce()>>; 2];

#[cfg(feature = "test-hooks")]
thread_local! {
    static EXCHANGE_HOOKS: std::cell::RefCell<ExchangeHooks> =
        const { std::cell::RefCell::new([None, None]) };
}

/// Installs one-shot hooks on this thread, immediately before and after the
/// exchange syscall. Only enabled by test dependents for process-death tests.
#[cfg(feature = "test-hooks")]
pub fn set_exchange_hooks(before: impl FnOnce() + 'static, after: impl FnOnce() + 'static) {
    EXCHANGE_HOOKS
        .with(|hooks| *hooks.borrow_mut() = [Some(Box::new(before)), Some(Box::new(after))]);
}

fn exchange_hook(_before: bool) {
    #[cfg(feature = "test-hooks")]
    EXCHANGE_HOOKS.with(|hooks| {
        let hook = hooks.borrow_mut()[usize::from(!_before)].take();
        if let Some(hook) = hook {
            hook();
        }
    });
}

enum Placed {
    /// Ours is in place.
    Written,
    /// The check found someone else's version; nothing was written.
    Found(Vec<u8>),
    /// Swapped in (and maybe back again); the temporary name is dealt
    /// with.
    Swapped(io::Result<Option<Vec<u8>>>),
}

/// Move the old inode durably into the Trash before publishing our image.
/// A crash anywhere in this sequence leaves the outside inode restorable;
/// the note's pending history save recovers an interrupted new placement.
fn put_retaining(
    path: &Path,
    tmp: &Path,
    expected: impl Fn(&[u8]) -> bool,
    retain: &Path,
) -> io::Result<Placed> {
    match std::fs::read(path) {
        Ok(old) if !expected(&old) => return Ok(Placed::Found(old)),
        Ok(_) => {
            crate::rename_safely(path, retain)?;
            let moved = std::fs::read(retain)?;
            if !expected(&moved) {
                // The changed inode is already safe. Put it back if its
                // public name is still free; otherwise retain both versions.
                match crate::rename_safely(retain, path) {
                    Ok(()) => return Ok(Placed::Found(moved)),
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                        return std::fs::read(path).map(Placed::Found);
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    match std::fs::hard_link(tmp, path) {
        Ok(()) => {
            std::fs::remove_file(tmp)?;
            Ok(Placed::Written)
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            std::fs::read(path).map(Placed::Found)
        }
        Err(e) => Err(e),
    }
}

/// Checks `path` and puts `tmp` in its place.
fn put(
    path: &Path,
    tmp: &Path,
    bytes: &[u8],
    expected: impl Fn(&[u8]) -> bool,
    missing_allowed: bool,
) -> io::Result<Placed> {
    put_with_hooks(path, tmp, bytes, expected, || {}, swap, missing_allowed)
}

#[cfg(test)]
fn put_with_create_hook(
    path: &Path,
    tmp: &Path,
    bytes: &[u8],
    expected: impl Fn(&[u8]) -> bool,
    before_create: impl FnOnce(),
) -> io::Result<Placed> {
    put_with_hooks(path, tmp, bytes, expected, before_create, swap, true)
}

fn put_with_hooks(
    path: &Path,
    tmp: &Path,
    bytes: &[u8],
    expected: impl Fn(&[u8]) -> bool,
    before_create: impl FnOnce(),
    exchange: impl Fn(&Path, &Path) -> io::Result<()>,
    missing_allowed: bool,
) -> io::Result<Placed> {
    let checked = match File::open(path) {
        Ok(mut f) => {
            let mut old = Vec::new();
            f.read_to_end(&mut old)?;
            if !expected(&old) {
                return Ok(Placed::Found(old));
            }
            // Kept open until after the swap, so its inode number can't be
            // reused by a newer file and mistaken for it.
            Some((f.metadata()?, f))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    match checked {
        Some((m, _open)) => {
            exchange_hook(true);
            exchange(tmp, path)?;
            exchange_hook(false);
            Ok(Placed::Swapped(after_swap(path, tmp, bytes, &m, &expected)))
        }
        None => {
            if !missing_allowed {
                return Ok(Placed::Found(Vec::new()));
            }
            before_create();
            // Linking a flushed image is exclusive: an outside create must
            // survive even when it arrives after the absence check.
            match std::fs::hard_link(tmp, path) {
                Ok(()) => {
                    std::fs::remove_file(tmp)?;
                    Ok(Placed::Written)
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    std::fs::read(path).map(Placed::Found)
                }
                Err(e) => Err(e),
            }
        }
    }
}

/// `path` holds our bytes and `tmp` what they replaced, which should be
/// the file `checked` describes.
fn after_swap(
    path: &Path,
    tmp: &Path,
    bytes: &[u8],
    checked: &std::fs::Metadata,
    expected: &impl Fn(&[u8]) -> bool,
) -> io::Result<Option<Vec<u8>>> {
    if std::fs::symlink_metadata(tmp).is_ok_and(|m| same_file(&m, checked))
        && std::fs::read(tmp).is_ok_and(|found| expected(&found))
    {
        std::fs::remove_file(tmp)?;
        return Ok(None);
    }
    // Another app renamed a new version in after the check: put it back.
    swap(tmp, path)?;
    if std::fs::read(tmp)? != bytes {
        // It wrote yet again while ours was in place; that version is the
        // newest, so it goes back instead.
        swap(tmp, path)?;
        // The one before it is kept under the hidden temporary name (the
        // watcher ignores it), never thrown away.
        tracing::warn!(file = %tmp.display(), "a note changed many times while saving; one version was set aside");
        return std::fs::read(path).map(Some);
    }
    std::fs::remove_file(tmp)?;
    std::fs::read(path).map(Some)
}

#[cfg(unix)]
pub(super) fn same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}

#[cfg(not(unix))]
pub(super) fn same_file(_: &std::fs::Metadata, _: &std::fs::Metadata) -> bool {
    true
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
fn swap(a: &Path, b: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};
    renameat_with(CWD, a, CWD, b, RenameFlags::EXCHANGE).map_err(io::Error::from)
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
fn swap(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(test)]
mod create_tests {
    use super::*;

    #[test]
    fn recovery_is_idempotent_at_each_exchange_boundary() {
        for phase in [
            "before exchange",
            "after exchange",
            "after rollback",
            "after cleanup",
        ] {
            for outside in [b"base".as_slice(), b"newer outside", b"{ damaged"] {
                let dir = crate::tests::tmp();
                let path = dir.join("settings.json");
                std::fs::write(&path, b"base").unwrap();
                let mut image = Prepared::new(&path, b"save").unwrap();
                publish_intent(&path, &image.tmp, b"base", b"save").unwrap();
                image.placed = true;
                std::fs::write(&path, outside).unwrap();
                if phase != "before exchange" {
                    swap(&image.tmp, &path).unwrap();
                }
                if phase == "after rollback" || phase == "after cleanup" {
                    swap(&image.tmp, &path).unwrap();
                }
                if phase == "after cleanup" {
                    std::fs::remove_file(&image.tmp).unwrap();
                }
                let expected = if phase == "after exchange" && outside == b"base" {
                    b"save".as_slice()
                } else {
                    outside
                };
                drop(image);
                for _ in 0..2 {
                    recover_exact(&path).unwrap();
                    assert_eq!(std::fs::read(&path).unwrap(), expected, "{phase}");
                    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
                }
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }

    #[test]
    fn unresolved_recovery_refuses_access_and_preserves_both_outside_files() {
        let dir = crate::tests::tmp();
        let path = dir.join("settings.json");
        std::fs::write(&path, b"base").unwrap();
        let mut image = Prepared::new(&path, b"save").unwrap();
        publish_intent(&path, &image.tmp, b"base", b"save").unwrap();
        image.placed = true;
        std::fs::write(&path, b"first outside").unwrap();
        swap(&image.tmp, &path).unwrap();
        std::fs::write(&path, b"second outside").unwrap();
        assert!(recover_exact(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"second outside");
        assert_eq!(std::fs::read(&image.tmp).unwrap(), b"first outside");
        assert!(intent_path(&path).unwrap().exists());
        assert!(
            Prepared::new(&path, b"another save")
                .unwrap()
                .place_exact_recoverable(Some(b"second outside"))
                .is_err()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn recovery_restores_displaced_directories_and_dangling_symlinks() {
        for directory in [false, true] {
            let dir = crate::tests::tmp();
            let path = dir.join("settings.json");
            std::fs::write(&path, b"base").unwrap();
            let mut image = Prepared::new(&path, b"save").unwrap();
            publish_intent(&path, &image.tmp, b"base", b"save").unwrap();
            image.placed = true;
            std::fs::remove_file(&path).unwrap();
            if directory {
                std::fs::create_dir(&path).unwrap();
                std::fs::write(path.join("outside"), b"keep").unwrap();
            } else {
                std::os::unix::fs::symlink("missing", &path).unwrap();
            }
            swap(&image.tmp, &path).unwrap();
            recover_exact(&path).unwrap();
            if directory {
                assert_eq!(std::fs::read(path.join("outside")).unwrap(), b"keep");
            } else {
                assert_eq!(std::fs::read_link(&path).unwrap(), Path::new("missing"));
            }
            assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn malformed_intent_refuses_access_without_touching_the_destination() {
        let dir = crate::tests::tmp();
        let path = dir.join("settings.json");
        std::fs::write(&path, b"do not trust this save").unwrap();
        for record in [
            b"truncated".as_slice(),
            b"../outside\n",
            b".calternal-tmp-image\nshort",
        ] {
            std::fs::write(intent_path(&path).unwrap(), record).unwrap();
            assert_eq!(
                recover_exact(&path).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
            assert_eq!(std::fs::read(&path).unwrap(), b"do not trust this save");
            assert_eq!(std::fs::read(intent_path(&path).unwrap()).unwrap(), record);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn exact_placement_preserves_outside_deletion() {
        let dir = crate::tests::tmp();
        let path = dir.join("settings.json");
        std::fs::write(&path, b"old").unwrap();
        let prepared = Prepared::new(&path, b"ours").unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(prepared.place_exact(Some(b"old")).unwrap());
        assert!(!path.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn replacements_at_exchange_preserve_exact_outside_bytes() {
        for in_place in [false, true] {
            for outside in [
                br#"{"version":1,"settings":{"ui.accent":"outside"}}"#.as_slice(),
                br#"{"version":2,"settings":{}}"#.as_slice(),
                b"{ damaged".as_slice(),
            ] {
                let dir = crate::tests::tmp();
                let path = dir.join("settings.json");
                let old = br#"{"version":1,"settings":{}}"#;
                std::fs::write(&path, old).unwrap();
                let image = Prepared::new(&path, b"ours").unwrap();
                let result = put_with_hooks(
                    &path,
                    &image.tmp,
                    &image.bytes,
                    |b| b == old,
                    || {},
                    |from, to| {
                        if in_place {
                            std::fs::write(to, outside)?;
                        } else {
                            crate::write_atomic(to, outside)?;
                        }
                        swap(from, to)
                    },
                    false,
                )
                .unwrap();
                assert!(matches!(result, Placed::Swapped(Ok(Some(ref b))) if b == outside));
                assert_eq!(std::fs::read(&path).unwrap(), outside);
                drop(image);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }

    #[test]
    fn unsupported_exchange_never_replaces_an_existing_file() {
        let dir = crate::tests::tmp();
        let path = dir.join("Existing.md");
        std::fs::write(&path, b"outside text").unwrap();
        let image = Prepared::new(&path, b"app text").unwrap();
        let result = put_with_hooks(
            &path,
            &image.tmp,
            &image.bytes,
            |_| true,
            || {},
            |_, _| Err(io::ErrorKind::Unsupported.into()),
            true,
        );
        assert!(matches!(result, Err(ref e) if e.kind() == io::ErrorKind::Unsupported));
        assert_eq!(std::fs::read(&path).unwrap(), b"outside text");
        drop(image);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn create_preserves_outside_file_appearing_after_absence_check() {
        let dir = crate::tests::tmp();
        let path = dir.join("Race target.md");
        let image = Prepared::new(&path, b"app initial text\n").unwrap();
        let outside = b"outside note that must survive\n";
        let result = put_with_create_hook(
            &path,
            &image.tmp,
            &image.bytes,
            |_| false,
            || {
                std::fs::write(&path, outside).unwrap();
            },
        )
        .unwrap();
        assert!(matches!(result, Placed::Found(ref bytes) if bytes == outside));
        assert_eq!(std::fs::read(&path).unwrap(), outside);
        drop(image);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
