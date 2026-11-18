//! Correlate opt-in stage markers with observed client deliveries.
use crate::http::Result;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// Hard release delivery metric in the budget catalog.
pub(crate) const COMMIT_TO_CLIENT: &str = "live.commit_to_client";

/// Read a bounded arm log and match each durable commit to its received event.
/// Required release runs reject missing, duplicate or reversed observations.
/// Debug markers can only populate their diagnostic metric.
pub(crate) fn collect(
    log: &Path,
    deliveries: &BTreeMap<String, u64>,
    samples: &mut BTreeMap<String, Vec<f64>>,
    require_release: bool,
) -> Result<()> {
    let mut committed = BTreeSet::new();
    let mut release_count = 0;
    for line in std::fs::read_to_string(log)?.lines() {
        let Some(json) = line.strip_prefix("CALTERNAL_STAGE ") else {
            continue;
        };
        let stage: Value = serde_json::from_str(json)?;
        let Some((request, &received_ns)) = stage["request_id"]
            .as_str()
            .and_then(|id| deliveries.get_key_value(id))
        else {
            continue;
        };
        let metric = match stage["stage"].as_str() {
            Some("admission") => "admission",
            Some("receipt_lookup") => "receipt.lookup",
            Some("journal_committed") => {
                let at_ns = stage["at_ns"].as_u64().ok_or("missing commit clock")?;
                let elapsed = received_ns
                    .checked_sub(at_ns)
                    .ok_or("commit clock follows client receipt")?;
                if !committed.insert(request) {
                    return Err(format!("duplicate durable commit marker for {request}").into());
                }
                let release = stage["release"].as_bool().ok_or("missing build mode")?;
                let metric = if release {
                    release_count += 1;
                    COMMIT_TO_CLIENT
                } else {
                    "live.commit_to_client.debug"
                };
                samples
                    .entry(metric.into())
                    .or_default()
                    .push(elapsed as f64 / 1_000_000.);
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
    if require_release && release_count != deliveries.len() {
        return Err(format!(
            "expected {} release commit markers, observed {release_count}",
            deliveries.len()
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn a_release_gate_requires_one_precise_marker_per_delivery() {
        let deliveries = BTreeMap::from([("save".into(), 1_000_247)]);
        let marker = |release| {
            format!(
                r#"CALTERNAL_STAGE {{"request_id":"save","stage":"journal_committed","at_ns":1000000,"release":{release}}}"#
            )
        };
        for (markers, passes) in [
            (vec![], false),
            (vec![marker(false)], false),
            (vec![marker(true), marker(true)], false),
            (vec![marker(true)], true),
        ] {
            let mut log = tempfile::NamedTempFile::new().unwrap();
            for line in markers {
                writeln!(log, "{line}").unwrap();
            }
            let mut samples = BTreeMap::new();
            assert_eq!(
                collect(log.path(), &deliveries, &mut samples, true).is_ok(),
                passes
            );
            if passes {
                assert_eq!(samples[COMMIT_TO_CLIENT], [0.000247]);
            }
        }
    }
}
