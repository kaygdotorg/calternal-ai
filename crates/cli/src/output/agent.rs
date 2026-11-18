//! Streaming provider-event rendering through the shared CLI output surface.
use super::{CliError, Output};
use serde_json::{Value, json};
use std::io::Write;

impl Output {
    /// Write and flush one provider event. JSON mode is newline-delimited JSON;
    /// human mode streams text deltas and renders steps as labelled status lines.
    /// Unknown event payloads remain structured JSON so no information is hidden.
    pub fn agent_event(&self, value: &Value) {
        let mut out = std::io::stdout().lock();
        if self.json {
            let _ = writeln!(out, "{value}");
        } else {
            let e = value.get("event").unwrap_or(value);
            match e.get("type").and_then(Value::as_str).unwrap_or("") {
                "discard" => {
                    let state = e["state"].as_str().unwrap_or("unknown");
                    let _ = writeln!(out, "Workspace discard: {state}");
                    let _ = writeln!(out, "Account login home: retained");
                    if state == "pending" {
                        let _ = writeln!(
                            out,
                            "Next: run this discard command again to check the machine confirmation."
                        );
                    }
                    if state == "failed" {
                        let _ = writeln!(
                            out,
                            "Next: inspect the machine and active work, then retry with --retry."
                        );
                    }
                }
                "text_delta" => {
                    let _ = write!(out, "{}", e["text"].as_str().unwrap_or(""));
                }
                "tool_started" | "tool_output" | "tool_finished" => {
                    let name = e["name"].as_str().unwrap_or("Tool");
                    let summary = e["summary"].as_str().unwrap_or("");
                    let status = match e["type"].as_str() {
                        Some("tool_output") => {
                            format!("{} bytes of output", e["bytes"].as_u64().unwrap_or(0))
                        }
                        Some("tool_finished") if e["failed"] == true => "failed".into(),
                        Some("tool_finished") => "finished".into(),
                        _ => "started".into(),
                    };
                    let _ = writeln!(out, "\nStep: {name} · {summary} ({status})");
                }
                "job" => {
                    let _ = writeln!(out, "Turn: {}", e["job"]["id"].as_str().unwrap_or(""));
                }
                "end" => {
                    let _ = writeln!(
                        out,
                        "\nState: {}\n{}",
                        e["job"]["state"].as_str().unwrap_or("unknown"),
                        e["hint"].as_str().unwrap_or("")
                    );
                }
                "error" => {
                    let _ = writeln!(
                        out,
                        "\nError: {}\n{}",
                        e["message"].as_str().unwrap_or("Provider failed"),
                        e["hint"].as_str().unwrap_or("")
                    );
                }
                _ => {
                    let _ = writeln!(out, "\n{e}");
                }
            }
        }
        let _ = out.flush();
    }
}

impl Output {
    /// Render a streaming command failure as one NDJSON event, retaining the
    /// approval and retry identity details a caller needs to proceed safely.
    pub fn agent_failure(&self, error: &CliError) {
        self.agent_event(&json!({"type":"error","code":error.code,"message":error.message,"hint":error.hint,"request_id":error.request_id,"details":error.details}));
    }
}
