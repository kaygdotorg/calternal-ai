use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, bounded, registry_v2::catalogue_validator, strict_json};

/// Maximum encoded bytes in one contribution exchange, including the final LF.
pub const MAX_ENTRY_FRAME_BYTES: usize = 16 * 1024;

/// Machine-private launch material carried under the installed package and
/// exact attempt ticket in the authenticated carrier. It is never exposed
/// to the entry process as authority.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryProgram {
    /// Approved, normalized OCI repository, excluding tag and digest.
    pub image_source: String,
    /// Pinned OCI manifest or index SHA-256.
    pub image_digest: String,
    /// Separately pinned SHA-256 of the executable asset.
    pub entry_sha256: String,
    /// Verified package asset bytes in standard padded base64.
    pub entry_base64: String,
    /// Preflighted ContributionEntryInvoke envelope.
    pub invocation: Value,
}

/// Validate and encode a checked contribution invoke envelope for entry stdin.
/// Allocates at most one bounded line. The caller checks the admitted input and
/// attempt authority before sending it.
pub fn encode_invoke(value: &Value) -> Result<Vec<u8>, Error> {
    static SCHEMA: OnceLock<jsonschema::Validator> = OnceLock::new();
    let mut bytes = bounded::json(value, MAX_ENTRY_FRAME_BYTES - 1)?;
    if !SCHEMA
        .get_or_init(|| catalogue_validator("ContributionEntryInvoke"))
        .is_valid(value)
    {
        return Err(Error::Invalid);
    }
    bytes.push(b'\n');
    Ok(bytes)
}

/// Decode one physical stdout line and validate the catalogue's frame envelope.
/// Rejects duplicate members, invalid encoding, and oversize before delivery.
/// Costs one bounded parse and one cached validation. The receiver still
/// checks the output interface and this attempt's producer sequence and fence.
pub fn decode_frame(line: &[u8]) -> Result<Value, Error> {
    static SCHEMA: OnceLock<jsonschema::Validator> = OnceLock::new();
    if line.len() > MAX_ENTRY_FRAME_BYTES {
        return Err(Error::TooLarge);
    }
    let object = line.strip_suffix(b"\n").ok_or(Error::Invalid)?;
    if object.iter().any(|&byte| byte == b'\r' || byte == b'\n') {
        return Err(Error::Invalid);
    }
    let value = strict_json::parse(object)?;
    if !SCHEMA
        .get_or_init(|| catalogue_validator("ContributionFrame"))
        .is_valid(&value)
    {
        return Err(Error::Invalid);
    }
    Ok(value)
}
