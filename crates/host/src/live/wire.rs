//! Frame serialization and caller-safe subscription failures.

use super::ServerFrame;
use crate::{Host, Session};
use axum::extract::ws::{Message, WebSocket};
use calternal_types::{Error, ErrorCode};
use std::{sync::Arc, time::Duration};

pub(super) struct LiveSocket {
    raw: WebSocket,
    host: Arc<Host>,
    session: Session,
    failed: bool,
}

impl LiveSocket {
    pub(super) fn new(raw: WebSocket, host: Arc<Host>, session: Session) -> Self {
        Self {
            raw,
            host,
            session,
            failed: false,
        }
    }

    pub(super) async fn recv(&mut self) -> Option<Result<Message, axum::Error>> {
        self.raw.recv().await
    }

    pub(super) async fn check(&mut self) -> Result<(), ()> {
        self.host
            .admit_session(Some(&self.session))
            .await
            .map(|_| ())
            .map_err(|_| ())
    }

    pub(super) async fn send(&mut self, message: Message) -> Result<(), ()> {
        if self.failed {
            return Err(());
        }
        let result = async {
            let _admission = self
                .host
                .admit_session(Some(&self.session))
                .await
                .map_err(|_| ())?;
            // Bound the time a pending send can delay this installation's logout.
            tokio::time::timeout(Duration::from_secs(2), self.raw.send(message))
                .await
                .map_err(|_| ())?
                .map_err(|_| ())
        }
        .await;
        self.failed = result.is_err();
        result
    }
}

pub(super) async fn send_frame(
    socket: &mut LiveSocket,
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
    socket: &mut LiveSocket,
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
