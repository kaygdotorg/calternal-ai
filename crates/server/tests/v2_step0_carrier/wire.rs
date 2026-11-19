//! Transport adapter used only by the enrolled-channel spike.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashSet,
    io::{self, BufRead, BufReader},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt},
    time::{Duration, timeout},
};
use uuid::Uuid;

pub(super) const CAP: usize = 256 * 1024;
pub(super) const DATA_CREDIT: usize = 2 * CAP;
pub(super) const CONTROL_CREDIT: usize = 4096;
pub(super) const WRITE_LIMIT: Duration = Duration::from_millis(300);
pub(super) const LEASE: Duration = Duration::from_secs(4);

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    pub v: u8,
    pub epoch: Uuid,
    pub seq: u64,
    pub package: String,
    pub digest: String,
    pub ticket: String,
    pub message: Message,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Message {
    Child { frame: Value },
    Credit { bytes: usize },
    Renew,
    Ready,
    Cancel,
    Fill { bytes: String },
}

pub(super) async fn line<R: AsyncRead + Unpin>(
    input: &mut tokio::io::BufReader<R>,
    bytes: &mut Vec<u8>,
) -> io::Result<()> {
    loop {
        let available = input.fill_buf().await?;
        if available.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let newline = available.iter().position(|&b| b == b'\n');
        let take = newline.map_or(available.len(), |at| at + 1);
        if bytes.len() + take > CAP + 1 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        bytes.extend_from_slice(&available[..take]);
        input.consume(take);
        if newline.is_some() {
            return Ok(());
        }
    }
}

pub(super) fn child_line<R: io::Read>(input: &mut BufReader<R>) -> io::Result<Vec<u8>> {
    let mut limited = io::Read::take(input, (CAP + 1) as u64);
    let mut bytes = Vec::new();
    limited.read_until(b'\n', &mut bytes)?;
    if bytes.last() != Some(&b'\n') {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(bytes)
}

pub(super) async fn send<W: AsyncWrite + Unpin>(writer: &mut W, bytes: &[u8]) -> io::Result<()> {
    timeout(WRITE_LIMIT, writer.write_all(bytes))
        .await
        .map_err(io::Error::other)?
}

pub(super) fn encode(value: &impl Serialize) -> Vec<u8> {
    let mut encoded = serde_json::to_vec(value).unwrap();
    assert!(encoded.len() <= CAP);
    encoded.push(b'\n');
    encoded
}

// Check duplicates and nesting before deserializing into maps. The final serde
// pass validates syntax, UTF-8, types and all envelope/message fields.
pub(super) fn strict(bytes: &[u8]) -> bool {
    fn quoted(bytes: &[u8], index: &mut usize) -> Option<String> {
        let start = *index;
        *index += 1;
        loop {
            match bytes.get(*index)? {
                b'\\' => *index += 2,
                b'"' => {
                    *index += 1;
                    return serde_json::from_slice(&bytes[start..*index]).ok();
                }
                _ => *index += 1,
            }
        }
    }
    let mut stack: Vec<Option<HashSet<String>>> = Vec::new();
    let mut index = 0;
    while let Some(&b) = bytes.get(index) {
        match b {
            b'{' => stack.push(Some(HashSet::new())),
            b'[' => stack.push(None),
            b']' | b'}' => {
                if stack.pop().is_none() {
                    return false;
                }
            }
            b'"' => {
                let Some(s) = quoted(bytes, &mut index) else {
                    return false;
                };
                let key = bytes[index..]
                    .iter()
                    .copied()
                    .find(|b| !b.is_ascii_whitespace())
                    == Some(b':');
                if key
                    && let Some(Some(keys)) = stack.last_mut()
                    && !keys.insert(s)
                {
                    return false;
                }
                continue;
            }
            _ => {}
        }
        if stack.len() > 32 {
            return false;
        }
        index += 1;
    }
    stack.is_empty()
}
