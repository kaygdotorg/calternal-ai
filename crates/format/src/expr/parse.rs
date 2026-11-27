use super::{Expression, Node, NodeKind, Op};
use crate::{
    Diagnostic, Value, dates,
    lines::{self, Site},
};

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Word(String),
    Function(String),
    Value(Value),
    Symbol(char),
    Operator(Op),
    End,
}

/// Compile a formula with rev 8 precedence, reporting its first invalid position.
pub fn parse_expression(source: &str) -> Result<Expression, Diagnostic> {
    parse(source, Site::START)
}

pub(crate) fn parse(source: &str, site: Site) -> Result<Expression, Diagnostic> {
    parse_at(source, |offset| site.after(&source[..offset]))
}

pub(crate) fn ordinary(source: &str, site: Site) -> Result<Expression, Diagnostic> {
    ordinary_at(source, |offset| site.after(&source[..offset]))
}

pub(crate) fn ordinary_at(
    source: &str,
    position: impl Fn(usize) -> Site,
) -> Result<Expression, Diagnostic> {
    let expression = parse_at(source, position)?;
    super::formula_scope(&expression, false, false)?;
    Ok(expression)
}

pub(crate) fn parse_at(
    source: &str,
    position: impl Fn(usize) -> Site,
) -> Result<Expression, Diagnostic> {
    if source.len() > 4096 {
        return Err(position(0).error("Keep the formula within 4 KiB"));
    }
    let mut parser = Parser {
        tokens: lex(source, &position)?,
        index: 0,
    };
    let (root, _) = parser.expression(1, 0)?;
    if parser.peek() != &Token::End {
        return Err(parser.site().error("Unexpected text after the formula"));
    }
    paths(&root, false, false)?;
    Ok(Expression { root })
}

fn lex(source: &str, position: &impl Fn(usize) -> Site) -> Result<Vec<(Token, Site)>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut start = 0;
    while start < source.len() {
        let rest = &source[start..];
        let ch = rest.chars().next().unwrap();
        let site = position(start);
        if ch == ' ' {
            start += 1;
            continue;
        }
        let (token, length) = match ch {
            '"' => {
                let mut chars = rest[1..].char_indices();
                let end = loop {
                    match chars.next() {
                        Some((index, '"')) => break index + 2,
                        Some((_, '\\')) => {
                            chars.next();
                        }
                        Some(_) => {}
                        None => return Err(site.error("Close the quoted formula text")),
                    }
                };
                (
                    Token::Value(Value::Text(lines::unquote(&rest[..end], site)?)),
                    end,
                )
            }
            '[' => {
                let end = rest
                    .find(']')
                    .ok_or_else(|| site.error("Close the bracketed name"))?;
                let name = &rest[1..end];
                if name.is_empty() || name.contains(['[', '"', '\n']) {
                    return Err(site.error("Write a name inside the brackets"));
                }
                (Token::Word(name.into()), end + 1)
            }
            '0'..='9' => number(rest, site)?,
            ch if ch.is_alphabetic() => word(rest),
            '(' | ')' | ',' | '.' | ':' => (Token::Symbol(ch), 1),
            _ => operator(rest)
                .ok_or_else(|| site.error(format!("Unexpected character in formula: {ch}")))?,
        };
        tokens.push((token, site));
        start += length;
    }
    tokens.push((Token::End, position(source.len())));
    Ok(tokens)
}

fn number(text: &str, site: Site) -> Result<(Token, usize), Diagnostic> {
    if let Some(candidate) = text.get(..10).filter(|date| {
        date.bytes().enumerate().all(|(offset, byte)| {
            if matches!(offset, 4 | 7) {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
    }) {
        dates::day(candidate)
            .ok_or_else(|| site.error("Use a valid date from 1900 through 2200"))?;
        return Ok((Token::Value(Value::Date(candidate.into())), 10));
    }
    let mut end = text.bytes().take_while(u8::is_ascii_digit).count();
    if text.as_bytes().get(end) == Some(&b'.')
        && text.as_bytes().get(end + 1).is_some_and(u8::is_ascii_digit)
    {
        end += 1 + text[end + 1..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
    }
    let unit_len = text[end..]
        .bytes()
        .take_while(u8::is_ascii_alphabetic)
        .count();
    if unit_len > 0 {
        let end = end + unit_len;
        let value = dates::duration(&text[..end])
            .ok_or_else(|| site.error("Use a duration such as 2min, 2h or 2mo"))?;
        return Ok((Token::Value(value), end));
    }
    let amount = text[..end]
        .parse()
        .map_err(|error: crate::InvalidDecimal| site.error(error.to_string()))?;
    if let Some(currency) = text[end..]
        .strip_prefix(' ')
        .and_then(|tail| tail.get(..3))
        .filter(|code| code.bytes().all(|byte| byte.is_ascii_uppercase()))
        .filter(|_| {
            text.as_bytes()
                .get(end + 4)
                .is_none_or(|byte| !byte.is_ascii_alphanumeric())
        })
    {
        return Ok((
            Token::Value(Value::Money {
                amount,
                currency: currency.into(),
            }),
            end + 4,
        ));
    }
    Ok((Token::Value(Value::Number(amount)), end))
}

fn word(text: &str) -> (Token, usize) {
    let single = |text: &str| {
        text.char_indices()
            .find(|(_, ch)| !ch.is_alphanumeric())
            .map_or(text.len(), |(at, _)| at)
    };
    if text.starts_with("links to ") {
        return (Token::Word("links to".into()), "links to".len());
    }
    if text.starts_with("records after ") {
        return (Token::Word("records after".into()), "records after".len());
    }
    let mut end = single(text);
    let first = &text[..end];
    let keyword = match first {
        "and" => Some(Token::Operator(Op::And)),
        "or" => Some(Token::Operator(Op::Or)),
        "not" => Some(Token::Operator(Op::Not)),
        "in" => Some(Token::Operator(Op::In)),
        "yes" | "true" => Some(Token::Value(Value::Bool(true))),
        "no" | "false" => Some(Token::Value(Value::Bool(false))),
        "empty" => Some(Token::Value(Value::Empty)),
        _ => None,
    };
    if let Some(keyword) = keyword {
        return (keyword, end);
    }
    if text[end..].starts_with('(') {
        return (Token::Function(first.into()), end);
    }
    while let Some(tail) = text[end..]
        .strip_prefix(' ')
        .filter(|tail| tail.starts_with(char::is_alphabetic))
    {
        let length = single(tail);
        if ["and", "or", "not", "in"].contains(&&tail[..length]) || tail[length..].starts_with('(')
        {
            break;
        }
        end += 1 + length;
    }
    (Token::Word(text[..end].into()), end)
}

fn operator(text: &str) -> Option<(Token, usize)> {
    [
        ("!=", Op::Ne),
        ("<=", Op::Le),
        (">=", Op::Ge),
        ("=", Op::Eq),
        ("<", Op::Lt),
        (">", Op::Gt),
        ("+", Op::Add),
        ("-", Op::Sub),
        ("*", Op::Mul),
        ("/", Op::Div),
    ]
    .into_iter()
    .find(|(symbol, _)| text.starts_with(symbol))
    .map(|(symbol, op)| (Token::Operator(op), symbol.len()))
}

struct Parser {
    tokens: Vec<(Token, Site)>,
    index: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.index].0
    }
    fn site(&self) -> Site {
        self.tokens[self.index].1
    }

    fn advance(&mut self) -> (Token, Site) {
        let current = &mut self.tokens[self.index];
        if current.0 == Token::End {
            return (Token::End, current.1);
        }
        self.index += 1;
        (std::mem::replace(&mut current.0, Token::End), current.1)
    }

    fn expression(&mut self, minimum: u8, nesting: usize) -> Result<(Node, usize), Diagnostic> {
        if nesting >= 64 {
            return Err(self.site().error("Keep formula nesting within 64 levels"));
        }
        let (token, site) = self.advance();
        let (mut left, mut depth) = match token {
            Token::Value(value) => (
                Node {
                    kind: NodeKind::Value(value),
                    site,
                },
                1,
            ),
            Token::Word(name) if name == "links to" => {
                let (target, depth) = self.expression(8, nesting + 1)?;
                (
                    Node {
                        kind: NodeKind::Call(name, vec![target]),
                        site,
                    },
                    depth + 1,
                )
            }
            Token::Word(name) if name == "records after" => {
                let (position, depth) = self.expression(8, nesting + 1)?;
                if !matches!(&position.kind, NodeKind::Member(scope, _) if matches!(&scope.kind, NodeKind::Name(name) if name == "state"))
                {
                    return Err(site.error("Count records after a state anchor"));
                }
                (
                    Node {
                        kind: NodeKind::Call(name, vec![position]),
                        site,
                    },
                    depth + 1,
                )
            }
            Token::Word(name) => (
                Node {
                    kind: NodeKind::Name(name),
                    site,
                },
                1,
            ),
            Token::Function(name) => {
                let (min, max) = arity(&name)
                    .ok_or_else(|| site.error(format!("Unknown formula function {name}")))?;
                self.advance();
                let (args, depth) = self.arguments(nesting + 1)?;
                if !(min..=max).contains(&args.len()) {
                    return Err(site.error(format!("Check the number of arguments to {name}")));
                }
                (
                    Node {
                        kind: NodeKind::Call(name, args),
                        site,
                    },
                    depth + 1,
                )
            }
            Token::Operator(op @ (Op::Not | Op::Sub)) => {
                let (node, depth) =
                    self.expression(if op == Op::Not { 4 } else { 8 }, nesting + 1)?;
                (
                    Node {
                        kind: NodeKind::Unary(op, Box::new(node)),
                        site,
                    },
                    depth + 1,
                )
            }
            Token::Symbol('(') => {
                let value = self.expression(1, nesting + 1)?;
                self.close()?;
                value
            }
            _ => return Err(site.error("Write a value, field or function here")),
        };
        loop {
            let site = self.site();
            if self.peek() == &Token::Symbol('.') {
                self.advance();
                let (Token::Word(field), field_site) = self.advance() else {
                    return Err(site.error("Write a field name after the dot"));
                };
                if depth >= 64 {
                    return Err(site.error("Keep formula nesting within 64 levels"));
                }
                left = Node {
                    kind: NodeKind::Member(Box::new(left), field),
                    site: field_site,
                };
                depth += 1;
                continue;
            }
            let Token::Operator(operator) = self.peek() else {
                break;
            };
            let operator = *operator;
            let precedence = match operator {
                Op::Or => 1,
                Op::And => 2,
                Op::Not => break,
                Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::In => 4,
                Op::Add | Op::Sub => 5,
                Op::Mul | Op::Div => 6,
            };
            if precedence < minimum {
                break;
            }
            self.advance();
            let (right, right_depth) = if operator == Op::In {
                if self.advance().0 != Token::Symbol('(') {
                    return Err(site.error("Put the membership values in parentheses"));
                }
                let (values, depth) = self.arguments(nesting + 1)?;
                (
                    Node {
                        kind: NodeKind::List(values),
                        site,
                    },
                    depth + 1,
                )
            } else {
                self.expression(precedence + 1, nesting + 1)?
            };
            depth = depth.max(right_depth) + 1;
            if depth > 64 {
                return Err(site.error("Keep formula nesting within 64 levels"));
            }
            left = Node {
                kind: NodeKind::Binary(operator, Box::new(left), Box::new(right)),
                site,
            };
        }
        Ok((left, depth))
    }

    fn arguments(&mut self, depth: usize) -> Result<(Vec<Node>, usize), Diagnostic> {
        let mut values = Vec::new();
        let mut max_depth = 0;
        if self.peek() == &Token::Symbol(')') {
            self.close()?;
            return Ok((values, max_depth));
        }
        loop {
            if matches!(self.peek(), Token::Word(name) if name == "after")
                && self
                    .tokens
                    .get(self.index + 1)
                    .is_some_and(|token| token.0 == Token::Symbol(':'))
            {
                self.advance();
                self.advance();
            }
            let (value, value_depth) = self.expression(1, depth)?;
            values.push(value);
            max_depth = max_depth.max(value_depth);
            if self.peek() != &Token::Symbol(',') {
                break;
            }
            self.advance();
        }
        self.close()?;
        Ok((values, max_depth))
    }

    fn close(&mut self) -> Result<(), Diagnostic> {
        let (token, site) = self.advance();
        if token == Token::Symbol(')') {
            Ok(())
        } else {
            Err(site.error("Close the expression with )"))
        }
    }
}

fn paths(node: &Node, from_target: bool, after_target: bool) -> Result<(), Diagnostic> {
    match &node.kind {
        NodeKind::Member(base, _) => {
            if let NodeKind::Member(root, _) = &base.kind {
                let scope = matches!(&root.kind, NodeKind::Name(name) if matches!(name.as_str(), "this" | "item" | "open item"));
                if !scope && !from_target {
                    return Err(node
                        .site
                        .error("Follow at most one link or read one data field"));
                }
            }
            paths(base, from_target, false)?;
        }
        NodeKind::Unary(_, value) => paths(value, false, false)?,
        NodeKind::Binary(_, a, b) => {
            paths(a, false, false)?;
            paths(b, false, false)?;
        }
        NodeKind::Call(name, args) => {
            if name == "from" && !matches!(&args[0].kind, NodeKind::Member(..)) {
                return Err(node.site.error("Name the kind and its link field in from"));
            }
            if name == "all" {
                let target = super::eval::reference_key(&args[0])
                    .ok_or_else(|| node.site.error("Name a kind in all()"))?;
                let parts: Vec<_> = target.split('.').collect();
                if parts.len() > 2
                    || parts
                        .iter()
                        .any(|part| crate::lines::id(part, node.site).is_err())
                    || crate::lines::kind_id(parts.last().unwrap(), node.site).is_err()
                {
                    return Err(node.site.error("Name a local or qualified kind in all()"));
                }
            }
            if name == "records after" && !after_target {
                return Err(node
                    .site
                    .error("Use records after as the only argument to count"));
            }
            for (index, arg) in args.iter().enumerate() {
                paths(
                    arg,
                    name == "from" && index == 0,
                    name == "count" && args.len() == 1,
                )?;
            }
        }
        NodeKind::List(items) => {
            for item in items {
                paths(item, false, false)?
            }
        }
        _ => {}
    }
    Ok(())
}

fn arity(function: &str) -> Option<(usize, usize)> {
    Some(match function {
        "today" | "now" => (0, 0),
        "if" => (3, 3),
        "contains" | "under" | "days" | "format" => (2, 2),
        "first" => (1, usize::MAX),
        "from" => (1, 3),
        "all" => (1, 2),
        "sum" | "count" | "min" | "max" | "average" | "next" | "round" => (1, 2),
        "previous" | "last" | "date" | "week" | "month" | "year" | "domain" | "streak" => (1, 1),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appendix_formula_lexing() {
        for source in [
            r#"contains(subject, "say \"hi\"")"#,
            "sum(from([calternal-calendar.events].task, logged).length)",
            r#"priority in ("urgent", "high")"#,
            "now() + 1mo",
            "next(repeat, after: last(log.done))",
            "not state.read through and unread > 0",
        ] {
            assert!(parse_expression(source).is_ok(), "{source}");
        }
        assert!(parse_expression("2m").is_err());
        assert!(parse_expression("unknown(2)").is_err());
        assert!(parse_expression(&"1 + ".repeat(64)).is_err());
    }
}
