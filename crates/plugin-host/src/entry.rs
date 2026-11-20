//! Machine-private launch description and the bounded wire exchanges of one
//! contributed harness entry: stdin invoke, stdout frames and the in-sandbox
//! broker stream at `/run/calternal/broker.sock`.
use std::sync::OnceLock;

use calternal_types::{AccountId, ErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    Error, bounded, registry_v2::catalogue_validator, space_channel::SpaceMethod, strict_json,
};

mod harness;
pub use harness::{DeclaredNetwork, HarnessDeclaration, harness_declaration};

/// Maximum encoded bytes in one contribution exchange, including the final LF.
pub const MAX_ENTRY_FRAME_BYTES: usize = 16 * 1024;

/// Path of the private broker stream inside the exact attempt sandbox, from
/// the catalogue's `harness_abi.broker.endpoint`.
pub const BROKER_ENDPOINT: &str = "/run/calternal/broker.sock";

/// Largest broker call or reply line, excluding its LF, from
/// `harness_abi.broker.max_frame_bytes_excluding_lf`.
pub const MAX_BROKER_FRAME_BYTES: usize = 262_144;

/// Callbacks one entry may have unanswered, from
/// `harness_abi.broker.max_outstanding_callbacks`.
pub const MAX_OUTSTANDING_CALLBACKS: usize = 128;

/// Longest run deadline the machine accepts from a host: seven days. The
/// host chooses the actual deadline; the machine enforces it locally and,
/// separately, stops a run after 24 hours of continuous server absence.
pub const MAX_RUN_LIMIT_SECS: u32 = 7 * 24 * 60 * 60;

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
    /// Host methods the contributing operation declared. The machine relay
    /// refuses every other callback before it reaches the carrier.
    pub methods: Vec<SpaceMethod>,
    /// Approved login directories to mount, or none.
    pub login: Option<EntryLogin>,
    /// Separately approved execution egress, independent of the worker.
    pub network: EntryNetwork,
    /// The run's own deadline, enforced by the machine without the server.
    pub run_limit_secs: u32,
}

/// One acting person's approved login directories for one harness. Each
/// directory maps to private backing storage for this package and
/// contribution only; nothing is shared between harnesses by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryLogin {
    /// Person whose personal backing storage is mounted. Space-scoped
    /// bindings are refused until their boundary is qualified.
    pub account: AccountId,
    /// Contributing package ID.
    pub package: String,
    /// Harness contribution ID within the package.
    pub contribution: String,
    /// Declared relative mount paths under the sandbox home.
    pub directories: Vec<String>,
}

/// Approved execution egress after the host resolved person-scoped servers.
/// Only HTTPS CONNECT to port 443 is ever relayed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EntryNetwork {
    /// No egress at all: the sandbox has no network and no proxy.
    None,
    /// Exactly these lowercase public hostnames.
    Sites {
        /// Approved hostnames, at most 128.
        hosts: Vec<String>,
    },
    /// Any public hostname, approved with the Owner's warning.
    AnyPublic,
}

impl EntryProgram {
    /// Check every field the machine relies on before staging anything:
    /// method count, login directory syntax and overlap, hostnames and the
    /// run deadline. Image, entry and invocation pins are checked at launch.
    /// Cost: linear in the declared lists, no I/O.
    pub fn check(&self) -> Result<(), Error> {
        if self.methods.len() > 64
            || self.run_limit_secs == 0
            || self.run_limit_secs > MAX_RUN_LIMIT_SECS
        {
            return Err(Error::Invalid);
        }
        if let Some(login) = &self.login {
            check_login(login)?;
        }
        if let EntryNetwork::Sites { hosts } = &self.network
            && (hosts.is_empty() || hosts.len() > 128 || !hosts.iter().all(|h| hostname(h)))
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }

    /// Whether the operation declared this callback method. Cost: at most 64
    /// string comparisons.
    pub fn declares(&self, method: &SpaceMethod) -> bool {
        self.methods.contains(method)
    }
}

fn check_login(login: &EntryLogin) -> Result<(), Error> {
    if !local_id(&login.package)
        || !local_id(&login.contribution)
        || login.directories.is_empty()
        || login.directories.len() > 8
        || !login.directories.iter().all(|path| login_path(path))
    {
        return Err(Error::Invalid);
    }
    for (index, path) in login.directories.iter().enumerate() {
        for other in &login.directories[index + 1..] {
            if nested(path, other) || nested(other, path) {
                return Err(Error::Invalid);
            }
        }
    }
    Ok(())
}

/// The manifest's `localId`: lowercase ASCII, starting with a letter,
/// single `-` or `_` separators, at most 64 bytes.
pub(crate) fn local_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-' || *b == b'_')
        && !id.contains("--")
        && !id.contains("__")
        && !id.contains("-_")
        && !id.contains("_-")
        && !id.ends_with(['-', '_'])
}

/// The manifest's `login_directories` item: relative components of
/// `[A-Za-z0-9._-]`, none of them `.` or `..`, at most 256 bytes.
pub fn login_path(path: &str) -> bool {
    path.len() <= 256
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
        })
}

fn nested(outer: &str, inner: &str) -> bool {
    inner == outer
        || inner
            .strip_prefix(outer)
            .is_some_and(|r| r.starts_with('/'))
}

/// A lowercase DNS hostname of at most 253 bytes with no IP literal, port
/// or trailing dot, as accepted for `fixed_sites`.
pub fn hostname(host: &str) -> bool {
    host.len() <= 253
        && host.contains('.')
        && host.parse::<std::net::IpAddr>().is_err()
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
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
    let value = line_json(line, MAX_ENTRY_FRAME_BYTES)?;
    if !SCHEMA
        .get_or_init(|| catalogue_validator("ContributionFrame"))
        .is_valid(&value)
    {
        return Err(Error::Invalid);
    }
    Ok(value)
}

/// One checked callback read from the entry's broker stream.
#[derive(Debug, PartialEq)]
pub struct BrokerCall {
    /// Entry-chosen ID (1 to 128 bytes) that pairs with exactly one reply.
    pub callback_id: String,
    /// Contract host method; the caller checks the operation declared it.
    pub method: SpaceMethod,
    /// Method input, validated later by the engine.
    pub input: Value,
}

/// Decode one broker line (JSON plus LF) as the catalogue's
/// `WorkerCallMessage`. Refuses extra members, such as forged call, attempt
/// or ticket metadata, duplicate keys, unknown methods and oversize lines.
/// Cost: one bounded parse and one cached validation.
pub fn decode_broker_call(line: &[u8]) -> Result<BrokerCall, Error> {
    static SCHEMA: OnceLock<jsonschema::Validator> = OnceLock::new();
    let value = line_json(line, MAX_BROKER_FRAME_BYTES + 1)?;
    if !SCHEMA
        .get_or_init(|| catalogue_validator("WorkerCallMessage"))
        .is_valid(&value)
    {
        return Err(Error::Invalid);
    }
    let Value::Object(mut fields) = value else {
        return Err(Error::Invalid);
    };
    let method = SpaceMethod::new(fields["method"].as_str().ok_or(Error::Invalid)?)?;
    let input = fields.remove("input").ok_or(Error::Invalid)?;
    let Some(Value::String(callback_id)) = fields.remove("callback_id") else {
        return Err(Error::Invalid);
    };
    Ok(BrokerCall {
        callback_id,
        method,
        input,
    })
}

/// Encode the catalogue's `HostReplyMessage` plus LF for one callback. A
/// reply larger than the broker frame bound becomes a `too_large` failure,
/// so the entry always gets exactly one line. Cost: one bounded encode.
pub fn encode_broker_reply(callback_id: &str, result: &Result<Value, ErrorCode>) -> Vec<u8> {
    let failure = |code: ErrorCode| {
        json!({"type":"reply","callback_id":callback_id,"ok":false,
               "error":{"status":code.http_status(),"code":code}})
    };
    let value = match result {
        Ok(data) => json!({"type":"reply","callback_id":callback_id,"ok":true,"data":data}),
        Err(code) => failure(*code),
    };
    let mut bytes = bounded::json(&value, MAX_BROKER_FRAME_BYTES).unwrap_or_else(|_| {
        serde_json::to_vec(&failure(ErrorCode::TooLarge)).expect("fixed reply encodes")
    });
    bytes.push(b'\n');
    bytes
}

fn line_json(line: &[u8], limit: usize) -> Result<Value, Error> {
    if line.len() > limit {
        return Err(Error::TooLarge);
    }
    let object = line.strip_suffix(b"\n").ok_or(Error::Invalid)?;
    if object.iter().any(|&byte| byte == b'\r' || byte == b'\n') {
        return Err(Error::Invalid);
    }
    strict_json::parse(object)
}
