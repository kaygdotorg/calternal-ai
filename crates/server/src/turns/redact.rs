//! Scrub the issued credential before provider output reaches any public state.
use calternal_core::ProviderEventKind;
use serde_json::Value;

// No Debug: both fields may hold credential bytes. Pending text is bounded by
// the credential's length, and only a possible cross-delta match is delayed.
pub(super) struct Redactor {
    secret: String,
    pending: String,
}
impl Redactor {
    pub(super) fn new(secret: String) -> Self {
        assert!(!secret.is_empty());
        Self {
            secret,
            pending: String::new(),
        }
    }
    fn string(&self, text: &mut String) {
        *text = text.replace(&self.secret, "[redacted]");
    }
    pub(super) fn event(&mut self, event: &mut ProviderEventKind) -> Option<String> {
        use ProviderEventKind::*;
        match event {
            TextDelta { text } => return self.delta(text),
            Check(status) => self.string(&mut status.sign_in_command),
            Session { session_id } => self.string(session_id),
            ToolStarted { id, name, summary }
            | ToolOutput {
                id, name, summary, ..
            } => {
                self.tool(id, name, summary);
            }
            ToolFinished {
                id,
                name,
                summary,
                detail,
                ..
            } => {
                self.tool(id, name, summary);
                self.string(detail);
                *detail = detail.replace(char::is_control, " ");
            }
            Question { text } => self.string(text),
            ApprovalRequest { summary } => self.string(summary),
            Error { code, message, .. } => {
                self.string(code);
                self.string(message);
            }
            Done { outcome } => self.string(outcome),
            // The turn reader routes reasoning through an independent text redactor.
            ReasoningStarted | ReasoningDelta { .. } | ReasoningFinished { .. } | Usage { .. } => {}
        }
        None
    }
    /// Scrub a contiguous text channel. A possible split credential stays
    /// private until later bytes distinguish it from the issued token.
    pub(super) fn delta(&mut self, text: &mut String) -> Option<String> {
        let mut combined = std::mem::take(&mut self.pending);
        combined.push_str(text);
        self.string(&mut combined);
        let held = (1..self.secret.len())
            .rev()
            .find(|&n| combined.ends_with(&self.secret[..n]))
            .unwrap_or(0);
        self.pending = combined.split_off(combined.len() - held);
        if combined.len() > 4096 {
            let mut cut = combined.len() - 4096;
            while !combined.is_char_boundary(cut) {
                cut += 1;
            }
            *text = combined.split_off(cut);
            return Some(combined);
        }
        *text = combined;
        None
    }
    fn tool(&self, id: &mut String, name: &mut String, summary: &mut String) {
        self.string(id);
        self.string(name);
        self.string(summary);
        for text in [&mut *name, &mut *summary] {
            *text = text.replace(char::is_control, " ").trim().to_owned();
        }
        if name.is_empty() {
            name.push_str("Tool");
        }
        if summary.is_empty() {
            summary.push_str("Run a provider tool");
        }
    }
    /// At terminal output a remaining proper prefix cannot become a complete
    /// credential. Flush it as a final ordered text delta before Done.
    pub(super) fn finish(&mut self) -> Option<String> {
        (!self.pending.is_empty()).then(|| std::mem::take(&mut self.pending))
    }
    /// Defense at every durable boundary, including checkpoint metadata strings.
    pub(super) fn checkpoint(&self, value: &mut Value) {
        match value {
            Value::String(s) => self.string(s),
            Value::Array(values) => {
                for v in values {
                    self.checkpoint(v)
                }
            }
            Value::Object(values) => {
                for v in values.values_mut() {
                    self.checkpoint(v)
                }
            }
            _ => {}
        }
    }
}

/// Independent text channels for one issued credential. Boundaries in one
/// channel cannot flush a withheld credential prefix from the other.
pub(super) struct Channels {
    text: Redactor,
    reasoning: Redactor,
}

impl Channels {
    /// Retain the issued token once per channel; storage is bounded by its size.
    pub(super) fn new(token: String) -> Self {
        Self {
            text: Redactor::new(token.clone()),
            reasoning: Redactor::new(token),
        }
    }

    /// Scrub a frame and return any overflow frame that must precede it.
    pub(super) fn event(&mut self, event: &mut ProviderEventKind) -> Option<ProviderEventKind> {
        use ProviderEventKind::*;
        match event {
            ReasoningDelta { text } => self
                .reasoning
                .delta(text)
                .map(|text| ReasoningDelta { text }),
            ReasoningFinished { .. } => {
                // Drop a possible credential prefix at the block boundary.
                let _ = self.reasoning.finish();
                None
            }
            Done { .. } => {
                let _ = self.text.event(event);
                self.text.finish().map(|text| TextDelta { text })
            }
            _ => self.text.event(event).map(|text| TextDelta { text }),
        }
    }

    /// Apply the existing whole-checkpoint scrub in addition to channel filtering.
    pub(super) fn checkpoint(&self, value: &mut Value) {
        self.text.checkpoint(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_text_split_and_checkpoint_value_is_scrubbed() {
        let token = "ct1_synthetic-private-credential";
        for split in 0..=token.len() {
            let mut r = Redactor::new(token.into());
            let mut output = String::new();
            for text in [
                format!("before {}", &token[..split]),
                format!("{} after c", &token[split..]),
            ] {
                let mut e = ProviderEventKind::TextDelta { text };
                if let Some(first) = r.event(&mut e) {
                    output.push_str(&first)
                }
                if let ProviderEventKind::TextDelta { text } = e {
                    output.push_str(&text)
                }
            }
            if let Some(last) = r.finish() {
                output.push_str(&last)
            }
            assert_eq!(output, "before [redacted] after c");
            let mut checkpoint =
                serde_json::json!({"text_tail":token,"nested":[{"summary":token}]});
            r.checkpoint(&mut checkpoint);
            assert!(!checkpoint.to_string().contains(token));
        }
    }
    #[test]
    fn delayed_prefix_keeps_text_frames_within_the_protocol_bound() {
        let mut r = Redactor::new("ct1_example".into());
        let mut first = ProviderEventKind::TextDelta {
            text: "ct1_".into(),
        };
        assert!(r.event(&mut first).is_none());
        let mut next = ProviderEventKind::TextDelta {
            text: "🌏".repeat(1024),
        };
        let overflow = r.event(&mut next).unwrap();
        if let ProviderEventKind::TextDelta { text } = next {
            assert!(overflow.len() <= 4096 && text.len() <= 4096);
            assert_eq!(
                format!("{overflow}{text}"),
                format!("ct1_{}", "🌏".repeat(1024))
            );
        }
    }
}
