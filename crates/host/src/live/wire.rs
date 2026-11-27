//! Frame serialization and caller-safe subscription failures.

use super::ServerFrame;
use axum::extract::ws::{Message, WebSocket};
use calternal_types::{Error, ErrorCode};

pub(super) async fn send_frame(
    socket: &mut WebSocket,
    frame: &ServerFrame,
) -> std::result::Result<(), ()> {
    let Ok(text) = serde_json::to_string(frame) else {
        return Err(());
    };
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|_| ())
}

pub(super) async fn send_error(
    socket: &mut WebSocket,
    sub: Option<String>,
    error: &Error,
) -> std::result::Result<(), ()> {
    let (code, next) = match error.code {
        ErrorCode::Unauthorized => ("needs-sign-in", "Sign in and try again."),
        ErrorCode::Forbidden | ErrorCode::PasskeyReverificationRequired => {
            ("not-allowed", "Check your access.")
        }
        ErrorCode::NotFound => ("not-found", "Check the requested ID."),
        ErrorCode::Conflict => ("conflict", "Refresh before retrying."),
        ErrorCode::InvalidInput => ("invalid", "Check the subscription arguments."),
        ErrorCode::TooLarge => ("too-large", "Reduce the frame size."),
        ErrorCode::EffectiveScopeLimitExceeded | ErrorCode::RetryAfter => {
            ("limit", "Reduce active subscriptions and try again.")
        }
        ErrorCode::Internal | ErrorCode::Unavailable => ("unreachable", "Try again later."),
    };
    send_frame(
        socket,
        &ServerFrame::Error {
            sub,
            code,
            message: error.message.clone(),
            next,
        },
    )
    .await
}
