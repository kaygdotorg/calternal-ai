//! Parse the supported CSS subset and serialize new declarations from safe tokens.

use cssparser::{Delimiter, ParseError, Parser, Token};
use std::fmt::Write;

const MAX_CSS_BYTES: usize = 32 * 1024;
const MAX_DECLARATIONS: usize = 64;

enum Declaration {
    Family(String),
    FontUrl(String),
    BackgroundUrl(String),
}

fn declarations(input: &mut Parser<'_>, font: bool) -> Vec<Declaration> {
    let mut result = Vec::new();
    while !input.is_exhausted() && result.len() < MAX_DECLARATIONS {
        let parsed: Result<_, ParseError<()>> =
            input.parse_until_after(Delimiter::Semicolon, |input| {
                let name = input.expect_ident_cloned()?;
                input.expect_colon()?;
                match &*name.to_ascii_lowercase() {
                    "font-family" => {
                        let name = input.expect_ident_or_string()?.to_string();
                        if name.len() <= 80
                            && !name.is_empty()
                            && name
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b" -_".contains(&b))
                        {
                            Ok(Declaration::Family(name))
                        } else {
                            Err(ParseError::custom(()))
                        }
                    }
                    "background-image" if !font => {
                        Ok(Declaration::BackgroundUrl(input.expect_url()?.to_string()))
                    }
                    "src" if font => {
                        let url = input.expect_url()?.to_string();
                        if !input.is_exhausted() {
                            input.expect_function_matching("format")?;
                            input.parse_nested_block(|input| {
                                input.expect_ident_or_string()?;
                                Ok::<_, ParseError<()>>(())
                            })?;
                        }
                        Ok(Declaration::FontUrl(url))
                    }
                    _ => Err(ParseError::custom(())),
                }
            });
        if let Ok(declaration) = parsed {
            result.push(declaration);
        }
    }
    result
}

pub(super) fn inline(css: &str, mut proxy: impl FnMut(&str) -> Option<String>) -> String {
    if css.len() > MAX_CSS_BYTES {
        return String::new();
    }
    let mut output = String::new();
    for declaration in declarations(&mut Parser::new(css), false) {
        match declaration {
            Declaration::Family(name) => {
                let _ = write!(output, "font-family:'{name}';");
            }
            Declaration::BackgroundUrl(url) => {
                if let Some(target) = proxy(&url) {
                    let _ = write!(output, "background-image:url('{target}');");
                }
            }
            Declaration::FontUrl(_) => (),
        }
    }
    output
}

pub(super) fn stylesheet(css: &str, mut proxy: impl FnMut(&str) -> Option<String>) -> String {
    if css.len() > MAX_CSS_BYTES {
        return String::new();
    }
    let mut input = Parser::new(css);
    let mut output = String::new();
    while let Ok(token) = input.next().cloned() {
        if !matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("font-face"))
            || input.expect_curly_bracket_block().is_err()
        {
            continue;
        }
        let block = input.parse_nested_block(|input| {
            let parts = declarations(input, true);
            Ok::<_, ParseError<()>>(parts)
        });
        let Ok(parts) = block else {
            continue;
        };
        let mut family = None;
        let mut url = None;
        for part in parts {
            match part {
                Declaration::Family(name) => family = Some(name),
                Declaration::FontUrl(source) => url = Some(source),
                Declaration::BackgroundUrl(_) => (),
            }
        }
        if let (Some(family), Some(url)) = (family, url)
            && let Some(target) = proxy(&url)
        {
            let _ = write!(
                output,
                "@font-face{{font-family:'{family}';src:url('{target}')}}"
            );
        }
    }
    output
}
