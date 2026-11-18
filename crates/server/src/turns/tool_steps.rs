//! Fold scrubbed tool state into the existing Markdown agent message.
use calternal_core::ProviderEventKind;
use calternal_types::Error;
use serde_json::{Value, json};
use std::{collections::HashSet, fmt::Write};

/// Update a pending tool and its bounded saved summary. An unpaired event
/// or too many simultaneous steps is a provider protocol error.
pub(super) fn observe(
    checkpoint: &mut Value,
    pending_tools: &mut HashSet<String>,
    event: &ProviderEventKind,
    saved_thread: bool,
) -> Result<(), Error> {
    match event {
        ProviderEventKind::ToolStarted { id, name, summary } => {
            if pending_tools.len() >= 128 || !pending_tools.insert(id.clone()) {
                return Err(Error::invalid_input(
                    "provider pending tool limit or duplicate tool id",
                ));
            }
            checkpoint["last_tool"] =
                json!({"id":id,"name":name,"summary":summary,"state":"started"});
            if saved_thread {
                append(checkpoint, name, summary, "started");
            }
        }
        ProviderEventKind::ToolOutput {
            id,
            name,
            summary,
            bytes,
        } => {
            if !pending_tools.contains(id) {
                return Err(Error::invalid_input("output for an unknown tool step"));
            }
            checkpoint["last_tool"] =
                json!({"id":id,"name":name,"summary":summary,"state":"output","bytes":bytes});
            if saved_thread {
                append(
                    checkpoint,
                    name,
                    summary,
                    &format!("{bytes} bytes of tool output"),
                );
            }
        }
        ProviderEventKind::ToolFinished {
            id,
            name,
            summary,
            failed,
            ..
        } => {
            if !pending_tools.remove(id) {
                return Err(Error::invalid_input("completion for an unknown tool step"));
            }
            checkpoint["last_tool"] =
                json!({"id":id,"name":name,"summary":summary,"state":"finished","failed":failed});
            if saved_thread {
                append(
                    checkpoint,
                    name,
                    summary,
                    if *failed { "failed" } else { "finished" },
                );
            }
        }
        _ => unreachable!("only tool events are folded here"),
    }
    Ok(())
}

fn append(checkpoint: &mut Value, name: &str, summary: &str, status: &str) {
    let mut saved = checkpoint["thread_text"].as_str().unwrap_or("").to_owned();
    saved.push_str("\n\n> Tool ");
    inline(&mut saved, name);
    saved.push_str(" · ");
    inline(&mut saved, summary);
    let _ = writeln!(saved, " ({status})\n");
    checkpoint["thread_text"] = json!(saved);
}

fn inline(out: &mut String, text: &str) {
    // Provider data stays within the quoted line. In particular it cannot write
    // a message delimiter, a link or a raw HTML tag.
    for ch in text.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\\' | '[' | ']' | '(' | ')' | '*' | '_' | '`' => {
                out.push('\\');
                out.push(ch);
            }
            _ if ch.is_control() => out.push(' '),
            _ => out.push(ch),
        }
    }
}
