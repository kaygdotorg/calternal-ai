//! Read one authorized linked prompt without projecting the record stream.

use super::{Signature, content, valid_anchor};
use crate::declarations::Kind;
use crate::internal;
use crate::space::{Head, Space};
use calternal_types::{Error, ErrorCode, MAX_FRAME_BYTES, Result};
use rusqlite::{OptionalExtension, params};
use std::io::{Read, Seek, SeekFrom};

impl Space {
    // Open the item once, authenticate the prefix and selected record, and
    // retain at most one frame of record bytes. No receipt or disk write.
    pub(crate) fn linked_record_body(
        &self,
        head: &Head,
        kind: &Kind<'_>,
        anchor: &str,
    ) -> Result<String> {
        let unavailable = || Error::forbidden("The prompt record is outside this run's session.");
        if !valid_anchor(anchor) {
            return Err(unavailable());
        }
        let (offset, length, hash) = self
            .db
            .prepare_cached("SELECT offset,length,hash FROM records WHERE item=?1 AND anchor=?2")
            .map_err(internal)?
            .query_row(params![head.id.to_string(), anchor], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })
            .optional()
            .map_err(internal)?
            .ok_or_else(unavailable)?;
        let offset = u64::try_from(offset).map_err(internal)?;
        let length = u64::try_from(length).map_err(internal)?;
        if length > MAX_FRAME_BYTES as u64 {
            return Err(Error::new(
                ErrorCode::TooLarge,
                "The prompt record exceeds the frame limit.",
            ));
        }
        if offset < head.prefix
            || offset
                .checked_add(length)
                .is_none_or(|end| end > head.length)
        {
            return Err(internal("invalid linked record bounds"));
        }
        let outside =
            || Error::conflict("The linked prompt or its item prefix has outside changes.");
        let mut file = self.open_regular(&head.path).map_err(internal)?;
        let metadata = file.metadata().map_err(internal)?;
        if metadata.len() != head.length {
            return Err(outside());
        }
        let signature = Signature::from(&metadata);
        let mut hasher = blake3::Hasher::new_keyed(&self.key);
        let mut remaining = head.prefix;
        let mut buffer = [0_u8; 8192];
        while remaining > 0 {
            let count = remaining.min(buffer.len() as u64) as usize;
            file.read_exact(&mut buffer[..count]).map_err(internal)?;
            hasher.update(&buffer[..count]);
            remaining -= count as u64;
        }
        if *hasher.finalize().as_bytes() != head.hash {
            return Err(outside());
        }
        file.seek(SeekFrom::Start(offset)).map_err(internal)?;
        let mut bytes = vec![0; length as usize];
        file.read_exact(&mut bytes).map_err(internal)?;
        if self.hash(&bytes) != hash.as_slice()
            || Signature::from(&file.metadata().map_err(internal)?) != signature
        {
            return Err(outside());
        }
        content(&bytes, kind, false).map(|(_, body)| body)
    }
}
