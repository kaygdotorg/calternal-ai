//! Lexer adapted from upstream, with UTF-16 positions and bounded template recursion.
use crate::{ast::Pos, error::Diagnostics};
use serde_json::{Value, json};
#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub t: &'static str,
    pub value: Value,
    pub pos: Pos,
    pub quasis: Vec<String>,
    pub parts: Vec<Vec<Token>>,
}
impl Token {
    fn new(t: &'static str, value: Value, pos: Pos) -> Self {
        Self {
            t,
            value,
            pos,
            quasis: vec![],
            parts: vec![],
        }
    }
    pub fn text(&self) -> &str {
        self.value.as_str().unwrap_or("")
    }
    pub fn describe(&self) -> String {
        match self.t {
            "num" => format!("number {}", self.value),
            "str" => "a string".into(),
            "tpl" => "a template string".into(),
            "ident" | "punct" => format!("`{}`", self.text()),
            "state" => format!("`${}`", self.text()),
            "sep" => {
                if self.text() == ";" {
                    "`;`".into()
                } else {
                    "end of line".into()
                }
            }
            _ => "end of input".into(),
        }
    }
}
const PUNCTUATION: &[&str] = &[
    "===", "!==", "==", "!=", "<=", ">=", "&&", "||", "??", "=>", "**", "(", ")", "[", "]", "{",
    "}", ",", ":", ".", "?", "<", ">", "+", "-", "*", "/", "%", "!", "=",
];
fn continues_after(s: &str) -> bool {
    matches!(
        s,
        "+" | "-"
            | "*"
            | "/"
            | "%"
            | "**"
            | "=="
            | "!="
            | "==="
            | "!=="
            | "<"
            | "<="
            | ">"
            | ">="
            | "&&"
            | "||"
            | "??"
            | "?"
            | ":"
            | "."
            | "="
            | "=>"
    )
}
fn continues_before(s: &str) -> bool {
    continues_after(s) && s != "-" && s != "="
}
fn ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}
fn ident_char(c: char) -> bool {
    ident_start(c) || c.is_ascii_digit()
}
pub(crate) fn tokenize(source: &str, diagnostics: &mut Diagnostics) -> Vec<Token> {
    let mut lexer = Lexer {
        src: source.chars().collect(),
        i: 0,
        pos: Pos::start(),
        diagnostics,
        templates: 0,
    };
    lexer.run(false)
}
struct Lexer<'a> {
    src: Vec<char>,
    i: usize,
    pos: Pos,
    diagnostics: &'a mut Diagnostics,
    templates: usize,
}
impl Lexer<'_> {
    fn c(&self) -> Option<char> {
        self.src.get(self.i).copied()
    }
    fn advance(&mut self, n: usize) {
        for _ in 0..n {
            if let Some(c) = self.c() {
                if c == '\n' {
                    self.pos.line += 1;
                    self.pos.column = 1;
                } else {
                    self.pos.column += c.len_utf16();
                }
                self.i += 1;
            }
        }
    }
    fn starts(&self, s: &str) -> bool {
        self.src[self.i..]
            .iter()
            .copied()
            .take(s.chars().count())
            .eq(s.chars())
    }
    fn blank(&mut self) {
        loop {
            match self.c() {
                Some(' ' | '\t' | '\r') => self.advance(1),
                Some('/') if self.starts("//") => {
                    while self.c().is_some_and(|c| c != '\n') {
                        self.advance(1);
                    }
                }
                _ => return,
            }
        }
    }
    fn ident(&mut self) -> Option<String> {
        if !self.c().is_some_and(ident_start) {
            return None;
        }
        let start = self.i;
        while self.c().is_some_and(ident_char) {
            self.advance(1);
        }
        Some(self.src[start..self.i].iter().collect())
    }
    fn num(&mut self) -> Option<String> {
        if !self.c().is_some_and(|c| c.is_ascii_digit()) {
            return None;
        }
        let start = self.i;
        self.advance(1);
        while self.c().is_some_and(|c| c.is_ascii_digit())
            || self.c() == Some('_') && self.src.get(self.i + 1).is_some_and(char::is_ascii_digit)
        {
            self.advance(1);
        }
        if self.c() == Some('.') && self.src.get(self.i + 1).is_some_and(char::is_ascii_digit) {
            self.advance(1);
            while self.c().is_some_and(|c| c.is_ascii_digit())
                || self.c() == Some('_')
                    && self.src.get(self.i + 1).is_some_and(char::is_ascii_digit)
            {
                self.advance(1);
            }
        }
        if matches!(self.c(), Some('e' | 'E')) {
            let saved = (self.i, self.pos);
            self.advance(1);
            if matches!(self.c(), Some('+' | '-')) {
                self.advance(1);
            }
            if self.c().is_some_and(|c| c.is_ascii_digit()) {
                while self.c().is_some_and(|c| c.is_ascii_digit()) {
                    self.advance(1);
                }
            } else {
                self.i = saved.0;
                self.pos = saved.1;
            }
        }
        Some(self.src[start..self.i].iter().collect())
    }
    fn run(&mut self, in_template: bool) -> Vec<Token> {
        let mut out = Vec::new();
        let mut depth = 0usize;
        loop {
            self.blank();
            let pos = self.pos;
            let Some(c) = self.c() else {
                if in_template {
                    self.diagnostics
                        .report(pos, "unterminated template: `${` is missing its `}`");
                }
                out.push(Token::new("eof", Value::Null, pos));
                return if in_template { out } else { join(out) };
            };
            if c == '\n' {
                self.advance(1);
                if !in_template && depth == 0 {
                    out.push(Token::new("sep", json!("\n"), pos));
                }
                continue;
            }
            if c == ';' {
                self.advance(1);
                out.push(Token::new("sep", json!(";"), pos));
                continue;
            }
            if in_template && c == '}' && depth == 0 {
                out.push(Token::new("eof", Value::Null, pos));
                self.advance(1);
                return out;
            }
            if let Some(num) = self.num() {
                if let Some(c) = self.c().filter(|c| ident_char(*c)) {
                    self.diagnostics
                        .report(pos, format!("invalid number `{num}{c}`"));
                }
                let n = num.replace('_', "").parse::<f64>().unwrap_or(f64::NAN);
                if !n.is_finite() {
                    self.diagnostics.report(pos, "number must be finite");
                }
                out.push(Token::new("num", json!(n), pos));
                continue;
            }
            if let Some(ident) = self.ident() {
                out.push(Token::new("ident", json!(ident), pos));
                continue;
            }
            if c == '$' {
                self.advance(1);
                if let Some(name) = self.ident() {
                    out.push(Token::new("state", json!(name), pos));
                } else {
                    self.diagnostics
                        .report(pos, "expected a state name after `$`, as in `$count`");
                }
                continue;
            }
            if c == '"' || c == '\'' {
                if let Some(s) = self.string(c, pos) {
                    out.push(Token::new("str", json!(s), pos));
                }
                continue;
            }
            if c == '`' {
                out.push(self.template(pos));
                continue;
            }
            let punct = PUNCTUATION.iter().find(|p| self.starts(p)).copied();
            let Some(punct) = punct else {
                self.diagnostics
                    .report(pos, format!("unexpected character `{c}`"));
                self.advance(1);
                continue;
            };
            if matches!(punct, "(" | "[" | "{") {
                depth += 1;
            }
            if matches!(punct, ")" | "]" | "}") {
                depth = depth.saturating_sub(1);
            }
            self.advance(punct.len());
            out.push(Token::new("punct", json!(punct), pos));
        }
    }
    fn escape(&mut self) -> Option<String> {
        let pos = self.pos;
        self.advance(1);
        let c = self.c();
        if let Some(decoded) = match c {
            Some('n') => Some('\n'),
            Some('t') => Some('\t'),
            Some('r') => Some('\r'),
            Some(c @ ('\\' | '"' | '\'' | '`' | '$')) => Some(c),
            _ => None,
        } {
            self.advance(1);
            return Some(decoded.to_string());
        }
        if c == Some('u') {
            let braced = self.src.get(self.i + 1) == Some(&'{');
            let end = if braced {
                self.src[self.i + 2..]
                    .iter()
                    .position(|c| *c == '}')
                    .map(|n| self.i + 2 + n)
            } else {
                Some(self.i + 5)
            };
            if let Some(end) = end.filter(|end| *end <= self.src.len()) {
                let hex: String = self.src[self.i + if braced { 2 } else { 1 }..end]
                    .iter()
                    .collect();
                if !hex.is_empty()
                    && hex.chars().all(|c| c.is_ascii_hexdigit())
                    && (braced || hex.len() == 4)
                {
                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                        if code <= 0x10ffff {
                            self.advance(end - self.i + usize::from(braced));
                            let mut code = code;
                            if (0xd800..=0xdbff).contains(&code) && self.starts("\\u") {
                                let next_start = self.i + 2;
                                if next_start + 4 <= self.src.len() {
                                    let hex: String =
                                        self.src[next_start..next_start + 4].iter().collect();
                                    if let Ok(low) = u32::from_str_radix(&hex, 16) {
                                        if (0xdc00..=0xdfff).contains(&low) {
                                            self.advance(6);
                                            code =
                                                0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00);
                                        }
                                    }
                                }
                            }
                            return Some(char::from_u32(code).unwrap_or('\u{fffd}').to_string());
                        }
                    }
                }
            }
        }
        self.diagnostics.report(
            pos,
            format!(
                r#"unknown escape `\{}`; use \n \t \\ \" \' \$ or \u{{hex}}"#,
                c.map(|c| c.to_string()).unwrap_or_default()
            ),
        );
        None
    }
    fn string(&mut self, quote: char, pos: Pos) -> Option<String> {
        self.advance(1);
        let mut value = String::new();
        loop {
            match self.c() {
                None | Some('\n') => {
                    self.diagnostics.report(
                        pos,
                        format!("unterminated string: close it with {quote} on the same line"),
                    );
                    return None;
                }
                Some(c) if c == quote => {
                    self.advance(1);
                    return Some(value);
                }
                Some('\\') => {
                    if let Some(s) = self.escape() {
                        value.push_str(&s);
                    }
                }
                Some(c) => {
                    value.push(c);
                    self.advance(1);
                }
            }
        }
    }
    fn template(&mut self, pos: Pos) -> Token {
        self.templates += 1;
        if self.templates > 64 {
            self.diagnostics.report(pos, "budget exceeded: depth");
            self.i = self.src.len();
            let mut t = Token::new("tpl", Value::Null, pos);
            t.quasis.push(String::new());
            return t;
        }
        self.advance(1);
        let mut token = Token::new("tpl", Value::Null, pos);
        let mut text = String::new();
        loop {
            match self.c() {
                None => {
                    self.diagnostics
                        .report(pos, "unterminated template: close it with `");
                    token.quasis.push(text);
                    self.templates -= 1;
                    return token;
                }
                Some('`') => {
                    self.advance(1);
                    token.quasis.push(text);
                    self.templates -= 1;
                    return token;
                }
                Some('\\') => {
                    if let Some(s) = self.escape() {
                        text.push_str(&s);
                    }
                }
                Some('$') if self.starts("${") => {
                    self.advance(2);
                    token.quasis.push(std::mem::take(&mut text));
                    token.parts.push(self.run(true));
                }
                Some(c) => {
                    text.push(c);
                    self.advance(1);
                }
            }
        }
    }
}
fn join(tokens: Vec<Token>) -> Vec<Token> {
    let mut out: Vec<Token> = vec![];
    for token in tokens {
        let last = out.last();
        if token.t == "sep" {
            if last.is_none_or(|t| t.t == "sep") {
                continue;
            }
            if token.text() == "\n"
                && last.is_some_and(|t| t.t == "punct" && continues_after(t.text()))
            {
                continue;
            }
        } else if token.t == "punct"
            && continues_before(token.text())
            && last.is_some_and(|t| t.t == "sep" && t.text() == "\n")
        {
            out.pop();
        }
        out.push(token);
    }
    out
}
