//! Bounded formulas and conditions.

pub(crate) mod eval;
pub(crate) mod functions;
pub(crate) mod math;
pub(crate) mod parse;

pub use eval::{EvalContext, QueryRowContext, Row, evaluate};
pub use parse::parse_expression;

use crate::{Diagnostic, Value, lines::Site};

/// A compiled formula, reusable across items and queries.
#[derive(Clone, Debug)]
pub struct Expression {
    pub(crate) root: Node,
}

impl Expression {
    /// Return whether the formula can be evaluated with only stored or
    /// computed row fields and a clock. The caller supplies the available
    /// field names and must also check every computed formula it evaluates.
    pub fn uses_only_row_and_clock(&self, field: impl Fn(&str) -> bool) -> bool {
        fn local(node: &Node, field: &impl Fn(&str) -> bool) -> bool {
            match &node.kind {
                NodeKind::Value(_) => true,
                NodeKind::Name(name) => {
                    !matches!(
                        name.as_str(),
                        "this" | "item" | "open item" | "records" | "state" | "settings" | "period"
                    ) && field(name)
                }
                NodeKind::Member(base, name) => {
                    matches!(&base.kind,
                    NodeKind::Name(scope) if scope == "this")
                        && field(name)
                }
                NodeKind::Call(name, args) => {
                    !matches!(
                        name.as_str(),
                        "from" | "all" | "previous" | "links to" | "records after"
                    ) && args.iter().all(|arg| local(arg, field))
                }
                NodeKind::List(values) => values.iter().all(|value| local(value, field)),
                NodeKind::Unary(_, value) => local(value, field),
                NodeKind::Binary(_, left, right) => local(left, field) && local(right, field),
            }
        }
        local(&self.root, &field)
    }

    /// Borrow the value when the expression is a single literal. No evaluation
    /// or context lookup is performed.
    pub fn literal(&self) -> Option<&Value> {
        match &self.root.kind {
            NodeKind::Value(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub kind: NodeKind,
    pub site: Site,
}

#[derive(Clone, Debug)]
pub(crate) enum NodeKind {
    Value(Value),
    Name(String),
    Member(Box<Node>, String),
    Call(String, Vec<Node>),
    List(Vec<Node>),
    Unary(Op, Box<Node>),
    Binary(Op, Box<Node>, Box<Node>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Op {
    Or,
    And,
    Not,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    Add,
    Sub,
    Mul,
    Div,
}

pub(crate) fn formula_scope(
    expression: &Expression,
    per: bool,
    totals: bool,
) -> Result<(), Diagnostic> {
    walk(&expression.root, &mut |node| {
        let invalid = |message| Err(node.site.error(message));
        match &node.kind {
            NodeKind::Call(name, _) if name == "all" && !totals => {
                invalid("Use all() only in a named total")
            }
            NodeKind::Call(name, _) if name == "previous" && !per => {
                invalid("Use previous() only in a per-period formula")
            }
            NodeKind::Name(name) if name == "period" && !per => {
                invalid("Use period only in a per-period formula")
            }
            _ => Ok(()),
        }
    })
}

pub(crate) fn walk(
    node: &Node,
    check: &mut impl FnMut(&Node) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    check(node)?;
    match &node.kind {
        NodeKind::Call(_, nodes) | NodeKind::List(nodes) => {
            for node in nodes {
                walk(node, check)?;
            }
        }
        NodeKind::Binary(_, a, b) => {
            walk(a, check)?;
            walk(b, check)?;
        }
        NodeKind::Member(base, _) | NodeKind::Unary(_, base) => walk(base, check)?,
        NodeKind::Name(_) | NodeKind::Value(_) => {}
    }
    Ok(())
}
