//! Shared CLI formatting for evidence; rendering runs only after timed samples finish.
use crate::Args;
use calternal_cli::output::Reply;
use serde_json::Value;
use std::path::Path;

/// Print the same report facts as JSON or width-aware fields and metric tables.
/// Rendering allocates in proportion to the bounded report, outside timed sections.
pub(crate) fn print(a: &Args, report: &Value, path: &Path) {
    if a.json {
        println!("{report}");
        return;
    }
    let reply = human_reply(report, path);
    print!("{}", a.output.render(&reply));
}
fn human_reply(report: &Value, path: &Path) -> Reply {
    let mut reply = Reply::new(report.clone())
        .line("Times: ms; memory: MiB.")
        .fields([("report", path.display().to_string())]);
    if let Some(object) = report.as_object() {
        let fields = object
            .iter()
            .filter(|(_, v)| !v.is_array() && !v.is_object())
            .map(|(k, v)| {
                (
                    k.replace('_', " "),
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string()),
                )
            });
        reply = reply.fields(fields);
    }
    if let Some(runs) = report["runs"].as_array() {
        for run in runs {
            let fields = [
                ("clients", "clients"),
                ("mode", "workload"),
                ("spaces", "spaces"),
                ("size", "notes"),
                ("arm", "arm"),
                ("pair", "pair"),
                ("cache", "cache"),
                ("file_placement", "file placement"),
            ]
            .into_iter()
            .filter_map(|(key, label)| {
                run.get(key).map(|value| {
                    (
                        if key == "spaces" && run.get("clients").is_some() {
                            "target spaces"
                        } else {
                            label
                        },
                        value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string()),
                    )
                })
            });
            reply = reply.blank().fields(fields);
            if let Some(values) = run["summary"].as_array() {
                reply = metrics(reply, values);
            }
        }
    } else if report["summary"].is_object() {
        reply = metrics(reply, std::slice::from_ref(&report["summary"]));
    }
    if let Some(counts) = report["sync_calls"].as_object() {
        reply = reply.blank().table(
            &["DESCRIPTOR", "SYNC CALLS"],
            counts
                .iter()
                .map(|(kind, count)| vec![kind.replace('_', " "), count.to_string()])
                .collect(),
            "No sync calls observed.",
        );
    }
    if let Some(rows) = report["observations"].as_array() {
        reply = reply.blank().table(
            &["SPACES", "REPEAT", "BEFORE MiB", "AFTER MiB", "WRITERS"],
            rows.iter()
                .map(|r| {
                    vec![
                        r["spaces"].to_string(),
                        r["repeat"].to_string(),
                        r["before_mib"].to_string(),
                        r["after_mib"].to_string(),
                        r["writers_after"].to_string(),
                    ]
                })
                .collect(),
            "No memory observations.",
        );
    }
    if let Some(rows) = report["failures"].as_array() {
        for row in rows {
            reply = reply.blank().fields([
                ("failed arm", row["arm"].as_str().unwrap_or("").to_owned()),
                ("space count", row["spaces"].to_string()),
                ("error", row["error"].as_str().unwrap_or("").to_owned()),
            ]);
        }
    }
    reply = reply.hint("Inspect the report file for evidence, or use --help for another run.");
    reply
}
fn metrics(reply: Reply, values: &[Value]) -> Reply {
    let rows = values
        .iter()
        .map(|r| {
            let mut row = vec![
                r["metric"].as_str().unwrap_or("").to_owned(),
                if r["breached"] == true {
                    "breach".into()
                } else {
                    "observed".into()
                },
            ];
            for k in ["p95", "p50", "p99", "max"] {
                row.push(
                    r[k].as_f64()
                        .map(|v| format!("{v:.3}"))
                        .unwrap_or_else(|| "—".into()),
                );
            }
            row.push(r["samples"].to_string());
            row
        })
        .collect();
    reply.table(
        &["METRIC", "STATE", "P95", "P50", "P99", "MAX", "N"],
        rows,
        "No latency samples; inspect failures.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn narrow_report_preserves_breach_and_primary_quantile() {
        let value = serde_json::json!({"qualification":"incomplete","runs":[{"spaces":1,"size":1000,"arm":"head","pair":0,"cache":"warm","summary":[{"metric":"notes.update","breached":true,"p50":42.0,"p95":50.0,"p99":52.0,"max":53.0,"samples":100}]}]});
        for width in [40, 80, 120] {
            let output = calternal_cli::output::Output::fixed_terminal(
                false,
                width,
                time::UtcOffset::UTC,
                true,
            );
            let text = output.render(&human_reply(&value, Path::new("summary.json")));
            assert!(
                text.contains("notes.update") && text.contains("breach") && text.contains("50.000"),
                "{text}"
            );
            assert!(
                text.lines().all(|line| line.chars().count() <= width),
                "{text}"
            );
        }
    }
}
