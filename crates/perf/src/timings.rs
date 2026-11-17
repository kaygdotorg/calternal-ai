//! Read debug-only stage markers separately from production latency evidence.
use crate::http::Result;
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

/// Parse bounded arm logs once and correlate stages with observed request IDs.
/// Plain releases have no markers. Malformed markers or reversed clocks fail the
/// arm; allocations scale with the harness-bounded log and sample count.
pub(crate) fn collect(
    log: &Path,
    deliveries: &BTreeMap<String, i64>,
    samples: &mut BTreeMap<String, Vec<f64>>,
) -> Result<()> {
    for line in std::fs::read_to_string(log)?.lines() {
        let Some(json) = line.strip_prefix("CALTERNAL_STAGE ") else {
            continue;
        };
        let stage: Value = serde_json::from_str(json)?;
        let Some(request) = stage["request_id"]
            .as_str()
            .filter(|r| deliveries.contains_key(*r))
        else {
            continue;
        };
        let metric = match stage["stage"].as_str() {
            Some("admission") => "admission",
            Some("receipt_lookup") => "receipt.lookup",
            Some("journal_committed") => {
                let committed = stage["at_ms"].as_i64().ok_or("missing commit clock")?;
                let elapsed = deliveries[request] - committed;
                if elapsed < 0 {
                    return Err("commit/delivery clock moved backward".into());
                }
                samples
                    .entry("live.commit_to_client".into())
                    .or_default()
                    .push(elapsed as f64);
                continue;
            }
            _ => continue,
        };
        let elapsed = stage["elapsed_us"]
            .as_u64()
            .ok_or("missing stage duration")?;
        samples
            .entry(metric.into())
            .or_default()
            .push(elapsed as f64 / 1000.);
    }
    Ok(())
}
