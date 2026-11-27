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
    codecs: Arc<tokio::sync::Semaphore>,
    failed: bool,
}

impl LiveSocket {
    pub(super) fn new(
        raw: WebSocket,
        host: Arc<Host>,
        session: Session,
        codecs: Arc<tokio::sync::Semaphore>,
    ) -> Self {
        Self {
            raw,
            host,
            session,
            codecs,
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

pub(super) const MAX_OUTGOING_BYTES: usize = 2 * 1024 * 1024;

async fn encode(
    codecs: Arc<tokio::sync::Semaphore>,
    frame: ServerFrame,
    distinct: bool,
) -> Result<(String, Option<blake3::Hash>), Error> {
    let permit = codecs
        .try_acquire_owned()
        .map_err(|_| Error::new(ErrorCode::RetryAfter, "Live frame encoding is busy."))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let encoded = calternal_plugin_host::bounded_json(&frame, MAX_OUTGOING_BYTES)
            .map_err(|_| Error::new(ErrorCode::TooLarge, "Live result exceeds 2 MiB."))?;
        let digest = distinct.then(|| blake3::hash(&encoded));
        let text = String::from_utf8(encoded).map_err(crate::internal)?;
        Ok((text, digest))
    })
    .await
    .map_err(crate::internal)?
}

async fn send_encoded(socket: &mut LiveSocket, text: String) -> Result<(), Error> {
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|_| Error::unavailable("The live socket closed."))
}

pub(super) async fn send_frame(socket: &mut LiveSocket, frame: ServerFrame) -> Result<(), Error> {
    let (text, _) = encode(socket.codecs.clone(), frame, false).await?;
    send_encoded(socket, text).await
}

pub(super) async fn send_distinct(
    socket: &mut LiveSocket,
    frame: ServerFrame,
    last: &mut Option<blake3::Hash>,
) -> Result<(), Error> {
    let (text, digest) = encode(socket.codecs.clone(), frame, true).await?;
    if *last != digest {
        send_encoded(socket, text).await?;
        *last = digest;
    }
    Ok(())
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
        ServerFrame::Error {
            sub,
            code,
            message: error.message.clone(),
            next,
        },
    )
    .await
    .map_err(|_| ())
}
