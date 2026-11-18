//! Stop admitting HTTP work and close idle streams while admitted requests drain.

use axum::{
    Extension, Router,
    body::Body,
    extract::{Request, State},
    http::{HeaderValue, header},
    middleware::{self, Next},
    response::Response,
};
use calternal_client::CancellationToken;
use futures_util::StreamExt;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// One-way shutdown for a serving process. HTTP admission costs one
/// atomic load; idle streams wait on the cancellation signal.
#[derive(Clone, Default)]
pub struct Shutdown(Arc<Inner>);

#[derive(Default)]
struct Inner {
    closing: AtomicBool,
    stop: CancellationToken,
    rooms: Arc<()>,
}

impl Shutdown {
    /// Attach the admission gate outside the router's auth and writer middleware.
    pub fn attach(&self, router: Router) -> Router {
        router
            .layer(Extension(self.clone()))
            .layer(middleware::from_fn_with_state(self.clone(), admit))
    }

    /// Refuse future requests and wake existing waits. Handlers that were
    /// already admitted can finish and publish their ordinary durable receipts.
    pub fn begin(&self) {
        self.0.closing.store(true, Ordering::Release);
        self.0.stop.cancel();
    }

    /// Wait for the process to stop accepting requests.
    pub async fn cancelled(&self) {
        self.0.stop.cancelled().await;
    }

    pub(crate) fn is_closing(&self) -> bool {
        self.0.closing.load(Ordering::Acquire)
    }

    pub(crate) fn room(&self) -> Arc<()> {
        self.0.rooms.clone()
    }

    /// After HTTP stops, wait for detached room connections that were already
    /// admitted. Their current frame finishes through its normal write fence.
    pub async fn drain_rooms(&self) {
        while Arc::strong_count(&self.0.rooms) > 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

pub(crate) async fn wait(shutdown: Option<&Shutdown>) {
    match shutdown {
        Some(shutdown) => shutdown.cancelled().await,
        None => std::future::pending().await,
    }
}

async fn admit(State(shutdown): State<Shutdown>, request: Request, next: Next) -> Response {
    if shutdown.is_closing() {
        let mut response = crate::api::error_response(calternal_types::Error::unavailable(
            "The server is restarting. Retry this request shortly.",
        ));
        response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
        return response;
    }
    let response = next.run(request).await;
    if !response
        .headers()
        .get(header::CONTENT_TYPE)
        .is_some_and(|value| value == "text/event-stream")
    {
        return response;
    }

    let (parts, body) = response.into_parts();
    let stream = body
        .into_data_stream()
        .take_until(async move { shutdown.cancelled().await });
    Response::from_parts(parts, Body::from_stream(stream))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::StatusCode, routing::post};
    use tokio::sync::Notify;
    use tower::ServiceExt;

    #[tokio::test]
    async fn refuses_new_writes_and_waits_for_the_admitted_response() {
        let entered = Arc::new(Notify::new());
        let finish = Arc::new(Notify::new());
        let shutdown = Shutdown::default();
        let router = shutdown.attach(Router::new().route(
            "/write",
            post({
                let entered = entered.clone();
                let finish = finish.clone();
                move || async move {
                    entered.notify_one();
                    finish.notified().await;
                    StatusCode::NO_CONTENT
                }
            }),
        ));
        let write = || Request::post("/write").body(Body::empty()).unwrap();
        let pending = tokio::spawn(router.clone().oneshot(write()));
        entered.notified().await;
        shutdown.begin();
        let refused = router.oneshot(write()).await.unwrap();
        assert_eq!(refused.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(!pending.is_finished());
        finish.notify_one();
        assert_eq!(pending.await.unwrap().unwrap().status(), StatusCode::NO_CONTENT);
    }
}
