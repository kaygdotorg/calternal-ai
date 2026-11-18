//! Bounded positioned I/O for a prepared Markdown append.
use std::io::{self, Read, Seek, SeekFrom, Write};

pub(super) const WITNESS_BYTES: usize = 4096;

/// Complete an absent or matching partial append without touching its prefix.
/// The caller must fence other writers and recheck the file's identity and
/// length on this descriptor. Read one bounded preceding range and any
/// already-written suffix; make at most one write of the remaining bytes.
pub(super) fn place<T: Read + Write + Seek>(
    file: &mut T,
    length: u64,
    offset: u64,
    witness: blake3::Hash,
    record: &[u8],
    before_write: impl FnOnce(&T) -> io::Result<()>,
) -> io::Result<()> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "thread append witness changed");
    let written = length.checked_sub(offset).ok_or_else(invalid)?;
    let written = usize::try_from(written).map_err(|_| invalid())?;
    let (saved, remaining) = record.split_at_checked(written).ok_or_else(invalid)?;
    let preceding = offset.min(WITNESS_BYTES as u64) as usize;
    let mut buffer = [0; WITNESS_BYTES];
    file.seek(SeekFrom::Start(offset - preceding as u64))?;
    file.read_exact(&mut buffer[..preceding])?;
    if blake3::hash(&buffer[..preceding]) != witness {
        return Err(invalid());
    }
    for expected in saved.chunks(buffer.len()) {
        let found = &mut buffer[..expected.len()];
        file.read_exact(found)?;
        if found != expected {
            return Err(invalid());
        }
    }
    if remaining.is_empty() {
        return Ok(());
    }
    before_write(file)?;
    if file.write(remaining)? != remaining.len() {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "short thread append",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "io_tests.rs"]
mod tests;
