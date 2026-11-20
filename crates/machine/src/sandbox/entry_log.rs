//! Private, append-only output of an enrolled entry.
use super::{SandboxError, SandboxResult, entry::EntryOutput};
use calternal_plugin_host::{Error, entry, space_channel::SpaceMessage};
use calternal_types::ErrorCode;
use std::{fs::File, os::unix::fs::FileExt, path::PathBuf, sync::Arc, time::Duration};
use tokio::{io::AsyncWriteExt, sync::watch, time::Instant};

const BATCH_BYTES: usize = 128 * 1024;
const BATCH_DELAY: Duration = Duration::from_millis(8);

/// Machine-local working log of one enrolled entry. The sandbox has no
/// writable mount to it. Its owner must retain it until delivery settles.
pub struct EntryLog {
    pub(crate) path: PathBuf,
    pub(crate) reader: Arc<File>,
    pub(crate) writer: tokio::fs::File,
}

impl EntryLog {
    /// Create a private, uniquely named log, with a synced directory entry.
    pub(crate) async fn create(parent: PathBuf, ticket: uuid::Uuid) -> SandboxResult<Self> {
        tokio::task::spawn_blocking(move || {
            calternal_fsio::private::directory(&parent)?;
            let name = format!("entry-{ticket}-");
            let temporary = tempfile::Builder::new()
                .prefix(&name)
                .tempfile_in(&parent)?;
            let reader = Arc::new(temporary.as_file().try_clone()?);
            calternal_fsio::sync_dir(&parent)?;
            let (writer, path) = temporary.keep().map_err(|error| error.error)?;
            Ok::<_, std::io::Error>(Self {
                path,
                reader,
                writer: tokio::fs::File::from_std(writer),
            })
        })
        .await
        .map_err(SandboxError::new)?
        .map_err(SandboxError::new)
    }
}

/// Drain checked stdout independently of relay progress. Publish only
/// complete, synced batches. Records are bounded with a four-byte length.
/// A zero length denotes a local failure terminal.
pub(crate) async fn record_entry(
    exec: super::Exec,
    mut file: tokio::fs::File,
    published: watch::Sender<u64>,
) -> Result<(), Error> {
    let mut output = EntryOutput::new(exec);
    let mut batch = Vec::with_capacity(BATCH_BYTES);
    let mut through = 0;
    loop {
        let mut next = output.next().await;
        let deadline = Instant::now() + BATCH_DELAY;
        let done = loop {
            let mut done = true;
            let encoded = match next {
                Ok(Some(value)) => {
                    let encoded = serde_json::to_vec(&value).map_err(|_| Error::Invalid)?;
                    if encoded.len() < entry::MAX_ENTRY_FRAME_BYTES {
                        done = value["kind"] == "terminal";
                        encoded
                    } else {
                        Vec::new()
                    }
                }
                _ => Vec::new(),
            };
            batch.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
            batch.extend_from_slice(&encoded);
            through += 1;
            if done || batch.len() >= BATCH_BYTES {
                break done;
            }
            match tokio::time::timeout_at(deadline, output.next()).await {
                Ok(frame) => next = frame,
                Err(_) => break false,
            }
        };
        file.write_all(&batch)
            .await
            .map_err(|_| Error::Invocation)?;
        file.sync_data().await.map_err(|_| Error::Invocation)?;
        published.send_replace(through);
        batch.clear();
        if done {
            return Ok(());
        }
    }
}

/// Read exactly one published record without moving the writer's file
/// offset. Returns the next byte position and a bounded carrier message.
pub(crate) async fn read_entry(file: Arc<File>, offset: u64) -> Result<(u64, SpaceMessage), Error> {
    tokio::task::spawn_blocking(move || {
        let mut header = [0; 4];
        file.read_exact_at(&mut header, offset)
            .map_err(|_| Error::Invocation)?;
        let len = u32::from_le_bytes(header) as usize;
        if len >= entry::MAX_ENTRY_FRAME_BYTES {
            return Err(Error::TooLarge);
        }
        let next = offset.checked_add(4 + len as u64).ok_or(Error::Invalid)?;
        if len == 0 {
            return Ok((
                next,
                SpaceMessage::Complete {
                    result: Err(ErrorCode::Unavailable),
                },
            ));
        }
        let mut frame = vec![0; len + 1];
        file.read_exact_at(&mut frame[..len], offset + 4)
            .map_err(|_| Error::Invocation)?;
        frame[len] = b'\n';
        let value = entry::decode_frame(&frame)?;
        let message = if value["kind"] == "output" {
            SpaceMessage::Chunk { data: value }
        } else {
            SpaceMessage::Complete { result: Ok(value) }
        };
        Ok((next, message))
    })
    .await
    .map_err(|_| Error::Invocation)?
}
