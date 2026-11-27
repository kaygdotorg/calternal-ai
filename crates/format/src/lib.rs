//! In-memory parsing and evaluation for Calternal plugin definitions and items.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod dates;
mod definition;
mod expr;
mod field;
mod item;
mod lines;
mod note_lines;
mod operation;
mod query;
mod value;

pub use definition::{
    AgentRule, AgentRules, CheckboxMapping, Collection, CollectionView, ComposerBindings,
    DavComponent, DavMapping, DavProperty, Directive, Editability, Fill, FillBinding,
    InstallOptions, ItemForm, Kind, LineClaim, LineForm, Manifest, PageEntry, Plugin, PluginFile,
    Program, Refusal, Rights, ShareSource, Sidebar, SidebarEntry, Slot, Sort, Source, Total,
    parse_agent_rules, parse_plugin,
};
pub use expr::{EvalContext, Expression, QueryRowContext, Row, evaluate, parse_expression};
pub use field::{Field, FieldType};
pub use item::{Item, Record, parse_item, parse_records, write_item};
pub use note_lines::{LineItem, NoteLine, scan_note_lines};
pub use operation::{
    Assignment, Change, Operation, OperationForm, OperationInput, apply_operation, check_item,
    operation_conditions,
};
pub use query::{Page, QueryGroup, QueryResult, QueryRow, evaluate_query};
pub use value::{Decimal, Fields, InvalidDecimal, Value};

/// A plain-language refusal with a position in the source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    /// Plugin-relative path when the input came from a plugin folder.
    pub path: Option<String>,
    /// One-based physical line.
    pub line: usize,
    /// One-based column measured in Unicode scalar values.
    pub column: usize,
    /// Plain-language explanation of the refusal.
    pub message: String,
}

impl Diagnostic {
    fn at(line: usize, column: usize, message: impl Into<String>) -> Self {
        Self {
            path: None,
            line,
            column,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(path) = &self.path {
            write!(f, "{path}:")?;
        }
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for Diagnostic {}
