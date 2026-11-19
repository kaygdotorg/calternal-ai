//! Per-statement timings on writer connections in disposable benchmarks.
use calternal_types::perf_monotonic_ns as now;
use rusqlite::{
    Connection,
    trace::{TraceEvent, TraceEventCodes},
};
use std::sync::OnceLock;

/// Enable statement profiling only in a diagnostic build with
/// `CALTERNAL_PROFILE_SQL=1`. Profiling emits unexpanded SQL and monotonic
/// timings, with no bound parameters. Keep it disabled for latency gates.
pub fn with_sql_profile(mut conn: Connection, store: &'static str) -> Connection {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if !*ENABLED.get_or_init(|| std::env::var_os("CALTERNAL_PROFILE_SQL").is_some_and(|v| v == "1"))
    {
        return conn;
    }
    let mut started = None;
    let event = move |event: TraceEvent<'_>| match event {
        TraceEvent::Stmt(_, sql) if !sql.starts_with("--") => started = Some(now()),
        TraceEvent::Profile(stmt, _) => {
            let ended = now();
            if let Some(started) = started.take() {
                eprintln!(
                    "CALTERNAL_SQL {}",
                    serde_json::json!({
                        "store":store,"sql":stmt.sql(),"start_ns":started,"end_ns":ended,
                    })
                );
            }
        }
        _ => {}
    };
    if let Err(error) = conn.trace_v2(
        TraceEventCodes::SQLITE_TRACE_STMT | TraceEventCodes::SQLITE_TRACE_PROFILE,
        Some(event),
    ) {
        eprintln!("SQLite profile setup failed for {store}: {error}");
    }
    conn
}
