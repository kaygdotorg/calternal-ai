//! Per-run transfer accounting and tunnel deadlines.

use std::{
    io,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf, copy_bidirectional},
    net::{TcpStream, UnixStream},
    sync::watch,
    time::{Instant, sleep_until, timeout},
};

#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub(super) concurrent: usize,
    pub(super) handshake: Duration,
    pub(super) idle: Duration,
    pub(super) lifetime: Duration,
    pub(super) bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            concurrent: 4,
            handshake: Duration::from_secs(5),
            idle: Duration::from_secs(30),
            lifetime: Duration::from_secs(180),
            bytes: 64 * 1024 * 1024,
        }
    }
}

pub(super) async fn relay(
    client: UnixStream,
    upstream: TcpStream,
    early: &[u8],
    budget: &AtomicU64,
    limits: Limits,
) {
    let (progress, mut last) = watch::channel(Instant::now());
    let mut client = Metered::new(client, budget, &progress);
    let mut upstream = Metered::new(upstream, budget, &progress);
    let transfer = async {
        upstream.write_all(early).await?;
        copy_bidirectional(&mut client, &mut upstream).await
    };
    let idle = async {
        loop {
            let deadline = *last.borrow_and_update() + limits.idle;
            tokio::select! {
                biased;
                _ = last.changed() => {}
                _ = sleep_until(deadline) => break,
            }
        }
    };
    tokio::select! {
        biased;
        _ = idle => {}
        _ = timeout(limits.lifetime, transfer) => {}
    }
}

// Reserve against both directions of all connections before attempting a write.
// Return unused capacity immediately, including after a pending write.
struct Metered<'a, S> {
    stream: S,
    budget: &'a AtomicU64,
    progress: &'a watch::Sender<Instant>,
}

impl<'a, S> Metered<'a, S> {
    fn new(stream: S, budget: &'a AtomicU64, progress: &'a watch::Sender<Instant>) -> Self {
        Self {
            stream,
            budget,
            progress,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Metered<'_, S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Metered<'_, S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        let cap = buf.len() as u64;
        let Ok(remaining) =
            self.budget
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                    (remaining > 0).then(|| remaining.saturating_sub(cap))
                })
        else {
            return Poll::Ready(Err(io::Error::other("run egress byte limit reached")));
        };
        let reserved = remaining.min(cap);
        let result = Pin::new(&mut self.stream).poll_write(cx, &buf[..reserved as usize]);
        let written = match result {
            Poll::Ready(Ok(written)) => written as u64,
            _ => 0,
        };
        self.budget.fetch_add(reserved - written, Ordering::Relaxed);
        if written > 0 {
            self.progress.send_replace(Instant::now());
        }
        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::AsyncReadExt, net::TcpListener};

    #[test]
    fn cancelling_metered_stream_does_not_run_a_log_destructor() {
        assert!(!std::mem::needs_drop::<Metered<'_, ()>>());
    }

    async fn connections() -> (UnixStream, UnixStream, TcpStream, TcpStream) {
        let (client, proxy) = UnixStream::pair().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (server, _) = listener.accept().await.unwrap();
        (client, proxy, upstream, server)
    }

    #[tokio::test]
    async fn transfer_budget_includes_early_bytes_and_is_shared_by_tunnels() {
        let budget = AtomicU64::new(8);
        let limits = Limits::default();
        let (mut client, proxy, upstream, mut server) = connections().await;
        let first = async {
            let mut early = [0; 3];
            server.read_exact(&mut early).await.unwrap();
            assert_eq!(&early, b"abc");
            server.write_all(b"de").await.unwrap();
            let mut response = [0; 2];
            client.read_exact(&mut response).await.unwrap();
            assert_eq!(&response, b"de");
            drop(client);
            drop(server);
        };
        tokio::join!(relay(proxy, upstream, b"abc", &budget, limits), first);
        assert_eq!(budget.load(Ordering::Relaxed), 3);

        let (_client, proxy, upstream, mut server) = connections().await;
        let second = async {
            let mut data = Vec::new();
            server.read_to_end(&mut data).await.unwrap();
            assert_eq!(data, b"123");
        };
        tokio::join!(relay(proxy, upstream, b"123456", &budget, limits), second);
        assert_eq!(budget.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn idle_and_lifetime_close_open_connections() {
        let short = Duration::from_millis(30);
        for limits in [
            Limits {
                idle: short,
                ..Limits::default()
            },
            Limits {
                lifetime: short,
                ..Limits::default()
            },
        ] {
            let (mut client, proxy, upstream, _server) = connections().await;
            let budget = AtomicU64::new(100);
            let started = Instant::now();
            timeout(
                Duration::from_secs(1),
                relay(proxy, upstream, &[], &budget, limits),
            )
            .await
            .expect("inactive peer should lose its tunnel");
            assert!(started.elapsed() >= short);
            assert_eq!(client.read(&mut [0]).await.unwrap(), 0);
        }
    }
}
