//! Bounded deterministic diagnostics shared by parsing, validation and evaluation.
use crate::ast::{Diagnostic, Pos};
#[derive(Clone, Debug)]
pub(crate) struct Failure {
    pub pos: Pos,
    pub message: String,
    pub limit: bool,
}
impl Failure {
    pub fn new(pos: Pos, message: impl Into<String>) -> Self {
        Self {
            pos,
            message: message.into(),
            limit: false,
        }
    }
    pub fn limit(pos: Pos, key: &str) -> Self {
        Self {
            pos,
            message: format!("budget exceeded: {key}"),
            limit: true,
        }
    }
}
#[derive(Default)]
pub(crate) struct Diagnostics {
    pub items: Vec<Diagnostic>,
    pub has_errors: bool,
}
impl Diagnostics {
    pub fn report(&mut self, pos: Pos, message: impl Into<String>) {
        self.has_errors = true;
        let message = message.into();
        if self.items.len() < 20
            && !self
                .items
                .iter()
                .any(|d| d.line == pos.line && d.column == pos.column && d.message == message)
        {
            self.items.push(Diagnostic {
                severity: "error".into(),
                line: pos.line,
                column: pos.column,
                message,
            });
        }
    }
}
