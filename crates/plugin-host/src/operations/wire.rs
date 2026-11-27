//! Bounded newline JSON frames for the owned child pipes.
use std::io;
use std::str::FromStr;
use std::sync::LazyLock;
use tokio::sync::Semaphore;

use calternal_types::{Error, ErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use uuid::Uuid;

use super::Delivery;

const VERSION: u8 = 1;
// A 32 MiB read can expand when serialized as JSON. The route separately
// limits the underlying item and write sizes.
pub(super) const MAX_FRAME: usize = 64 * 1024 * 1024;

/// The ten on-wire format §8.1 classifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Code {
    /// The current authority or declaration excludes this effect.
    NotAllowed,
    /// The addressed item does not exist or cannot be disclosed.
    NotFound,
    /// The input or frame is invalid.
    Invalid,
    /// The request conflicts with current state or a receipt.
    Conflict,
    /// The value exceeds its byte budget.
    TooLarge,
    /// Admission or another bounded resource is exhausted.
    Limit,
    /// A required worker or service is unavailable.
    Unreachable,
    /// The operation requires a sign-in.
    NeedsSignIn,
    /// The worker or run was stopped.
    Stopped,
    /// The event or effect exceeded its deadline.
    Timeout,
}

impl Code {
    /// Build a shared caller-safe error that retains this pipe's classification
    /// and next action. Cost is proportional to the two diagnostic strings.
    pub fn error(self, message: impl Into<String>, next: impl Into<String>) -> Error {
        let category = match self {
            Self::NotAllowed => ErrorCode::Forbidden,
            Self::NotFound => ErrorCode::NotFound,
            Self::Invalid => ErrorCode::InvalidInput,
            Self::Conflict => ErrorCode::Conflict,
            Self::TooLarge => ErrorCode::TooLarge,
            Self::Limit => ErrorCode::RetryAfter,
            Self::NeedsSignIn => ErrorCode::Unauthorized,
            Self::Unreachable | Self::Stopped | Self::Timeout => ErrorCode::Unavailable,
        };
        Error::new(category, message).with_details(serde_json::json!({
            "operation_code": self, "next": next.into()
        }))
    }

    /// Recover the format classification, mapping ordinary shared errors when
    /// no format classification was attached. Cost is one small lookup.
    pub fn from_error(error: &Error) -> Self {
        if let Some(code) = error.details.as_ref().and_then(|v| v.get("operation_code"))
            && let Some(code) = code.as_str().and_then(|s| Self::from_str(s).ok())
        {
            return code;
        }
        match error.code {
            ErrorCode::Forbidden => Self::NotAllowed,
            ErrorCode::Unauthorized | ErrorCode::PasskeyReverificationRequired => Self::NeedsSignIn,
            ErrorCode::NotFound => Self::NotFound,
            ErrorCode::Conflict => Self::Conflict,
            ErrorCode::InvalidInput => Self::Invalid,
            ErrorCode::TooLarge => Self::TooLarge,
            ErrorCode::EffectiveScopeLimitExceeded | ErrorCode::RetryAfter => Self::Limit,
            ErrorCode::Internal | ErrorCode::Unavailable => Self::Unreachable,
        }
    }
}

impl FromStr for Code {
    type Err = serde::de::value::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::deserialize(serde::de::value::StrDeserializer::new(value))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireError<'a> {
    code: Code,
    message: std::borrow::Cow<'a, str>,
    next: std::borrow::Cow<'a, str>,
}

impl<'a> From<&'a Error> for WireError<'a> {
    fn from(error: &'a Error) -> Self {
        Self {
            code: Code::from_error(error),
            message: error.message.as_str().into(),
            next: error
                .details
                .as_ref()
                .and_then(|v| v.get("next"))
                .and_then(Value::as_str)
                .unwrap_or("Try again or check the operation input.")
                .into(),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Outcome {
    Success(Success),
    Failure(Failure),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Success {
    result: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    error: WireError<'static>,
}

impl Outcome {
    fn into_result(self) -> Result<Value, Error> {
        match self {
            Self::Success(value) => Ok(value.result),
            Self::Failure(value) => Err(value
                .error
                .code
                .error(value.error.message, value.error.next)),
        }
    }
}

/// Route name accepted from the worker. There are no dynamic transport methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Method {
    /// Query rows through the host.
    #[serde(rename = "s.query")]
    Query,
    /// Read an item through the host.
    #[serde(rename = "s.read")]
    Read,
    /// Commit or preview an ordered list atomically through the host.
    #[serde(rename = "s.write")]
    Write,
    /// Get or set viewer state through the host.
    #[serde(rename = "s.state")]
    State,
    /// Invoke a declared operation through the host.
    #[serde(rename = "s.call")]
    Call,
    /// Start a run through the host.
    #[serde(rename = "s.run")]
    Run,
    /// Send input to a run through the host.
    #[serde(rename = "s.run.send")]
    RunSend,
    /// Stop a run through the host.
    #[serde(rename = "s.run.stop")]
    RunStop,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Incoming {
    Call(CallFrame),
    Complete(CompleteFrame),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CallFrame {
    v: u8,
    invocation: Uuid,
    id: u32,
    call: Method,
    input: Value,
}

#[derive(Deserialize)]
struct CompleteFrame {
    v: u8,
    invocation: Uuid,
    #[serde(flatten)]
    outcome: Outcome,
}

pub(super) enum Message {
    Call {
        id: u32,
        method: Method,
        input: Value,
    },
    Complete(Result<Value, Error>),
}

pub(super) async fn receive(
    reader: &mut BufReader<impl AsyncRead + Unpin>,
    invocation: Uuid,
    next_id: u32,
    remaining: usize,
) -> Result<(Message, usize), Error> {
    let mut bytes = Vec::new();
    loop {
        let chunk = reader.fill_buf().await.map_err(|_| disconnected())?;
        if chunk.is_empty() {
            return Err(disconnected());
        }
        let ending = chunk.iter().position(|&byte| byte == b'\n');
        let size = ending.unwrap_or(chunk.len());
        if size > MAX_FRAME.min(remaining).saturating_sub(bytes.len()) {
            return Err(oversized());
        }
        bytes.extend_from_slice(&chunk[..size]);
        reader.consume(size + usize::from(ending.is_some()));
        if ending.is_some() {
            break;
        }
    }
    let length = bytes.len();
    Ok((
        codec(move || decode(bytes, invocation, next_id)).await?,
        length,
    ))
}

pub(super) async fn codec<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    static ADMISSION: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(8));
    let permit = ADMISSION
        .try_acquire()
        .map_err(|_| Code::Limit.error("The plugin codec is busy.", "Retry the operation."))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work()
    })
    .await
    .map_err(|_| disconnected())?
}

fn decode(bytes: Vec<u8>, invocation: Uuid, next_id: u32) -> Result<Message, Error> {
    match crate::strict_json::parse(&bytes).map_err(|_| invalid())? {
        Incoming::Call(frame)
            if frame.v == VERSION && frame.invocation == invocation && frame.id == next_id =>
        {
            Ok(Message::Call {
                id: frame.id,
                method: frame.call,
                input: frame.input,
            })
        }
        Incoming::Complete(frame) if frame.v == VERSION && frame.invocation == invocation => {
            Ok(Message::Complete(frame.outcome.into_result()))
        }
        _ => Err(invalid()),
    }
}

#[derive(Serialize)]
struct Envelope<T> {
    v: u8,
    invocation: Uuid,
    #[serde(flatten)]
    body: T,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Outgoing<'a> {
    Invoke {
        selector: &'a str,
        input: &'a Value,
        me: &'a super::Person,
        space: Space,
        event: &'a super::Event,
    },
    Reply {
        id: u32,
        #[serde(flatten)]
        outcome: Reply<'a>,
    },
}

#[derive(Serialize)]
struct Space {
    id: calternal_types::SpaceId,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Reply<'a> {
    Success { result: &'a Value },
    Failure { error: WireError<'a> },
}

pub(super) fn invoke(invocation: Uuid, delivery: &Delivery) -> Result<Vec<u8>, Error> {
    encode(
        invocation,
        Outgoing::Invoke {
            selector: &delivery.selector,
            input: &delivery.input,
            me: &delivery.me,
            space: Space { id: delivery.space },
            event: &delivery.event,
        },
        MAX_FRAME,
    )
}

pub(super) fn reply(
    invocation: Uuid,
    id: u32,
    result: &Result<Value, Error>,
    remaining: usize,
) -> Result<Vec<u8>, Error> {
    let outcome = match result {
        Ok(result) => Reply::Success { result },
        Err(error) => Reply::Failure {
            error: error.into(),
        },
    };
    encode(
        invocation,
        Outgoing::Reply { id, outcome },
        MAX_FRAME.min(remaining),
    )
}

fn encode(invocation: Uuid, body: Outgoing<'_>, maximum: usize) -> Result<Vec<u8>, Error> {
    let mut bytes = crate::bounded::json(
        &Envelope {
            v: VERSION,
            invocation,
            body,
        },
        maximum,
    )
    .map_err(|_| oversized())?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) async fn send(
    output: &mut (impl AsyncWriteExt + Unpin),
    bytes: &[u8],
) -> Result<(), Error> {
    output
        .write_all(bytes)
        .await
        .map_err(|_: io::Error| disconnected())
}

pub(super) fn disconnected() -> Error {
    Code::Unreachable.error(
        "The plugin worker exited or could not start.",
        "Retry the operation.",
    )
}

fn invalid() -> Error {
    Code::Invalid.error(
        "The plugin worker sent an invalid frame.",
        "Update the plugin.",
    )
}

pub(super) fn oversized() -> Error {
    Code::TooLarge.error(
        "The plugin frame is too large.",
        "Reduce the operation input or result.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_ambiguous_or_stale_worker_frames() {
        let invocation = Uuid::new_v4();
        for body in [
            r#""type":"complete","result":null,"result":null"#,
            r#""type":"complete","result":null,"error":{"code":"invalid","message":"m","next":"n"}"#,
            r#""type":"complete","result":null,"identity":"forged""#,
            r#""type":"reply","id":1,"result":null"#,
            r#""type":"call","id":2,"call":"s.read","input":{}"#,
            r#""type":"call","id":1,"call":"s.fetch","input":{}"#,
        ] {
            let line = format!("{{\"v\":1,\"invocation\":\"{invocation}\",{body}}}\n");
            let mut input = BufReader::new(line.as_bytes());
            let error = receive(&mut input, invocation, 1, MAX_FRAME)
                .await
                .err()
                .expect("bad frame");
            assert_eq!(Code::from_error(&error), Code::Invalid);
        }
        let stale = Uuid::new_v4();
        let line = format!(
            "{{\"v\":1,\"invocation\":\"{stale}\",\"type\":\"complete\",\"result\":null}}\n"
        );
        let mut input = BufReader::new(line.as_bytes());
        assert!(receive(&mut input, invocation, 1, MAX_FRAME).await.is_err());
    }
}
