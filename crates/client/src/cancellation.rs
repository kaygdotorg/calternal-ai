//! Sticky local cancellation shared by transport and native operation adapters.
/// Cloneable, one-way local cancellation signal. Cancellation remains observable by
/// future waiters and every clone; a fresh operation needs a fresh token.
/// Holds one bounded watch value; no task, queue growth or persistent I/O.
#[derive(Clone)]
pub struct CancellationToken {
    signal: tokio::sync::watch::Sender<bool>,
}
impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}
impl CancellationToken {
    /// Allocate one watch channel without spawning tasks or I/O; constant cost.
    pub fn new() -> Self {
        Self {
            signal: tokio::sync::watch::channel(false).0,
        }
    }
    /// Cancel all waits using this token; updates one synchronized watch value and wakes
    /// current waiters, O(waiters). No network, persistent I/O or server rollback.
    pub fn cancel(&self) {
        self.signal.send_replace(true);
    }
    pub(crate) async fn cancelled(&self) {
        let mut receiver = self.signal.subscribe();
        loop {
            if *receiver.borrow_and_update() {
                return;
            }
            if receiver.changed().await.is_err() {
                return;
            }
        }
    }
}
