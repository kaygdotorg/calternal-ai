//! The high-water witness (#918), stored outside backups.
//!
//! The file is one preallocated page with two checksummed slots. A flush
//! overwrites the older slot in place, so an incomplete write leaves the
//! previous record available for the next bounded startup read.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// What one valid witness slot holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mark {
    /// The journal epoch whose head was witnessed.
    pub epoch: String,
    /// The newest journal position in that epoch.
    pub seq: i64,
    /// The entry check for `seq`, or zero at position zero.
    pub check: u32,
}

/// The whole file is one page after its first valid witness is written.
pub(crate) const PAGE_BYTES: usize = 4096;
const SLOT_BYTES: usize = 64;
const SLOT_OFFSETS: [u64; 2] = [0, (PAGE_BYTES / 2) as u64];
const SLOT_MAGIC: &[u8; 4] = b"CHW2";
const CHECKSUM_END: usize = 41;
const CHECKSUM_START: usize = CHECKSUM_END;
const CHECKSUM_BYTES: usize = 16;
const EPOCH_BYTES: usize = 16;

/// The path and last durable slot state used to open the witness afresh per flush.
pub(crate) struct Writer {
    /// Configured witness path opened afresh for every flush.
    path: PathBuf,
    /// The most recent valid slot sequence in the file.
    sequence: u64,
    /// Which slot held that sequence, when a valid slot exists.
    active_slot: Option<usize>,
    /// The newest mark this writer has durably stored.
    mark: Option<Mark>,
}

impl Writer {
    /// Reads an existing page, returning its newest valid record.
    ///
    /// A missing path stays absent until the first valid mark is ready. A
    /// short text witness from the earlier format is carried into slot zero
    /// before the store starts. Existing owner-only pages are not chmodded on
    /// every open; only loose modes are repaired.
    pub(crate) fn open(path: &Path) -> io::Result<(Self, Option<Mark>)> {
        let file = match OpenOptions::new().read(true).write(true).open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok((
                    Self {
                        path: path.to_owned(),
                        sequence: 0,
                        active_slot: None,
                        mark: None,
                    },
                    None,
                ));
            }
            Err(error) => return Err(error),
        };
        let metadata = file.metadata()?;
        let length = metadata.len();
        if needs_private_mode_repair(metadata.mode()) {
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        let bytes = read_page(&file)?;
        let (mark, sequence, active_slot) = if length == PAGE_BYTES as u64 {
            read_slots(&bytes).unwrap_or((None, 0, None))
        } else {
            // Not a witness page (a new, empty file): start one with no mark.
            file.set_len(PAGE_BYTES as u64)?;
            write_all_at(&file, &[0; PAGE_BYTES], 0)?;
            file.sync_data()?;
            (None, 0, None)
        };

        Ok((
            Self {
                path: path.to_owned(),
                sequence,
                active_slot,
                mark: mark.clone(),
            },
            mark,
        ))
    }

    /// Opens the configured path, writes one slot, syncs it, and checks the path again.
    ///
    /// A new inode with the same valid mark is equivalent evidence. Checks
    /// before the write and after sync catch replacement while this descriptor
    /// was open; the last durable mark is restored before reporting a mismatch.
    pub(crate) fn write(&mut self, mark: &Mark) -> io::Result<()> {
        let durable = self.mark.as_ref().unwrap_or(mark).clone();
        let (file, created) = open_path(&self.path)?;
        let length = file.metadata()?.len();
        let bytes = read_page(&file)?;
        let (on_disk, disk_sequence, disk_slot) =
            read_slots(&bytes).unwrap_or((None, self.sequence, self.active_slot));
        if !fd_names_path(&file, &self.path) {
            drop(file);
            self.repair_path(&durable)?;
            return Err(path_replaced_error());
        }
        let first_page = length == 0 && self.mark.is_none() && self.sequence == 0;
        if (!first_page && length != PAGE_BYTES as u64)
            || self
                .mark
                .as_ref()
                .is_some_and(|previous| on_disk.as_ref() != Some(previous))
        {
            let durable = self.mark.as_ref().unwrap_or(mark).clone();
            drop(file);
            self.repair_path(&durable)?;
            return Err(path_replaced_error());
        }

        file.set_permissions(fs::Permissions::from_mode(0o600))?;

        let sequence = self
            .sequence
            .max(disk_sequence)
            .checked_add(1)
            .ok_or_else(|| io::Error::other("high-water slot sequence overflow"))?;
        let slot = disk_slot
            .or(self.active_slot)
            .map_or(0, |active| 1 - active);
        let bytes = encode_slot(sequence, mark)?;
        if first_page {
            file.set_len(PAGE_BYTES as u64)?;
        }
        write_all_at(&file, &bytes, SLOT_OFFSETS[slot])?;
        file.sync_data()?;
        if created {
            sync_parent(&self.path)?;
        }
        self.finish_synced_write(file, mark, sequence, slot)
    }

    /// Checks the current path's durable mark or restores the last durable mark there.
    ///
    /// This is used when the journal head already matches the last flush. A
    /// replacement is repaired and reported before another rotation can rely on it.
    pub(crate) fn verify_path_or_recover(&mut self, mark: &Mark) -> io::Result<()> {
        let durable = self.mark.as_ref().unwrap_or(mark).clone();
        if path_has_mark(&self.path, &durable).unwrap_or(false) {
            return Ok(());
        }
        self.repair_path(&durable)?;
        Err(path_replaced_error())
    }

    /// Records a synced slot only while its descriptor still names the path.
    fn finish_synced_write(
        &mut self,
        file: File,
        mark: &Mark,
        sequence: u64,
        slot: usize,
    ) -> io::Result<()> {
        if !fd_names_path(&file, &self.path) {
            let durable = self.mark.as_ref().unwrap_or(mark).clone();
            drop(file);
            self.repair_path(&durable)?;
            return Err(path_replaced_error());
        }
        self.sequence = sequence;
        self.active_slot = Some(slot);
        self.mark = Some(mark.clone());
        Ok(())
    }

    /// Restores the last durable mark with the slow atomic rewrite after path change.
    fn repair_path(&mut self, mark: &Mark) -> io::Result<()> {
        let disk_sequence = path_sequence(&self.path)?;
        let sequence = self
            .sequence
            .max(disk_sequence)
            .checked_add(1)
            .ok_or_else(|| io::Error::other("high-water slot sequence overflow"))?;
        let record = encode_slot(sequence, mark)?;
        let mut page = [0; PAGE_BYTES];
        page[..SLOT_BYTES].copy_from_slice(&record);
        calternal_fsio::write_atomic_mode(&self.path, &page, 0o600)?;

        let file = File::open(&self.path)?;
        let valid = file.metadata()?.len() == PAGE_BYTES as u64
            && read_slots(&read_page(&file)?).is_some_and(|(found, found_sequence, active)| {
                found.as_ref() == Some(mark) && found_sequence == sequence && active == Some(0)
            });
        if !valid || !fd_names_path(&file, &self.path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "rewritten high-water page did not verify",
            ));
        }
        self.sequence = sequence;
        self.active_slot = Some(0);
        self.mark = Some(mark.clone());
        Ok(())
    }
}

/// Reports whether a witness mode needs to be restricted to owner read/write.
fn needs_private_mode_repair(mode: u32) -> bool {
    mode & 0o7777 != 0o600
}

/// Reads the newest slot and checks whether the configured path holds `mark`.
fn path_has_mark(path: &Path, mark: &Mark) -> io::Result<bool> {
    let file = File::open(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    if file.metadata()?.len() != PAGE_BYTES as u64 {
        return Ok(false);
    }
    Ok(read_slots(&read_page(&file)?).is_some_and(|(found, _, _)| found.as_ref() == Some(mark)))
}

/// Reads a replacement page's sequence so the recovery slot stays newer.
fn path_sequence(path: &Path) -> io::Result<u64> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    if file.metadata()?.len() != PAGE_BYTES as u64 {
        return Ok(0);
    }
    Ok(read_slots(&read_page(&file)?).map_or(0, |(_, sequence, _)| sequence))
}

/// Opens a fresh descriptor and reports whether it created the path without clobbering a race.
fn open_path(path: &Path) -> io::Result<(File, bool)> {
    match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => Ok((file, false)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
            {
                Ok(file) => Ok((file, true)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(path)
                    .map(|file| (file, false)),
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

/// Returns whether the synced descriptor is still the inode named by the path.
fn fd_names_path(file: &File, path: &Path) -> bool {
    let Ok(opened) = file.metadata() else {
        return false;
    };
    let Ok(named) = fs::metadata(path) else {
        return false;
    };
    opened.nlink() > 0 && opened.dev() == named.dev() && opened.ino() == named.ino()
}

/// Describes a witness path change that must be retried after durable repair.
fn path_replaced_error() -> io::Error {
    io::Error::other("high-water path changed; witness was repaired and must be retried")
}

/// Reads at most one page and returns the newest checksum-valid slot.
#[cfg(test)]
pub(crate) fn read(path: &Path) -> Option<Mark> {
    let file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let bytes = read_page(&file).ok()?;
    if length == PAGE_BYTES as u64 {
        read_slots(&bytes)?.0
    } else {
        None
    }
}

/// Opens a writer for tests that do not keep a handle between updates.
#[cfg(test)]
pub(crate) fn write(path: &Path, mark: &Mark) -> io::Result<()> {
    let (mut writer, _) = Writer::open(path)?;
    writer.write(mark)
}

/// Reads the page with a single bounded `pread` call.
fn read_page(file: &File) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0; PAGE_BYTES];
    let read = file.read_at(&mut bytes, 0)?;
    bytes.truncate(read);
    Ok(bytes)
}

/// Chooses the valid slot with the greatest write sequence.
fn read_slots(bytes: &[u8]) -> Option<(Option<Mark>, u64, Option<usize>)> {
    if bytes.len() != PAGE_BYTES {
        return None;
    }
    let mut newest = None;
    for (slot, offset) in SLOT_OFFSETS.iter().enumerate() {
        let start = *offset as usize;
        if let Some((sequence, mark)) = decode_slot(&bytes[start..start + SLOT_BYTES])
            && newest
                .as_ref()
                .is_none_or(|(previous, _, _): &(u64, Mark, usize)| sequence > *previous)
        {
            newest = Some((sequence, mark, slot));
        }
    }
    newest
        .map(|(sequence, mark, slot)| (Some(mark), sequence, Some(slot)))
        .or(Some((None, 0, None)))
}

/// Encodes a record into its fixed 64-byte slot.
fn encode_slot(sequence: u64, mark: &Mark) -> io::Result<[u8; SLOT_BYTES]> {
    let epoch = mark.epoch.as_bytes();
    if sequence == 0 || epoch.is_empty() || epoch.len() > EPOCH_BYTES || mark.seq < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "high-water mark does not fit its fixed slot",
        ));
    }
    let mut bytes = [0; SLOT_BYTES];
    bytes[..4].copy_from_slice(SLOT_MAGIC);
    bytes[4..12].copy_from_slice(&sequence.to_le_bytes());
    bytes[12] = epoch.len() as u8;
    bytes[13..13 + epoch.len()].copy_from_slice(epoch);
    bytes[29..37].copy_from_slice(&mark.seq.to_le_bytes());
    bytes[37..41].copy_from_slice(&mark.check.to_le_bytes());
    let checksum = blake3::hash(&bytes[..CHECKSUM_END]);
    bytes[CHECKSUM_START..CHECKSUM_START + CHECKSUM_BYTES]
        .copy_from_slice(&checksum.as_bytes()[..CHECKSUM_BYTES]);
    Ok(bytes)
}

/// Decodes one slot only when its bounds and checksum are valid.
fn decode_slot(bytes: &[u8]) -> Option<(u64, Mark)> {
    if bytes.len() != SLOT_BYTES || &bytes[..4] != SLOT_MAGIC {
        return None;
    }
    let sequence = u64::from_le_bytes(bytes[4..12].try_into().ok()?);
    let epoch_len = usize::from(bytes[12]);
    if sequence == 0 || epoch_len == 0 || epoch_len > EPOCH_BYTES {
        return None;
    }
    let expected = blake3::hash(&bytes[..CHECKSUM_END]);
    if bytes[CHECKSUM_START..CHECKSUM_START + CHECKSUM_BYTES]
        != expected.as_bytes()[..CHECKSUM_BYTES]
    {
        return None;
    }
    let epoch = std::str::from_utf8(&bytes[13..13 + epoch_len])
        .ok()?
        .to_owned();
    let seq = i64::from_le_bytes(bytes[29..37].try_into().ok()?);
    if seq < 0 {
        return None;
    }
    let check = u32::from_le_bytes(bytes[37..41].try_into().ok()?);
    Some((sequence, Mark { epoch, seq, check }))
}

/// Completes a possibly short `pwrite` without allocating another record.
fn write_all_at(file: &File, bytes: &[u8], offset: u64) -> io::Result<()> {
    let mut written = 0;
    while written < bytes.len() {
        let count = file.write_at(&bytes[written..], offset + written as u64)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "could not write the full high-water slot",
            ));
        }
        written += count;
    }
    Ok(())
}

/// Makes a newly created directory entry durable after allocating its page.
fn sync_parent(path: &Path) -> io::Result<()> {
    let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Ok(());
    };
    File::open(parent)?.sync_all()
}

#[cfg(test)]
mod tests {
    //! Checks fixed-page persistence and recovery from interrupted slot writes.

    use super::*;
    use std::fs::OpenOptions;
    use std::path::PathBuf;

    /// A private scratch directory for one witness file test.
    struct TestDir(PathBuf);

    impl TestDir {
        /// Creates an isolated directory so each record starts from an empty file.
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "calternal-highwater-{}",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        /// Returns this test directory's witness path.
        fn head(&self) -> PathBuf {
            self.0.join("head")
        }
    }

    impl Drop for TestDir {
        /// Removes the scratch files after each record test.
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Builds a small known witness for slot-format tests.
    fn mark(seq: i64, check: u32) -> Mark {
        Mark {
            epoch: "epoch-a".into(),
            seq,
            check,
        }
    }

    /// Permission repair is needed only when a witness is not owner-only.
    #[test]
    fn only_non_private_witness_modes_need_repair() {
        assert!(!needs_private_mode_repair(0o100600));
        assert!(needs_private_mode_repair(0o100644));
        assert!(needs_private_mode_repair(0o104600));
    }

    /// An existing page with broad permissions is repaired before it is trusted.
    #[test]
    fn opening_a_non_private_witness_restricts_its_mode() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let dir = TestDir::new();
        let path = dir.head();
        write(&path, &mark(1, 0x1234)).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let (_writer, opened_mark) = Writer::open(&path).unwrap();

        assert_eq!(opened_mark, Some(mark(1, 0x1234)));
        assert_eq!(std::fs::metadata(path).unwrap().mode() & 0o777, 0o600);
    }

    /// A witness stays within one preallocated page instead of growing as text.
    #[test]
    fn highwater_file_is_one_preallocated_page() {
        let dir = TestDir::new();
        write(&dir.head(), &mark(1, 0x1234)).unwrap();

        assert_eq!(
            std::fs::metadata(dir.head()).unwrap().len(),
            PAGE_BYTES as u64
        );
        assert_eq!(read(&dir.head()), Some(mark(1, 0x1234)));
    }

    /// A replacement path gets an atomic durable repair and recovers next time.
    #[test]
    fn a_changed_path_is_atomically_repaired_and_recovers_on_the_next_flush() {
        use std::os::unix::fs::MetadataExt;

        let dir = TestDir::new();
        let path = dir.head();
        let saved = dir.0.join("head.saved");
        let (mut writer, _) = Writer::open(&path).unwrap();
        writer.write(&mark(1, 0x1111)).unwrap();

        calternal_fsio::rename_safely(&path, &saved).unwrap();
        calternal_fsio::write_atomic(&path, b"different replacement").unwrap();
        let replacement_inode = std::fs::metadata(&path).unwrap().ino();

        assert!(writer.write(&mark(2, 0x2222)).is_err());
        assert_ne!(
            std::fs::metadata(&path).unwrap().ino(),
            replacement_inode,
            "repair replaces the changed path durably"
        );
        assert_eq!(read(&path), Some(mark(1, 0x1111)));
        assert_eq!(read(&saved), Some(mark(1, 0x1111)));

        writer.write(&mark(2, 0x2222)).unwrap();
        assert_eq!(read(&path), Some(mark(2, 0x2222)));
    }

    /// A missing witness stays absent until its first valid slot is ready.
    #[test]
    fn opening_a_missing_witness_defers_page_creation() {
        let dir = TestDir::new();
        let path = dir.head();
        let (mut writer, opened_mark) = Writer::open(&path).unwrap();

        assert!(opened_mark.is_none());
        assert!(!path.exists(), "open must not sync a blank page");

        let first = mark(1, 0x1234);
        writer.write(&first).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), PAGE_BYTES as u64);
        assert_eq!(read(&path), Some(first));
    }

    /// A path replacement after sync cannot make the old descriptor authoritative.
    #[test]
    fn a_replaced_path_after_sync_is_not_current_for_publication() {
        let dir = TestDir::new();
        let path = dir.head();
        let saved = dir.0.join("head.saved");
        let previous = mark(1, 0x1111);
        let next = mark(2, 0x2222);
        let (mut writer, _) = Writer::open(&path).unwrap();
        writer.write(&previous).unwrap();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let (_, sequence, active) = read_slots(&read_page(&file).unwrap()).unwrap();
        let slot = 1 - active.unwrap();
        let sequence = sequence + 1;
        write_all_at(
            &file,
            &encode_slot(sequence, &next).unwrap(),
            SLOT_OFFSETS[slot],
        )
        .unwrap();
        file.sync_data().unwrap();
        calternal_fsio::rename_safely(&path, &saved).unwrap();
        std::fs::copy(&saved, &path).unwrap();

        assert!(
            writer
                .finish_synced_write(file, &next, sequence, slot)
                .is_err()
        );
        assert_eq!(read(&path), Some(previous));

        writer.write(&next).unwrap();
        assert_eq!(read(&path), Some(next));
    }

    /// A partial slot from a process that stopped during pwrite cannot hide the prior slot.
    #[test]
    fn a_crash_mid_flush_keeps_the_previous_valid_slot() {
        let dir = TestDir::new();
        let path = dir.head();
        write(&path, &mark(1, 0x1111)).unwrap();

        let file = OpenOptions::new().write(true).open(&path).unwrap();
        file.write_at(&[0xA5; 24], SLOT_OFFSETS[1]).unwrap();
        drop(file);

        assert_eq!(read(&path), Some(mark(1, 0x1111)));
    }

    /// A complete newer slot with a damaged checksum falls back to the older slot.
    #[test]
    fn a_torn_slot_falls_back_to_the_previous_valid_slot() {
        let dir = TestDir::new();
        let path = dir.head();
        write(&path, &mark(1, 0x1111)).unwrap();
        write(&path, &mark(2, 0x2222)).unwrap();

        let file = OpenOptions::new().write(true).open(&path).unwrap();
        file.write_at(&[0xFF], SLOT_OFFSETS[1] + CHECKSUM_START as u64)
            .unwrap();
        file.sync_data().unwrap();

        assert_eq!(read(&path), Some(mark(1, 0x1111)));
    }
}
