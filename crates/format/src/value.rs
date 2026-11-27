//! Shared values used in item files, expressions and operations.

use std::{cmp::Ordering, collections::BTreeMap, fmt, str::FromStr};

/// A deterministic base-ten number. The coefficient and scale are normalized.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Decimal {
    coefficient: i128,
    scale: u32,
}

/// A decimal literal exceeds the supported precision or has invalid syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidDecimal;

impl fmt::Display for InvalidDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Use a decimal within 38 digits and 18 decimal places")
    }
}

impl std::error::Error for InvalidDecimal {}

impl Decimal {
    pub(crate) const ZERO: Self = Self::integer(0);

    pub(crate) const fn integer(value: i128) -> Self {
        Self {
            coefficient: value,
            scale: 0,
        }
    }

    fn new(mut coefficient: i128, mut scale: u32) -> Option<Self> {
        while scale > 0 && coefficient % 10 == 0 {
            coefficient /= 10;
            scale -= 1;
        }
        (scale <= 18 && coefficient.unsigned_abs() < 10_u128.pow(38))
            .then_some(Self { coefficient, scale })
    }

    pub(crate) fn neg(self) -> Self {
        Self {
            coefficient: -self.coefficient,
            ..self
        }
    }

    pub(crate) fn add(self, other: Self) -> Option<Self> {
        let scale = self.scale.max(other.scale);
        let left = self
            .coefficient
            .checked_mul(10_i128.pow(scale - self.scale))?;
        let right = other
            .coefficient
            .checked_mul(10_i128.pow(scale - other.scale))?;
        Self::new(left.checked_add(right)?, scale)
    }

    pub(crate) fn mul(self, other: Self) -> Option<Self> {
        Self::new(
            self.coefficient.checked_mul(other.coefficient)?,
            self.scale + other.scale,
        )
    }

    pub(crate) fn div(self, other: Self) -> Option<Self> {
        if other == Self::ZERO {
            return None;
        }
        // Long division avoids scaling an already-large coefficient up front.
        let shift = other.scale as i32 - self.scale as i32;
        let numerator = self.coefficient.unsigned_abs();
        let denominator = other.coefficient.unsigned_abs();
        let mut quotient = numerator / denominator;
        let mut remainder = numerator % denominator;
        let mut scale = -shift;
        while (remainder > 0 || scale < 0) && scale < 18 {
            let (digit, next) = decimal_digit(remainder, denominator);
            quotient = quotient.checked_mul(10)?.checked_add(digit)?;
            remainder = next;
            scale += 1;
        }
        let quotient = i128::try_from(quotient).ok()?;
        let quotient = if self.coefficient.signum() != other.coefficient.signum() {
            -quotient
        } else {
            quotient
        };
        Self::new(quotient, scale as u32)
    }

    pub(crate) fn whole(self) -> Option<i128> {
        (self.scale == 0).then_some(self.coefficient)
    }

    pub(crate) fn rounded(self, places: u32) -> Self {
        if places >= self.scale {
            return self;
        }
        let divisor = 10_i128.pow(self.scale - places);
        let remainder = self.coefficient % divisor;
        let adjustment = if remainder.abs() >= divisor / 2 {
            self.coefficient.signum()
        } else {
            0
        };
        Self::new(self.coefficient / divisor + adjustment, places).unwrap()
    }
}

fn decimal_digit(remainder: u128, denominator: u128) -> (u128, u128) {
    let mut result = 0;
    let mut reduced = 0;
    for _ in 0..10 {
        // Add modulo the denominator without ever forming remainder * 10.
        let gap = denominator - remainder;
        if reduced >= gap {
            result += 1;
            reduced -= gap;
        } else {
            reduced += remainder;
        }
    }
    (result, reduced)
}

impl FromStr for Decimal {
    type Err = InvalidDecimal;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (negative, text) = match text.as_bytes().first() {
            Some(b'-') => (true, &text[1..]),
            Some(b'+') => (false, &text[1..]),
            _ => (false, text),
        };
        let (mantissa, exponent) = match text.split_once(['e', 'E']) {
            Some((mantissa, exponent)) => (
                mantissa,
                exponent.parse::<i32>().map_err(|_| InvalidDecimal)?,
            ),
            None => (text, 0),
        };
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        if (whole.is_empty() && fraction.is_empty())
            || !whole
                .bytes()
                .chain(fraction.bytes())
                .all(|byte| byte.is_ascii_digit())
        {
            return Err(InvalidDecimal);
        }
        let mut coefficient = whole
            .bytes()
            .chain(fraction.bytes())
            .try_fold(0_i128, |n, byte| {
                n.checked_mul(10)?.checked_add(i128::from(byte - b'0'))
            })
            .ok_or(InvalidDecimal)?;
        let scale = i32::try_from(fraction.len())
            .map_err(|_| InvalidDecimal)?
            .checked_sub(exponent)
            .ok_or(InvalidDecimal)?;
        if coefficient == 0 {
            return Ok(Self::ZERO);
        }
        if scale < 0 {
            let power = 10_i128
                .checked_pow(scale.unsigned_abs())
                .ok_or(InvalidDecimal)?;
            coefficient = coefficient.checked_mul(power).ok_or(InvalidDecimal)?;
        }
        Self::new(
            if negative { -coefficient } else { coefficient },
            scale.max(0) as u32,
        )
        .ok_or(InvalidDecimal)
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.scale == 0 {
            return write!(f, "{}", self.coefficient);
        }
        let magnitude = self.coefficient.unsigned_abs();
        let divisor = 10_u128.pow(self.scale);
        let sign = if self.coefficient < 0 { "-" } else { "" };
        write!(
            f,
            "{sign}{}.{:0width$}",
            magnitude / divisor,
            magnitude % divisor,
            width = self.scale as usize
        )
    }
}

impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> Ordering {
        let left_sign = self.coefficient.signum();
        let right_sign = other.coefficient.signum();
        if left_sign != right_sign {
            return left_sign.cmp(&right_sign);
        }
        let left = self.coefficient.unsigned_abs();
        let right = other.coefficient.unsigned_abs();
        let a = 10_u128.pow(self.scale);
        let b = 10_u128.pow(other.scale);
        let order = (left / a)
            .cmp(&(right / b))
            .then_with(|| ((left % a) * b).cmp(&((right % b) * a)));
        if left_sign < 0 {
            order.reverse()
        } else {
            order
        }
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A map of item, record, viewer state or settings values keyed by declared name.
pub type Fields = BTreeMap<String, Value>;

/// A value read from an item or produced by an expression.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub enum Value {
    /// An absent or explicitly null value.
    #[default]
    Empty,
    /// A yes/no value.
    Bool(bool),
    /// An exact number with no currency.
    Number(Decimal),
    /// An amount in one currency, never subject to implicit conversion.
    Money {
        /// Exact amount.
        amount: Decimal,
        /// Uppercase ISO currency identifier.
        currency: String,
    },
    /// Text, including references carried by the host as strings.
    Text(String),
    /// An ISO civil day.
    Date(String),
    /// An ISO civil time with an optional numeric UTC offset.
    Time(String),
    /// A length with separate calendar months and elapsed seconds.
    Duration {
        /// Calendar months, applied before any seconds.
        months: i32,
        /// Elapsed seconds.
        seconds: i64,
    },
    /// An ordered list of values.
    List(Vec<Value>),
    /// Named values, such as those in a record or a data field.
    Map(Fields),
}

impl Value {
    /// Estimate owned bytes when copying this value. Walks nested values
    /// without allocating and saturates on arithmetic overflow.
    pub fn copy_bytes(&self) -> usize {
        let base = std::mem::size_of::<Self>();
        let contents = match self {
            Self::Text(text) | Self::Date(text) | Self::Time(text) => text.len(),
            Self::Money { currency, .. } => currency.len(),
            Self::List(values) => values.iter().fold(0, |bytes, value| {
                usize::saturating_add(bytes, value.copy_bytes())
            }),
            Self::Map(fields) => fields.iter().fold(0, |bytes, (key, value)| {
                usize::saturating_add(bytes, key.len()).saturating_add(value.copy_bytes())
            }),
            _ => 0,
        };
        base.saturating_add(contents)
    }

    pub(crate) fn truth(&self) -> bool {
        !matches!(self, Self::Empty | Self::Bool(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_retains_exact_nonbinary_values() {
        let a: Decimal = "0.1".parse().unwrap();
        let b: Decimal = "0.2".parse().unwrap();
        assert_eq!(a.add(b).unwrap().to_string(), "0.3");
        assert_eq!(b.div(a).unwrap().to_string(), "2");
        assert_eq!(
            a.div(Decimal::integer(3)).unwrap().to_string(),
            "0.033333333333333333"
        );
        assert!("-9.55".parse::<Decimal>().unwrap() < "-9.5".parse().unwrap());
    }
}
