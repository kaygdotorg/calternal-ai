//! Portable generated-UI descriptions: bounded parsing, validation and pure evaluation.
//!
//! Native clients walk the returned JSON into their own controls. Actions are data; no
//! filesystem, network, platform UI, credentials, JavaScript execution or Wasm is included.
//! Source positions follow the upstream language's one-based UTF-16 columns.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

/// Serializable source positions, diagnostics and expression/statement AST types.
pub mod ast;
/// Resource ceilings for the current description, local state and output.
pub mod budgets;
/// Shared pure-data native component vocabulary.
pub mod catalog;
mod error;
/// State initialization and bounded native-description evaluation.
pub mod evaluate;
mod lexer;
mod math;
mod parser;
mod validate;
mod value;

pub use ast::{Diagnostic, Program, Statement};
pub use budgets::Budgets;
use error::Diagnostics;
pub use evaluate::{Evaluation, evaluate, initial_state};

/// Parse/validate result. Any error leaves the program absent and prevents activation.
#[derive(Clone, Debug)]
pub struct Compilation {
    /// Accepted program, created only after all static validation succeeds.
    pub program: Option<Program>,
    /// Parsed statements, including statements that have semantic errors.
    pub statements: Vec<Statement>,
    /// Source-ordered diagnostics, capped at twenty distinct messages.
    pub diagnostics: Vec<Diagnostic>,
}
/// Parse the credited upstream syntax and validate catalog, accessibility and budgets.
/// Cost is O(source bytes + bounded AST/dependency walks), allocating tokens, AST and local
/// symbol/reference tables. No locks beyond first catalog initialization, I/O, fsyncs or network.
/// Invalid budgets and every syntax/validation error produce diagnostics and no program.
pub fn compile(source: &str, limits: Budgets) -> Compilation {
    let mut diagnostics = Diagnostics::default();
    if let Err(message) = limits.validate() {
        diagnostics.report(ast::Pos::start(), message);
        return Compilation {
            program: None,
            statements: vec![],
            diagnostics: diagnostics.items,
        };
    }
    if source.len() > limits.serialized_bytes {
        diagnostics.report(ast::Pos::start(), "budget exceeded: serializedBytes");
        return Compilation {
            program: None,
            statements: vec![],
            diagnostics: diagnostics.items,
        };
    }
    let tokens = lexer::tokenize(source, &mut diagnostics);
    let statements = parser::parse(&tokens, &mut diagnostics);
    let mut program = None;
    if !diagnostics.has_errors {
        if budgets::json_bytes(&serde_json::to_value(&statements).expect("AST is JSON"))
            > limits.serialized_bytes
        {
            diagnostics.report(ast::Pos::start(), "budget exceeded: serializedBytes");
        } else {
            program = Some(validate::validate(&statements, &mut diagnostics, limits));
        }
    }
    diagnostics.items.sort_by_key(|d| (d.line, d.column));
    Compilation {
        program: if diagnostics.has_errors {
            None
        } else {
            program
        },
        statements,
        diagnostics: diagnostics.items,
    }
}
