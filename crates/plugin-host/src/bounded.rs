//! Bounded JSON serialization and allocation-free size checks for in-memory values.
use crate::Error;
use serde::Serialize;
use std::io::{self, Write};

struct Budget { written: usize, maximum: usize }
impl Write for Budget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.maximum.saturating_sub(self.written) {
            return Err(io::Error::other("JSON budget exceeded"));
        }
        self.written += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
struct Buffer { bytes: Vec<u8>, budget: Budget }
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.budget.write_all(bytes)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
pub(crate) fn check(value: &impl Serialize, maximum: usize) -> Result<(), Error> {
    serde_json::to_writer(Budget { written: 0, maximum }, value).map_err(|_| Error::TooLarge)
}
pub(crate) fn json(value: &impl Serialize, maximum: usize) -> Result<Vec<u8>, Error> {
    let mut buffer = Buffer { bytes: Vec::with_capacity(maximum.min(4096)), budget: Budget { written: 0, maximum } };
    serde_json::to_writer(&mut buffer, value).map_err(|_| Error::TooLarge)?;
    Ok(buffer.bytes)
}
