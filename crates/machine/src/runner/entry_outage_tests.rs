//! The machine's own clocks for an enrolled entry, on tokio's paused clock:
//! the 24-hour unreachable-server cutoff and the run deadline.
use super::entry_channel::{OUTAGE_LIMIT, Run, drive_entry};
use crate::sandbox::{Exec, ExecRequest, Sandbox, SandboxError, SandboxResult};
use calternal_plugin_host::{Error as ProtocolError, space_channel::PackagePin};
use futures_util::future::BoxFuture;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

struct Probe(Arc<AtomicBool>);

impl Sandbox for Probe {
    fn exec(&self, _: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>> {
        Box::pin(async { Err(SandboxError("unused".into())) })
    }
    fn freeze(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.0.store(true, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
    fn release(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async { Ok(()) })
    }
}

/// Drive a detached entry whose producer never finishes and report when
/// the sandbox was stopped, checked one second either side of `expected`.
async fn stopped_at(run_limit: Duration, expected: Duration) {
    let package = PackagePin {
        package: "fixture".into(),
        digest: "0".repeat(64),
    };
    let stopped = Arc::new(AtomicBool::new(false));
    let (_publish, published) = watch::channel(0);
    let mut run = Run {
        package: &package,
        ticket: Uuid::new_v4(),
        reader: Arc::new(tempfile::tempfile().unwrap()),
        producer: tokio::spawn(std::future::pending::<Result<(), ProtocolError>>()),
        published,
        sandbox: Some(Arc::new(Probe(stopped.clone()))),
        broker: None,
    };
    let (_attach, replacements) = mpsc::channel(1);
    let deadline = tokio::time::sleep(run_limit);
    tokio::pin!(deadline);
    let start = tokio::time::Instant::now();
    {
        let drive = drive_entry(&mut run, None, replacements, deadline);
        tokio::pin!(drive);
        tokio::select! {
            _ = &mut drive => panic!("a detached entry with unsent output kept its log"),
            _ = tokio::time::sleep(expected - Duration::from_secs(1)) => {}
        }
        assert!(!stopped.load(Ordering::SeqCst), "stopped early");
        tokio::select! {
            _ = &mut drive => panic!("the relay must keep the log after stopping"),
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
        }
        assert!(
            stopped.load(Ordering::SeqCst),
            "not stopped by {expected:?}"
        );
    }
    assert!(start.elapsed() < expected + Duration::from_secs(2));
    assert!(run.sandbox.is_none());
}

#[tokio::test(start_paused = true)]
async fn unreachable_server_stops_the_entry_after_24_hours() {
    stopped_at(OUTAGE_LIMIT * 2, OUTAGE_LIMIT).await;
}

#[tokio::test(start_paused = true)]
async fn the_run_deadline_applies_during_an_outage() {
    stopped_at(Duration::from_secs(3600), Duration::from_secs(3600)).await;
}
