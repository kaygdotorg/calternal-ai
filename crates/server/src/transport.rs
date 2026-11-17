//! Accepted HTTP sockets send small live-feed frames without Nagle batching.
use axum::serve::{ListenerExt, TapIo};
use tokio::net::{TcpListener, TcpStream};

/// Configure each accepted socket once, preserving the peer address used by
/// authentication/proxy checks. Cost: one nonblocking socket-option syscall per
/// connection. A refused option is logged; authorization and durability remain
/// enforced, while the performance gate still observes the resulting latency.
pub(super) fn listener(listener: TcpListener) -> TapIo<TcpListener, fn(&mut TcpStream)> {
    listener.tap_io(configure as fn(&mut TcpStream))
}
fn configure(stream: &mut TcpStream) {
    if let Err(error) = stream.set_nodelay(true) {
        tracing::warn!(%error, "cannot disable batching on live HTTP socket");
    }
}
