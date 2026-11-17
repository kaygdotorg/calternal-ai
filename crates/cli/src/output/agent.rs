//! Streaming provider-event rendering through the shared CLI output surface.
use super::{CliError,Output};
use serde_json::{Value,json};
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
                "text_delta" => {
                    let _ = write!(out, "{}", e["text"].as_str().unwrap_or(""));
                }
                "tool_started" => {
                    let _ = writeln!(
                        out,
                        "\nStep: {}",
                        e["summary"].as_str().unwrap_or("Tool started")
                    );
                }
                "tool_finished" => {
                    let _ = writeln!(
                        out,
                        "Step finished: {}",
                        if e["failed"] == true {
                            "failed"
                        } else {
                            "done"
                        }
                    );
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

