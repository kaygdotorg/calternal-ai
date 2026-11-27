//! Physical line folding for plain and flow-quoted YAML scalars.

use std::{borrow::Cow, ops::Range};

use super::{Parser, list_entry, mapping_colon, plain_comment};
use crate::{Diagnostic, lines::Site};

impl<'a> Parser<'a> {
    pub(super) fn scalar_lines(
        &mut self,
        raw: &'a str,
        site: Site,
        parent: usize,
        comment: &mut Option<Range<usize>>,
    ) -> Result<Cow<'a, str>, Diagnostic> {
        match raw.as_bytes()[0] {
            quote @ (b'"' | b'\'') => self.quoted_lines(raw, quote, site, parent, comment),
            b'[' if !raw.starts_with("[[") || !raw.ends_with("]]") => {
                self.flow_lines(raw, site, parent, comment)
            }
            b'[' | b'&' | b'*' | b'!' | b'{' | b'}' => Ok(Cow::Borrowed(raw)),
            _ => Ok(self.plain_lines(raw, parent, comment)),
        }
    }

    fn quoted_lines(
        &mut self,
        raw: &'a str,
        quote: u8,
        site: Site,
        parent: usize,
        comment: &mut Option<Range<usize>>,
    ) -> Result<Cow<'a, str>, Diagnostic> {
        if closing_quote(&raw[1..], quote).is_some() {
            return Ok(Cow::Borrowed(raw));
        }
        let mut folded = raw.to_owned();
        let mut escaped_break = eat_break_escape(&mut folded, quote);
        let mut blanks = 0;
        while let Some(line) = self.lines.get(self.at) {
            let part = line.text.trim_start_matches([' ', '\t']);
            if part.is_empty() {
                blanks += 1;
                self.at += 1;
                continue;
            }
            if line.indent <= parent {
                break;
            }
            self.at += 1;
            if !escaped_break || blanks > 0 {
                fold(&mut folded, blanks);
            }
            if let Some(end) = closing_quote(part, quote) {
                folded.push_str(&part[..=end]);
                let tail = &part[end + 1..];
                let trimmed = tail.trim_start_matches([' ', '\t']);
                if trimmed.starts_with('#') && tail.len() > trimmed.len() {
                    *comment = Some(line.start + line.text.len() - trimmed.len()..line.end);
                } else if !trimmed.is_empty() {
                    return Err(line
                        .site
                        .error("Only a comment may follow a quoted item value"));
                }
                return Ok(Cow::Owned(folded));
            }
            folded.push_str(part.trim_end_matches([' ', '\t']));
            escaped_break = eat_break_escape(&mut folded, quote);
            blanks = 0;
        }
        Err(site.error("Close the quoted item value"))
    }

    fn flow_lines(
        &mut self,
        raw: &'a str,
        site: Site,
        parent: usize,
        comment: &mut Option<Range<usize>>,
    ) -> Result<Cow<'a, str>, Diagnostic> {
        let mut scan = FlowScanner::default();
        scan.consume(raw);
        if scan.depth == 0 {
            return Ok(Cow::Borrowed(raw));
        }
        let mut folded = raw.to_owned();
        let mut escaped_break = eat_break_escape(&mut folded, scan.quote.unwrap_or_default());
        let mut blanks = 0;
        while let Some(line) = self.lines.get(self.at) {
            let text = line.text.trim_start_matches([' ', '\t']);
            if text.is_empty() {
                self.at += 1;
                blanks += 1;
                continue;
            }
            if line.indent <= parent {
                break;
            }
            self.at += 1;
            let inline = scan.consume(text);
            let part = &text[..inline.unwrap_or(text.len())];
            if part.trim_end_matches([' ', '\t']).is_empty() {
                continue;
            }
            if !escaped_break || blanks > 0 {
                fold(&mut folded, blanks);
            }
            folded.push_str(part.trim_end_matches([' ', '\t']));
            if scan.depth == 0 {
                if let Some(index) = inline {
                    *comment = Some(line.start + line.text.len() - text.len() + index..line.end);
                }
                return Ok(Cow::Owned(folded));
            }
            escaped_break = eat_break_escape(&mut folded, scan.quote.unwrap_or_default());
            blanks = 0;
        }
        Err(site.error("Close the flow list with ]"))
    }

    fn plain_lines(
        &mut self,
        raw: &'a str,
        parent: usize,
        comment: &mut Option<Range<usize>>,
    ) -> Cow<'a, str> {
        let mut folded = Cow::Borrowed(raw);
        while comment.is_none() {
            let mut next = self.at;
            while self
                .lines
                .get(next)
                .is_some_and(|line| line.text.trim().is_empty())
            {
                next += 1;
            }
            let Some(line) = self.lines.get(next).filter(|line| line.indent > parent) else {
                break;
            };
            let part = &line.text[line.indent..];
            let (part, inline) = plain_comment(part);
            if part.is_empty() || list_entry(part).is_some() || mapping_colon(part).is_some() {
                break;
            }
            let buffer = folded.to_mut();
            fold(buffer, next - self.at);
            buffer.push_str(part.trim_end_matches([' ', '\t']));
            *comment = inline.map(|offset| line.start + line.indent + offset..line.end);
            self.at = next + 1;
        }
        folded
    }
}

fn fold(text: &mut String, blanks: usize) {
    if blanks == 0 {
        text.push(' ');
    } else {
        text.extend(std::iter::repeat_n('\n', blanks));
    }
}

fn eat_break_escape(text: &mut String, quote: u8) -> bool {
    if quote == b'"' && text.bytes().rev().take_while(|byte| *byte == b'\\').count() % 2 == 1 {
        text.pop();
        true
    } else {
        false
    }
}

fn closing_quote(text: &str, quote: u8) -> Option<usize> {
    let mut bytes = text.bytes().enumerate().peekable();
    while let Some((offset, byte)) = bytes.next() {
        if quote == b'"' && byte == b'\\'
            || quote == b'\''
                && byte == quote
                && bytes.peek().is_some_and(|(_, next)| *next == quote)
        {
            bytes.next();
        } else if byte == quote {
            return Some(offset);
        }
    }
    None
}

#[derive(Default)]
struct FlowScanner {
    depth: usize,
    quote: Option<u8>,
    at_value: bool,
}

impl FlowScanner {
    fn consume(&mut self, text: &str) -> Option<usize> {
        let mut bytes = text.bytes().enumerate().peekable();
        let mut separated = true;
        while let Some((index, byte)) = bytes.next() {
            match self.quote {
                Some(b'"') if byte == b'\\' => {
                    bytes.next();
                }
                Some(b'\'')
                    if byte == b'\'' && bytes.peek().is_some_and(|(_, next)| *next == byte) =>
                {
                    bytes.next();
                }
                Some(quote) if quote == byte => {
                    self.quote = None;
                }
                Some(_) => {}
                None => match byte {
                    b'[' => {
                        self.depth += 1;
                        self.at_value = true;
                    }
                    b']' => {
                        self.depth = self.depth.saturating_sub(1);
                        self.at_value = false;
                    }
                    b',' => self.at_value = true,
                    b' ' | b'\t' => {}
                    b'#' if separated => return Some(index),
                    b'\'' | b'"' if self.at_value => {
                        self.quote = Some(byte);
                        self.at_value = false;
                    }
                    _ => self.at_value = false,
                },
            }
            separated = matches!(byte, b' ' | b'\t');
        }
        None
    }
}
