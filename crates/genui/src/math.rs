//! Pure numeric helpers, including upstream deterministic bit hashing and decimal rounding.
use crate::{
    ast::Pos,
    error::Failure,
    value::{Value, number_text},
};
pub(crate) fn digits(v: Option<&Value>, pos: Pos) -> Result<usize, Failure> {
    let n = v.map(|v| v.number(pos)).transpose()?.unwrap_or(0.0);
    if n.fract() != 0.0 || !(0.0..=10.0).contains(&n) {
        Err(Failure::new(pos, "digits must be an integer from 0 to 10"))
    } else {
        Ok(n as usize)
    }
}
pub(crate) fn round(n: f64, digits: usize) -> f64 {
    let shifted = format!("{}e{}", number_text(n.abs()), digits)
        .parse::<f64>()
        .unwrap_or(f64::NAN);
    let r = format!("{}e-{}", number_text((shifted + 0.5).floor()), digits)
        .parse::<f64>()
        .unwrap_or(f64::NAN);
    let r = if r.is_finite() {
        r
    } else {
        (n.abs() * 10f64.powi(digits as i32) + 0.5).floor() / 10f64.powi(digits as i32)
    };
    if n < 0.0 { -r } else { r }
}
pub(crate) fn fixed(n: f64, digits: usize) -> String {
    if n.abs() >= 1e21 {
        number_text(n)
    } else {
        format!("{:.*}", digits, round(n, digits))
    }
}
pub(crate) fn fmt(n: f64, digits: Option<usize>) -> String {
    let mut s = fixed(n, digits.unwrap_or(2));
    if digits.is_none() && s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    let (whole, fraction) = s.split_once('.').unwrap_or((&s, ""));
    let sign = whole.starts_with('-');
    let whole = whole.trim_start_matches('-');
    let mut out = String::new();
    if sign {
        out.push('-');
    }
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if !fraction.is_empty() {
        out.push('.');
        out.push_str(fraction);
    }
    out
}
pub(crate) fn random(i: f64) -> f64 {
    let bits = if i == 0.0 {
        0f64.to_bits()
    } else {
        i.to_bits()
    };
    let mut h = (bits >> 32) as u32 ^ ((bits as u32) ^ 0x9e3779b9).wrapping_mul(0x85ebca6b);
    h = (h ^ (h >> 16)).wrapping_mul(0x85ebca6b);
    h = (h ^ (h >> 13)).wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;
    h as f64 / 4294967296.0
}
