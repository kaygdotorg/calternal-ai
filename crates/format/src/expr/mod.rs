//! Bounded formulas and conditions.

pub(crate) mod eval;
pub(crate) mod functions;
pub(crate) mod math;
pub(crate) mod parse;

pub use eval::{EvalContext, QueryRowContext, Row, evaluate};
pub use parse::parse_expression;

use crate::{Value, lines::Site};

/// A compiled formula, reusable across items and queries.
#[derive(Clone, Debug)]
pub struct Expression {
    pub(crate) root: Node,
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
