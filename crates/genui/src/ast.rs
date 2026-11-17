//! Serializable syntax shared with TypeScript. Positions use one-based UTF-16 columns.
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A source location; line/column count from one, columns in UTF-16 code units.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Pos {
    /// One-based source line.
    pub line: usize,
    /// One-based UTF-16 source column.
    pub column: usize,
}
impl Pos {
    /// Start of a description. Constant time/space; no allocation, locks or I/O.
    pub const fn start() -> Self {
        Self { line: 1, column: 1 }
    }
}
/// A source-positioned syntax or evaluation error. Errors prevent activation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Diagnostic {
    /// Currently `error`; warnings are reserved for non-blocking diagnostics.
    pub severity: String,
    /// One-based line of the expression that failed.
    pub line: usize,
    /// One-based UTF-16 column of the expression that failed.
    pub column: usize,
    /// Deterministic explanation shared with TypeScript.
    pub message: String,
}
/// One unevaluated expression. This is data, never executable platform code.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Expr {
    /// Location used for diagnostics and native component identity.
    pub pos: Pos,
    /// Tagged syntax payload, flattened to the TypeScript AST JSON shape.
    #[serde(flatten)]
    pub kind: ExprKind,
}
/// A named call argument and its source location.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NamedArg {
    /// Catalog argument name.
    pub name: String,
    /// Unevaluated argument expression.
    pub value: Expr,
    /// Position of the argument name.
    pub pos: Pos,
}
/// An object entry, kept in insertion order rather than sorted by key.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Entry {
    /// Literal key, with prototype-related names rejected by validation.
    pub key: String,
    /// Unevaluated entry expression.
    pub value: Expr,
    /// Position of the key.
    pub pos: Pos,
}
/// Expression alternatives from the upstream language. Callbacks stay inside the interpreter.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ExprKind {
    /// A JSON scalar literal.
    Literal {
        /// Scalar value; structured literal values are rejected.
        value: Value,
    },
    /// A template with one more literal segment than expression parts.
    Template {
        /// Literal segments in source order.
        quasis: Vec<String>,
        /// Interpolated expressions.
        parts: Vec<Expr>,
    },
    /// A local state read.
    State {
        /// Name without `$`.
        name: String,
    },
    /// A lexical parameter or named top-level value.
    Ident {
        /// Identifier text.
        name: String,
    },
    /// An ordered list literal.
    List {
        /// List element expressions.
        items: Vec<Expr>,
    },
    /// A JSON object literal.
    Object {
        /// Source-ordered key/value entries.
        entries: Vec<Entry>,
    },
    /// A logical or arithmetic prefix expression.
    Unary {
        /// `!` or `-`.
        op: String,
        /// Expression being negated.
        operand: Box<Expr>,
    },
    /// A pure binary operation, including lazy logical operators.
    Binary {
        /// Whitelisted operator spelling.
        op: String,
        /// Left operand, evaluated first.
        left: Box<Expr>,
        /// Right operand, evaluated only when needed.
        right: Box<Expr>,
    },
    /// A lazy conditional expression.
    Conditional {
        /// Truth-tested expression.
        test: Box<Expr>,
        /// Branch for truthy tests.
        then: Box<Expr>,
        /// Branch for false tests.
        r#else: Box<Expr>,
    },
    /// A property read on JSON data.
    Member {
        /// Receiver expression.
        object: Box<Expr>,
        /// Property name.
        name: String,
    },
    /// An indexed list, string or object read.
    Index {
        /// Receiver expression.
        object: Box<Expr>,
        /// Numeric index or string key.
        index: Box<Expr>,
    },
    /// A catalog component, action, pure function or named-function call.
    Call {
        /// Whitelisted or declared callee.
        name: String,
        /// Positional arguments.
        args: Vec<Expr>,
        /// Named arguments.
        named: Vec<NamedArg>,
    },
    /// A whitelisted pure method call.
    Method {
        /// Receiver expression.
        object: Box<Expr>,
        /// Method name.
        name: String,
        /// Positional arguments.
        args: Vec<Expr>,
        /// Named arguments (diagnosed and rejected).
        named: Vec<NamedArg>,
    },
    /// A named function or list-method callback; never exported as a runtime function.
    Arrow {
        /// Parameter names.
        params: Vec<String>,
        /// Pure callback expression.
        body: Box<Expr>,
    },
}
/// A state or named definition.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Decl {
    /// Name without a state prefix.
    pub name: String,
    /// Initializer or named expression.
    pub value: Expr,
    /// Position of the declared name.
    pub pos: Pos,
}
/// Source-level statement. Names and states occupy separate namespaces.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Statement {
    /// A local state initializer.
    State {
        /// Flattened definition.
        #[serde(flatten)]
        decl: Decl,
    },
    /// A top-level named expression or function.
    Named {
        /// Flattened definition.
        #[serde(flatten)]
        decl: Decl,
    },
    /// A bare expression whose value becomes native root descriptions.
    Root {
        /// Root expression.
        value: Expr,
    },
}
/// Validated program. Create through [`crate::compile`]; clients do not deserialize untrusted ASTs.
#[derive(Clone, Debug)]
pub struct Program {
    pub(crate) serialized_bytes: usize,
    pub(crate) states: Vec<Decl>,
    pub(crate) named: Vec<Decl>,
    pub(crate) roots: Vec<Expr>,
}
impl Expr {
    pub(crate) fn children(&self) -> Vec<&Expr> {
        match &self.kind {
            ExprKind::Literal { .. } | ExprKind::State { .. } | ExprKind::Ident { .. } => vec![],
            ExprKind::Template { parts, .. } => parts.iter().collect(),
            ExprKind::List { items } => items.iter().collect(),
            ExprKind::Object { entries } => entries.iter().map(|e| &e.value).collect(),
            ExprKind::Unary { operand, .. } => vec![operand],
            ExprKind::Binary { left, right, .. } => vec![left, right],
            ExprKind::Conditional { test, then, r#else } => vec![test, then, r#else],
            ExprKind::Member { object, .. } => vec![object],
            ExprKind::Index { object, index } => vec![object, index],
            ExprKind::Arrow { body, .. } => vec![body],
            ExprKind::Call { args, named, .. } => {
                args.iter().chain(named.iter().map(|n| &n.value)).collect()
            }
            ExprKind::Method {
                object,
                args,
                named,
                ..
            } => std::iter::once(object.as_ref())
                .chain(args.iter())
                .chain(named.iter().map(|n| &n.value))
                .collect(),
        }
    }
}
