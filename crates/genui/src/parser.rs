//! Recursive-descent parser ported from the credited upstream TypeScript implementation.
use crate::{
    ast::{Decl, Entry, Expr, ExprKind as K, NamedArg, Statement},
    error::{Diagnostics, Failure},
    lexer::Token,
};
use serde_json::json;
type Result<T> = std::result::Result<T, Failure>;
fn precedence(op: &str) -> usize {
    match op {
        "??" => 1,
        "||" => 2,
        "&&" => 3,
        "==" | "!=" | "===" | "!==" => 4,
        "<" | "<=" | ">" | ">=" => 5,
        "+" | "-" => 6,
        "*" | "/" | "%" => 7,
        _ => 0,
    }
}
fn keyword(name: &str) -> bool {
    matches!(name, "true" | "false" | "null")
}
pub(crate) fn parse(tokens: &[Token], diagnostics: &mut Diagnostics) -> Vec<Statement> {
    let mut parser = Parser {
        tokens,
        i: 0,
        depth: 0,
    };
    let mut statements = vec![];
    loop {
        while parser.peek(0).t == "sep" {
            parser.i += 1;
        }
        if parser.peek(0).t == "eof" {
            return statements;
        }
        match parser.statement() {
            Ok(s) => statements.push(s),
            Err(e) => {
                diagnostics.report(e.pos, e.message);
                while !matches!(parser.peek(0).t, "sep" | "eof") {
                    parser.i += 1;
                }
            }
        }
    }
}
struct Parser<'a> {
    tokens: &'a [Token],
    i: usize,
    depth: usize,
}
impl Parser<'_> {
    fn peek(&self, offset: usize) -> &Token {
        &self.tokens[(self.i + offset).min(self.tokens.len() - 1)]
    }
    fn punct(&self, s: &str, offset: usize) -> bool {
        let t = self.peek(offset);
        t.t == "punct" && t.text() == s
    }
    fn eat(&mut self, s: &str) -> bool {
        if self.punct(s, 0) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn close(&mut self, closer: &str, open: &Token, alternatives: &str) -> Result<()> {
        if self.eat(closer) {
            return Ok(());
        }
        let opener = if open.t == "punct" { open.text() } else { "(" };
        Err(Failure::new(
            self.peek(0).pos,
            format!(
                "expected {alternatives}`{closer}` to close `{opener}` from {}:{}, found {}",
                open.pos.line,
                open.pos.column,
                self.peek(0).describe()
            ),
        ))
    }
    fn ident(&mut self, what: &str) -> Result<Token> {
        let t = self.peek(0).clone();
        if t.t != "ident" {
            return Err(Failure::new(
                t.pos,
                format!("expected {what}, found {}", t.describe()),
            ));
        }
        self.i += 1;
        Ok(t)
    }
    fn nest(&mut self, at: &Token) -> Result<()> {
        self.depth += 1;
        if self.depth > 100 {
            Err(Failure::new(
                at.pos,
                "expression is nested too deeply; split it into named values",
            ))
        } else {
            Ok(())
        }
    }
    fn statement(&mut self) -> Result<Statement> {
        self.depth = 0;
        let first = self.peek(0).clone();
        let s = if matches!(first.t, "state" | "ident") && self.punct("=", 1) {
            if first.t == "ident" && keyword(first.text()) {
                return Err(Failure::new(
                    first.pos,
                    format!("`{}` is a keyword and cannot be assigned", first.text()),
                ));
            }
            self.i += 2;
            let decl = Decl {
                name: first.text().into(),
                value: self.expr()?,
                pos: first.pos,
            };
            if first.t == "state" {
                Statement::State { decl }
            } else {
                Statement::Named { decl }
            }
        } else {
            Statement::Root {
                value: self.expr()?,
            }
        };
        let next = self.peek(0);
        if !matches!(next.t, "sep" | "eof") {
            return Err(Failure::new(
                next.pos,
                format!(
                    "expected a new line or `;` after the statement, found {}; wrap multi-line expressions in brackets",
                    next.describe()
                ),
            ));
        }
        Ok(s)
    }
    fn expr(&mut self) -> Result<Expr> {
        let saved = self.depth;
        self.nest(&self.peek(0).clone())?;
        let e = if self.arrow_start() {
            self.arrow()
        } else {
            self.conditional()
        };
        self.depth = saved;
        e
    }
    fn arrow_start(&self) -> bool {
        if self.peek(0).t == "ident" {
            return self.punct("=>", 1);
        }
        if !self.punct("(", 0) {
            return false;
        }
        let mut offset = 1;
        if self.peek(offset).t == "ident" {
            offset += 1;
            while self.punct(",", offset) && self.peek(offset + 1).t == "ident" {
                offset += 2;
            }
        }
        self.punct(")", offset) && self.punct("=>", offset + 1)
    }
    fn arrow(&mut self) -> Result<Expr> {
        let start = self.peek(0).pos;
        let mut params = vec![];
        if self.eat("(") {
            while !self.eat(")") {
                params.push(self.ident("a parameter name")?.text().to_string());
                self.eat(",");
            }
        } else {
            params.push(self.ident("a parameter name")?.text().to_string());
        }
        if let Some(name) = params.iter().find(|name| keyword(name)) {
            return Err(Failure::new(
                start,
                format!("`{name}` cannot be a parameter name"),
            ));
        }
        self.i += 1;
        Ok(Expr {
            pos: start,
            kind: K::Arrow {
                params,
                body: Box::new(self.expr()?),
            },
        })
    }
    fn conditional(&mut self) -> Result<Expr> {
        let test = self.binary(1)?;
        let question = self.peek(0).clone();
        if !self.eat("?") {
            return Ok(test);
        }
        let then = self.expr()?;
        if !self.eat(":") {
            return Err(Failure::new(
                self.peek(0).pos,
                format!(
                    "expected `:` for the `?` at {}:{}, found {}",
                    question.pos.line,
                    question.pos.column,
                    self.peek(0).describe()
                ),
            ));
        }
        Ok(Expr {
            pos: question.pos,
            kind: K::Conditional {
                test: Box::new(test),
                then: Box::new(then),
                r#else: Box::new(self.expr()?),
            },
        })
    }
    fn binary(&mut self, min: usize) -> Result<Expr> {
        let saved = self.depth;
        let mut left = self.unary()?;
        loop {
            let t = self.peek(0).clone();
            let p = if t.t == "punct" {
                precedence(t.text())
            } else {
                0
            };
            if p < min {
                break;
            }
            self.nest(&t)?;
            self.i += 1;
            let right = self.binary(p + 1)?;
            left = Expr {
                pos: t.pos,
                kind: K::Binary {
                    op: t.text().into(),
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        self.depth = saved;
        Ok(left)
    }
    fn unary(&mut self) -> Result<Expr> {
        let saved = self.depth;
        let mut ops = vec![];
        while self.punct("!", 0) || self.punct("-", 0) {
            let t = self.peek(0).clone();
            self.nest(&t)?;
            self.i += 1;
            ops.push(t);
        }
        let mut e = self.postfix()?;
        if self.punct("**", 0) {
            let t = self.peek(0).clone();
            self.nest(&t)?;
            self.i += 1;
            e = Expr {
                pos: t.pos,
                kind: K::Binary {
                    op: "**".into(),
                    left: Box::new(e),
                    right: Box::new(self.unary()?),
                },
            };
        }
        for t in ops.into_iter().rev() {
            e = Expr {
                pos: t.pos,
                kind: K::Unary {
                    op: t.text().into(),
                    operand: Box::new(e),
                },
            };
        }
        self.depth = saved;
        Ok(e)
    }
    fn postfix(&mut self) -> Result<Expr> {
        let mut e = self.primary()?;
        loop {
            if self.punct(".", 0) || self.punct("[", 0) || self.punct("(", 0) {
                self.nest(&self.peek(0).clone())?;
            }
            if self.eat(".") {
                let name = self.ident("a property or method name after `.`")?;
                if self.punct("(", 0) {
                    let (args, named) = self.args()?;
                    let k = if matches!(&e.kind,K::Ident{name} if name=="Math"||name=="Object") {
                        K::Call {
                            name: name.text().into(),
                            args,
                            named,
                        }
                    } else {
                        K::Method {
                            object: Box::new(e),
                            name: name.text().into(),
                            args,
                            named,
                        }
                    };
                    e = Expr {
                        pos: name.pos,
                        kind: k,
                    };
                } else {
                    e = Expr {
                        pos: name.pos,
                        kind: K::Member {
                            object: Box::new(e),
                            name: name.text().into(),
                        },
                    };
                }
            } else if self.punct("[", 0) {
                let open = self.peek(0).clone();
                self.i += 1;
                let index = self.expr()?;
                self.close("]", &open, "")?;
                e = Expr {
                    pos: open.pos,
                    kind: K::Index {
                        object: Box::new(e),
                        index: Box::new(index),
                    },
                };
            } else if self.punct("(", 0) {
                let name = if let K::Ident { name } = &e.kind {
                    name.clone()
                } else {
                    return Err(Failure::new(
                        self.peek(0).pos,
                        "only components, actions, functions and methods can be called",
                    ));
                };
                let (args, named) = self.args()?;
                e = Expr {
                    pos: e.pos,
                    kind: K::Call { name, args, named },
                };
            } else {
                return Ok(e);
            }
        }
    }
    fn args(&mut self) -> Result<(Vec<Expr>, Vec<NamedArg>)> {
        let open = self.peek(0).clone();
        self.i += 1;
        let mut args = vec![];
        let mut named = vec![];
        while !self.eat(")") {
            let t = self.peek(0).clone();
            if t.t == "ident" && self.punct(":", 1) {
                self.i += 2;
                named.push(NamedArg {
                    name: t.text().into(),
                    value: self.expr()?,
                    pos: t.pos,
                });
            } else {
                if !named.is_empty() {
                    return Err(Failure::new(
                        t.pos,
                        "positional arguments must come before named ones",
                    ));
                }
                args.push(self.expr()?);
            }
            if !self.eat(",") {
                self.close(")", &open, "`,` or ")?;
                break;
            }
        }
        Ok((args, named))
    }
    fn primary(&mut self) -> Result<Expr> {
        let t = self.peek(0).clone();
        let pos = t.pos;
        let kind = match t.t {
            "num" | "str" => {
                self.i += 1;
                K::Literal { value: t.value }
            }
            "ident" => {
                self.i += 1;
                if keyword(t.text()) {
                    K::Literal {
                        value: match t.text() {
                            "true" => json!(true),
                            "false" => json!(false),
                            _ => json!(null),
                        },
                    }
                } else {
                    K::Ident {
                        name: t.text().into(),
                    }
                }
            }
            "state" => {
                self.i += 1;
                K::State {
                    name: t.text().into(),
                }
            }
            "tpl" => {
                self.i += 1;
                let mut parts = vec![];
                for tokens in &t.parts {
                    let mut sub = Parser {
                        tokens,
                        i: 0,
                        depth: self.depth,
                    };
                    let part = sub.expr()?;
                    if sub.peek(0).t != "eof" {
                        return Err(Failure::new(
                            sub.peek(0).pos,
                            format!(
                                "expected `}}` to end the template expression, found {}",
                                sub.peek(0).describe()
                            ),
                        ));
                    }
                    parts.push(part);
                }
                K::Template {
                    quasis: t.quasis,
                    parts,
                }
            }
            "punct" if t.text() == "(" => {
                self.i += 1;
                let e = self.expr()?;
                self.close(")", &t, "")?;
                return Ok(e);
            }
            "punct" if t.text() == "[" => {
                self.i += 1;
                let mut items = vec![];
                while !self.eat("]") {
                    items.push(self.expr()?);
                    if !self.eat(",") {
                        self.close("]", &t, "`,` or ")?;
                        break;
                    }
                }
                K::List { items }
            }
            "punct" if t.text() == "{" => return self.object(&t),
            _ => {
                return Err(Failure::new(
                    pos,
                    format!("expected an expression, found {}", t.describe()),
                ));
            }
        };
        Ok(Expr { pos, kind })
    }
    fn object(&mut self, open: &Token) -> Result<Expr> {
        self.i += 1;
        let mut entries = vec![];
        while !self.eat("}") {
            let key = self.peek(0).clone();
            if !matches!(key.t, "ident" | "str") {
                return Err(Failure::new(
                    key.pos,
                    format!(
                        "expected an object key (a name or a string), found {}",
                        key.describe()
                    ),
                ));
            }
            self.i += 1;
            let mut name = key.text().to_string();
            if key.t == "ident" {
                while matches!(self.peek(0).t, "ident" | "num") {
                    let t = self.peek(0);
                    name.push(' ');
                    name.push_str(&if t.t == "num" {
                        t.value.as_f64().unwrap_or(0.0).to_string()
                    } else {
                        t.text().into()
                    });
                    self.i += 1;
                }
            }
            if !self.eat(":") {
                return Err(Failure::new(
                    self.peek(0).pos,
                    format!(
                        "expected `:` after the object key, found {}",
                        self.peek(0).describe()
                    ),
                ));
            }
            entries.push(Entry {
                key: name,
                value: self.expr()?,
                pos: key.pos,
            });
            if !self.eat(",") {
                self.close("}", open, "`,` or ")?;
                break;
            }
        }
        Ok(Expr {
            pos: open.pos,
            kind: K::Object { entries },
        })
    }
}
