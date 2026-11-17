//! Writing big files without making note saves wait (#298).
//!
//! Left alone, Linux keeps a big write in memory and sends it to the disk
//! later, all at once. A note save has to reach the disk, and on our disks
//! (ZFS volumes) a flush carries everything written before it: a note
//! save during a 1 GB upload waited behind the upload, 2 to 3 s instead
//! of 40 ms, on the perf machine.
//!
//! So a big write goes to the disk in steps of [`STEP`]: each step is
//! flushed before the next one is written, and then dropped from memory
//! (it is on disk, and it shouldn't push the notes people read out of the
//! cache). A note save then never waits behind more than one step.
//!
//! The step size is a measured trade (storage-bench, #298): bigger steps
//! make saves wait longer, smaller ones slow the upload for little gain.
//! At 1 MiB an upload still writes about as fast as a gigabit network
//! brings data in. Going around the cache (`O_DIRECT`) was worse: the upload
//! then reaches the disk at full speed and every save's flush carries it.
//!
//! The caller still flushes the whole file at the end (`fsync`) before it
//! counts as saved.

use std::fs::File;
use std::io::{self, Read, Write};

/// How much of a big write goes to the disk at a time.
pub const STEP: usize = 1 << 20;

/// Copies all of `src` into `f`, in steps. Returns the bytes written.
pub fn copy(f: &mut File, src: &mut impl Read) -> io::Result<u64> {
    let mut buf = vec![0u8; STEP];
    let mut done: u64 = 0;
    loop {
        let n = fill(src, &mut buf)?;
        if n == 0 {
            return Ok(done);
        }
        f.write_all(&buf[..n])?;
        step(f, done, n as u64);
        done += n as u64;
    }
}

/// Writes `bytes` into `f`. Small writes go in one go; big ones in steps.
pub fn write_all(f: &mut File, bytes: &[u8]) -> io::Result<()> {
    if bytes.len() <= STEP {
        return f.write_all(bytes);
    }
    let mut at: u64 = 0;
    for chunk in bytes.chunks(STEP) {
        f.write_all(chunk)?;
        step(f, at, chunk.len() as u64);
        at += chunk.len() as u64;
    }
    Ok(())
}

/// Writes a bounded streaming chunk, flushing whenever the accumulated upload
/// crosses a step boundary. `at` is the number of bytes already written.
pub fn write_chunk(f: &mut File, bytes: &[u8], at: u64) -> io::Result<()> {
    f.write_all(bytes)?;
    let end = at.saturating_add(bytes.len() as u64);
    if end / STEP as u64 > at / STEP as u64 {
        let start = at / STEP as u64 * STEP as u64;
        step(f, start, end - start);
    }
    Ok(())
}

/// Reads until `buf` is full or `src` ends.
fn fill(src: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match src.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

/// The step at `at..at + len` was just written: wait until it is on the
/// disk, then drop it from memory. Errors are left to the caller's final
/// flush, which reports them.
fn step(f: &File, at: u64, len: u64) {
    let _ = f.sync_data();
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let _ = rustix::fs::fadvise(
        f,
        at,
        std::num::NonZeroU64::new(len),
        rustix::fs::Advice::DontNeed,
    );
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let _ = (at, len);
}
