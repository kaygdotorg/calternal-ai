//! Check authority after socket backpressure clears, before handing off data.

use futures_util::Sink;
use std::{
    future::poll_fn,
    pin::Pin,
    task::{Poll, ready},
};

/// Leave the frame queued if its authority expired while waiting. A successful
/// decision admits it to the socket and submits it in the same poll.
pub(super) async fn stage<S, T>(
    socket: &mut S,
    frame: &mut Option<T>,
    authorized: impl Fn() -> bool,
) -> Result<bool, S::Error>
where
    S: Sink<T> + Unpin,
{
    poll_fn(|cx| {
        let mut socket = Pin::new(&mut *socket);
        ready!(socket.as_mut().poll_ready(cx))?;
        if !authorized() {
            return Poll::Ready(Ok(false));
        }
        socket.start_send(frame.take().expect("frame staged only once"))?;
        Poll::Ready(Ok(true))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        convert::Infallible,
        future::Future,
        task::{Context, Waker},
    };

    struct Socket<'a> {
        ready: &'a Cell<bool>,
        staged: Vec<&'static str>,
    }

    impl Sink<&'static str> for Socket<'_> {
        type Error = Infallible;

        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            if self.ready.get() {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }

        fn start_send(mut self: Pin<&mut Self>, frame: &'static str) -> Result<(), Self::Error> {
            self.staged.push(frame);
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn revocation_drops_queued_data_after_socket_backpressure() {
        let writable = Cell::new(true);
        let authority_epoch = Cell::new(0);
        let has_view = || authority_epoch.get() == 0;
        let mut socket = Socket {
            ready: &writable,
            staged: Vec::new(),
        };
        let mut first = Some("already handed off");
        assert!(stage(&mut socket, &mut first, has_view).await.unwrap());

        writable.set(false);
        let mut queued = Some("accepted update awaiting fanout");
        {
            let send = stage(&mut socket, &mut queued, has_view);
            let mut send = std::pin::pin!(send);
            let mut cx = Context::from_waker(Waker::noop());
            assert_eq!(send.as_mut().poll(&mut cx), Poll::Pending);
            // Commit the revoke while the destination is blocked.
            authority_epoch.set(2);
            writable.set(true);
            assert_eq!(send.poll(&mut cx), Poll::Ready(Ok(false)));
        }
        assert_eq!(socket.staged, ["already handed off"]);
        let mut peer = Socket {
            ready: &writable,
            staged: Vec::new(),
        };
        assert!(stage(&mut peer, &mut queued, || true).await.unwrap());
        assert_eq!(peer.staged, ["accepted update awaiting fanout"]);
    }
}
