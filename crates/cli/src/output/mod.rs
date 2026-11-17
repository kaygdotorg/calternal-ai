//! How every command prints: [`Reply`] for success, [`CliError`] for failure,
//! [`Output`] to render both.
//!
//! Human mode (default) writes results to stdout and errors to stderr. With
//! `--json` the same information is one pretty-printed JSON document on stdout,
//! for errors too, so a script reads stdout and the exit status only:
//!
//! ```json
//! {"ok": true,  "data": { ... }}
//! {"ok": false, "error": {"code": "not_found", "message": "...", "hint": "..."}}
//! ```
//!
//! Exit codes: [`EXIT_OK`] 0, [`EXIT_FAILURE`] 1, [`EXIT_USAGE`] 2 (a command line
//! that does not parse); commands may add their own (`calternal machine run`
//! uses 3 for "revoked"). Secrets are never part of a [`Reply`] unless the command
//! exists to issue them.
//!
//! Colour: only bold table headers, only when stdout is a terminal, `NO_COLOR` is
//! unset and `TERM` is not `dumb`. Width: `COLUMNS`, else the terminal's; pipes
//! are never shortened. A successful command may add one terminal-only hint.

mod table;

use std::io::{IsTerminal, Write};

use serde_json::{Map, Value, json};
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

/// Success.
pub const EXIT_OK: i32 = 0;
/// The command ran and failed.
pub const EXIT_FAILURE: i32 = 1;
/// The command line did not parse.
pub const EXIT_USAGE: i32 = 2;

/// One piece of a human-readable result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// A single sentence or line.
    Line(String),
    /// Several lines of free text, printed as is (for example a note body).
    Text(String),
    /// A labelled single thing: `(label, value)` pairs, aligned.
    Fields(Vec<(String, String)>),
    /// A list: header cells and rows of cells.
    Table {
        /// Column headings, upper case by convention.
        headers: Vec<String>,
        /// One `Vec` of cells per row.
        rows: Vec<Vec<String>>,
    },
    /// An empty line between blocks.
    Blank,
}

/// A successful result: the JSON `data` plus its human rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    json: Value,
    request_id: Option<String>,
    blocks: Vec<Block>,
    hint: Option<String>,
}

impl Reply {
    /// Start a reply whose `--json` form is `data` (documented per command in `docs/cli-shapes.json`).
    pub fn new(data: Value) -> Self {
        Self {
            json: data,
            request_id: None,
            blocks: Vec::new(),
            hint: None,
        }
    }
    /// Add the request identity (shown as `request_id` in JSON and as a field line in human mode only when asked for).
    pub fn request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }
    /// Append a one-line block.
    pub fn line(mut self, text: impl Into<String>) -> Self {
        self.blocks.push(Block::Line(text.into()));
        self
    }
    /// Append a free-text block.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.blocks.push(Block::Text(text.into()));
        self
    }
    /// Append a blank line.
    pub fn blank(mut self) -> Self {
        self.blocks.push(Block::Blank);
        self
    }
    /// Append a labelled block.
    pub fn fields<K: Into<String>, V: Into<String>>(
        mut self,
        items: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        let items = items
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        self.blocks.push(Block::Fields(items));
        self
    }
    /// Append a table, or the `empty` sentence when there are no rows.
    pub fn table(mut self, headers: &[&str], rows: Vec<Vec<String>>, empty: &str) -> Self {
        self.blocks.push(if rows.is_empty() {
            Block::Line(empty.to_owned())
        } else {
            Block::Table {
                headers: headers.iter().map(|h| (*h).to_owned()).collect(),
                rows,
            }
        });
        self
    }
    /// Add one short next-step hint for terminal human output.
    ///
    /// The hint is omitted from JSON and from piped output. Calling this more
    /// than once replaces the earlier hint, so a reply can show at most one.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// A failed command: what happened (`message`), what to do next (`hint`), and a
/// stable machine-readable `code` (`usage`, `not_found`, `unauthorized`, `io`, …).
#[derive(Debug, Clone, PartialEq)]
pub struct CliError {
    /// Stable snake_case category; scripts branch on this, never on `message`.
    pub code: String,
    /// What happened, in a sentence, without secrets.
    pub message: String,
    /// The next step, or `None` when there is nothing more to try.
    pub hint: Option<String>,
    /// Process exit status ([`EXIT_FAILURE`] unless set otherwise).
    pub exit: i32,
    request_id: Option<String>,
    details: Option<Box<Value>>,
    human_message: Option<String>,
    human_hint: Option<String>,
}

impl CliError {
    /// A failure with `code` and `message`, exit status 1.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            hint: None,
            exit: EXIT_FAILURE,
            request_id: None,
            details: None,
            human_message: None,
            human_hint: None,
        }
    }
    /// A command line that does not parse: code `usage`, exit status 2.
    pub fn usage(message: impl Into<String>) -> Self {
        Self::new("usage", message).exit(EXIT_USAGE)
    }
    /// Set the next-step sentence.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    /// Set wording for terminal output while keeping JSON `message` unchanged.
    pub fn human_message(mut self, message: impl Into<String>) -> Self {
        self.human_message = Some(message.into());
        self
    }
    /// Set terminal-only next-step wording while keeping JSON `hint` unchanged.
    pub fn human_hint(mut self, hint: impl Into<String>) -> Self {
        self.human_hint = Some(hint.into());
        self
    }
    /// Set the process exit status.
    pub fn exit(mut self, exit: i32) -> Self {
        self.exit = exit;
        self
    }
    /// Attach the request identity of the failed call.
    pub fn request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }
    /// Attach structured diagnostics (JSON `error.details`; human mode ignores them).
    pub fn details(mut self, details: Value) -> Self {
        self.details = Some(Box::new(details));
        self
    }
}

/// Renders replies and errors for one process. Build it with [`Output::detect`]
/// before any thread starts (the local time zone cannot be read safely after).
#[derive(Debug, Clone)]
pub struct Output {
    json: bool,
    bold: bool,
    dim: bool,
    hints: bool,
    width: Option<usize>,
    offset: UtcOffset,
}

impl Output {
    /// Output for this process: `json` from `--json`; colour, width and time
    /// zone from the environment. Reads `--no-hints` from the argument vector.
    /// Call from `main` before starting a runtime.
    pub fn detect(json: bool) -> Self {
        let no_hints_arg = std::env::args_os().any(|arg| arg == "--no-hints");
        Self::detect_with_options(json, no_hints_arg)
    }

    /// Output for a process when its parser has already read the hint option.
    ///
    /// `CALTERNAL_NO_HINTS=1` also disables hints. A width is used only when
    /// stdout is a terminal, so redirected output is never shortened.
    pub fn detect_with_options(json: bool, no_hints_arg: bool) -> Self {
        let stdout = std::io::stdout();
        let tty = stdout.is_terminal();
        let plain = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
            || std::env::var("TERM").is_ok_and(|t| t == "dumb");
        let measured_width = tty
            .then(|| rustix::termios::tcgetwinsize(&stdout).ok())
            .flatten()
            .map(|size| usize::from(size.ws_col));
        let columns = std::env::var("COLUMNS").ok();
        let width = terminal_width(tty, columns.as_deref(), measured_width);
        let hints_disabled = std::env::var_os("CALTERNAL_NO_HINTS");
        Self {
            json,
            bold: tty && !plain && !json,
            dim: tty && !plain && !json,
            hints: hints_enabled(tty, json, no_hints_arg, hints_disabled.as_deref()),
            width: if json { None } else { width },
            offset: UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC),
        }
    }
    /// Output with fixed settings and no colour, for tests and golden files.
    pub fn fixed(json: bool, width: Option<usize>, offset: UtcOffset) -> Self {
        Self {
            json,
            bold: false,
            dim: false,
            hints: false,
            width,
            offset,
        }
    }
    /// Output with a simulated terminal for fixed-width tests and golden files.
    ///
    /// `hints` simulates the default terminal behavior after `--no-hints` and
    /// `CALTERNAL_NO_HINTS=1` have been applied. Colour remains disabled.
    #[cfg(any(test, feature = "testing"))]
    pub fn fixed_terminal(json: bool, width: usize, offset: UtcOffset, hints: bool) -> Self {
        Self {
            json,
            bold: false,
            dim: false,
            hints: hints && !json,
            width: if json { None } else { Some(width.max(1)) },
            offset,
        }
    }
    /// True when `--json` was requested.
    pub fn is_json(&self) -> bool {
        self.json
    }
    /// A Unix-millisecond time in this process's local zone, `2026-11-17 07:12:03 +01:00`.
    pub fn local_ms(&self, unix_ms: i64) -> String {
        match OffsetDateTime::from_unix_timestamp_nanos(i128::from(unix_ms) * 1_000_000) {
            Ok(t) => self.local(t),
            Err(_) => unix_ms.to_string(),
        }
    }
    /// An RFC 3339 time in the local zone; text that is not RFC 3339 is returned unchanged.
    pub fn local_rfc3339(&self, text: &str) -> String {
        OffsetDateTime::parse(text, &Rfc3339).map_or_else(|_| text.to_owned(), |t| self.local(t))
    }
    fn local(&self, t: OffsetDateTime) -> String {
        let t = t.to_offset(self.offset);
        let (h, m, _) = t.offset().as_hms();
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {}{:02}:{:02}",
            t.year(),
            u8::from(t.month()),
            t.day(),
            t.hour(),
            t.minute(),
            t.second(),
            if h < 0 || m < 0 { '-' } else { '+' },
            h.abs(),
            m.abs()
        )
    }
    /// A Unix-millisecond time as UTC RFC 3339, the form every JSON document uses.
    pub fn iso_ms(unix_ms: i64) -> String {
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(unix_ms) * 1_000_000)
            .ok()
            .and_then(|t| t.format(&Rfc3339).ok())
            .unwrap_or_else(|| unix_ms.to_string())
    }

    /// The text written to stdout for a successful `reply`. Cost: linear in the cells.
    pub fn render(&self, reply: &Reply) -> String {
        if self.json {
            let mut doc = Map::new();
            doc.insert("ok".into(), Value::Bool(true));
            doc.insert("data".into(), reply.json.clone());
            if let Some(id) = &reply.request_id {
                doc.insert("request_id".into(), json!(id));
            }
            return pretty(Value::Object(doc));
        }
        let mut out = String::new();
        for block in &reply.blocks {
            match block {
                Block::Line(l) => out.push_str(&format!("{l}\n")),
                Block::Text(t) => {
                    out.push_str(t);
                    if !t.ends_with('\n') {
                        out.push('\n');
                    }
                }
                Block::Fields(items) => out.push_str(&table::fields(items)),
                Block::Table { headers, rows } => {
                    out.push_str(&table::table(headers, rows, self.width, self.bold))
                }
                Block::Blank => out.push('\n'),
            }
        }
        if self.hints
            && let Some(hint) = &reply.hint
        {
            let hint = table::fit_line(hint, self.width);
            if self.dim {
                out.push_str("\x1b[2m");
                out.push_str(&hint);
                out.push_str("\x1b[0m\n");
            } else {
                out.push_str(&hint);
                out.push('\n');
            }
        }
        out
    }
    /// The `(stdout, stderr)` text for a failure: JSON on stdout with `--json`,
    /// otherwise `error: …` and `hint: …` on stderr.
    pub fn render_error(&self, e: &CliError) -> (String, String) {
        if self.json {
            let mut err = Map::new();
            err.insert("code".into(), json!(e.code));
            err.insert("message".into(), json!(e.message));
            err.insert("hint".into(), json!(e.hint));
            if let Some(d) = &e.details {
                err.insert("details".into(), (**d).clone());
            }
            let mut doc = Map::new();
            doc.insert("ok".into(), Value::Bool(false));
            doc.insert("error".into(), Value::Object(err));
            if let Some(id) = &e.request_id {
                doc.insert("request_id".into(), json!(id));
            }
            return (pretty(Value::Object(doc)), String::new());
        }
        let message = e.human_message.as_ref().unwrap_or(&e.message);
        let hint = e.human_hint.as_ref().or(e.hint.as_ref());
        let mut err = format!("error: {message}\n");
        if let Some(h) = hint {
            err.push_str(&format!("hint: {h}\n"));
        }
        (String::new(), err)
    }
    /// Print one streaming reply, as one compact JSON line with `--json` or the
    /// ordinary width-aware human rendering. Flushes stdout before returning;
    /// broken pipes and write failures return EXIT_FAILURE. O(reply bytes).
    pub fn event(&self, reply: &Reply) -> i32 {
        let text = if self.json {
            format!("{}\n", serde_json::json!({"ok":true,"data":reply.json}))
        } else {
            self.render(reply)
        };
        let mut stdout = std::io::stdout().lock();
        if stdout
            .write_all(text.as_bytes())
            .and_then(|_| stdout.flush())
            .is_ok()
        {
            EXIT_OK
        } else {
            EXIT_FAILURE
        }
    }
    /// Print `reply` to stdout and return [`EXIT_OK`].
    pub fn success(&self, reply: &Reply) -> i32 {
        let _ = std::io::stdout().write_all(self.render(reply).as_bytes());
        EXIT_OK
    }
    /// Print `error` (see [`Output::render_error`]) and return its exit status.
    pub fn failure(&self, error: &CliError) -> i32 {
        let (out, err) = self.render_error(error);
        let _ = std::io::stdout().write_all(out.as_bytes());
        let _ = std::io::stderr().write_all(err.as_bytes());
        error.exit
    }
    /// Print the result of a command and return its exit status.
    pub fn finish(&self, result: Result<Reply, CliError>) -> i32 {
        match result {
            Ok(reply) => self.success(&reply),
            Err(e) => self.failure(&e),
        }
    }
}

/// Whether a successful result may show a next-step hint.
fn hints_enabled(
    terminal: bool,
    json: bool,
    no_hints_arg: bool,
    no_hints_env: Option<&std::ffi::OsStr>,
) -> bool {
    terminal && !json && !no_hints_arg && no_hints_env != Some(std::ffi::OsStr::new("1"))
}

/// Use a width only when stdout is a terminal; a pipe always keeps full cells.
fn terminal_width(terminal: bool, columns: Option<&str>, measured: Option<usize>) -> Option<usize> {
    if !terminal {
        return None;
    }
    columns
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width > 0)
        .or_else(|| measured.filter(|width| *width > 0))
}

/// Rebuild `v` with object keys in alphabetical order, so the printed order is
/// the same whether or not some crate in the build enabled `serde_json`'s
/// `preserve_order`. Cost: one pass over the document.
fn sorted(v: Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(entries.into_iter().map(|(k, v)| (k, sorted(v))).collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

/// `v` as indented JSON with alphabetical keys, for showing a structure as text.
pub fn json_text(v: &Value) -> String {
    serde_json::to_string_pretty(&sorted(v.clone())).expect("JSON value serializes")
}

fn pretty(doc: Value) -> String {
    let mut s = json_text(&doc);
    s.push('\n');
    s
}

/// Parse the process arguments as `C`. On a bad command line prints the usage
/// error (as `--json` JSON when the arguments contain `--json`) and returns the
/// exit status to use; help and version requests print and exit 0 directly.
pub fn parse_args<C: clap::Parser>() -> Result<C, i32> {
    C::try_parse().map_err(|e| {
        if !e.use_stderr() {
            e.exit()
        }
        let json = std::env::args_os().any(|a| a == "--json");
        let rendered = e.render().to_string();
        let first = rendered
            .lines()
            .next()
            .unwrap_or("")
            .trim_start_matches("error: ");
        let error = CliError::usage(first).hint("run the command with --help to see its usage");
        Output::detect(json).failure(&error)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hints_show_once_only_for_human_terminal_output() {
        let reply = Reply::new(json!({"ok": true}))
            .line("Done.")
            .hint("Next: run `cl notes list`.")
            .hint("Next: run `cl notes create`.");
        let terminal = Output::fixed_terminal(false, 80, UtcOffset::UTC, true);
        let text = terminal.render(&reply);
        assert!(text.ends_with("Next: run `cl notes create`.\n"));
        assert_eq!(text.matches("Next:").count(), 1);

        let disabled = Output::fixed_terminal(false, 80, UtcOffset::UTC, false);
        assert_eq!(disabled.render(&reply), "Done.\n");

        let json = Output::fixed_terminal(true, 80, UtcOffset::UTC, true);
        assert_eq!(
            json.render(&reply),
            "{\n  \"data\": {\n    \"ok\": true\n  },\n  \"ok\": true\n}\n"
        );
        assert!(!hints_enabled(
            true,
            false,
            false,
            Some(std::ffi::OsStr::new("1"))
        ));
        assert!(!hints_enabled(true, false, true, None));
        assert!(!hints_enabled(false, false, false, None));
        assert!(!hints_enabled(true, true, false, None));
        assert!(hints_enabled(true, false, false, None));
    }

    #[test]
    fn redirected_output_ignores_columns_and_keeps_full_width() {
        assert_eq!(terminal_width(false, Some("40"), Some(80)), None);
        assert_eq!(terminal_width(true, Some("40"), Some(80)), Some(40));
        assert_eq!(terminal_width(true, Some("0"), Some(80)), Some(80));
        assert_eq!(terminal_width(true, Some("invalid"), None), None);
    }

    #[test]
    fn json_errors_keep_original_text_while_human_output_can_be_friendly() {
        let error = CliError::new("not_found", "no machine id")
            .hint("run the list command")
            .human_message("No machine with that ID was found.")
            .human_hint("Next: run `calternal machines list`.");
        let human = Output::fixed(false, None, UtcOffset::UTC)
            .render_error(&error)
            .1;
        assert_eq!(
            human,
            "error: No machine with that ID was found.\nhint: Next: run `calternal machines list`.\n"
        );
        let json = Output::fixed(true, None, UtcOffset::UTC)
            .render_error(&error)
            .0;
        assert!(json.contains("no machine id"));
        assert!(json.contains("run the list command"));
        assert!(!json.contains("No machine with that ID was found."));
    }
}
