//! Evidence supplied by the local operator's trusted release gate.
use serde::Deserialize;
use std::{
    io::{self, Read},
    path::Path,
};

const FORMAT: &str = "calternal-upgrade-proof/v1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proof {
    format: String,
    previous: String,
    candidate: String,
}

/// Both images are pinned to their local content addresses.
pub(super) fn check_release_ids(previous: &str, candidate: &str) -> io::Result<()> {
    if previous == candidate
        || [previous, candidate]
            .iter()
            .any(|id| id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(io::Error::other("use two different image digests"));
    }
    Ok(())
}

pub(super) fn require(path: Option<&Path>, previous: &str, candidate: &str) -> io::Result<()> {
    let path = path.ok_or_else(|| {
        io::Error::other(
            "this deployment needs compatibility evidence for the previous and candidate images",
        )
    })?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(4097)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(io::Error::other(
            "compatibility evidence exceeds its size limit",
        ));
    }
    let proof: Proof = serde_json::from_slice(&bytes)?;
    calternal_fsio::format::require(&proof.format, FORMAT)?;
    if proof.previous != previous || proof.candidate != candidate {
        return Err(io::Error::other(
            "compatibility evidence is for different images",
        ));
    }
    Ok(())
}
