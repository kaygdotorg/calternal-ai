use super::Op;
use crate::{Decimal, Diagnostic, Value, dates, lines::Site};
use std::cmp::Ordering;

pub(crate) fn day_value(value: &Value, offset: Option<i32>) -> Option<i64> {
    match value {
        Value::Date(date) => dates::day(date),
        Value::Time(time) => dates::time(time).map(|(seconds, at)| {
            let offset = offset.filter(|_| {
                time.get(11..)
                    .is_some_and(|tail| tail.contains(['Z', '+', '-']))
            });
            (seconds + i64::from(offset.unwrap_or(at) - at)).div_euclid(86400)
        }),
        _ => None,
    }
}

pub(crate) fn compare(
    left: &Value,
    right: &Value,
    site: Site,
) -> Result<Option<Ordering>, Diagnostic> {
    Ok(match (left, right) {
        (Value::Number(a), Value::Number(b)) => Some(a.cmp(b)),
        (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
        (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),
        (Value::Date(a), Value::Date(b)) => Some(a.cmp(b)),
        (Value::Time(a), Value::Time(b)) => dates::time(a)
            .zip(dates::time(b))
            .map(|((a, ao), (b, bo))| (a - i64::from(ao)).cmp(&(b - i64::from(bo)))),
        (Value::Time(_), Value::Date(_)) | (Value::Date(_), Value::Time(_)) => {
            day_value(left, None)
                .zip(day_value(right, None))
                .map(|(a, b)| a.cmp(&b))
        }
        (
            Value::Duration {
                months: a,
                seconds: x,
            },
            Value::Duration {
                months: b,
                seconds: y,
            },
        ) if a == b => Some(x.cmp(y)),
        (
            Value::Duration {
                months: a,
                seconds: x,
            },
            Value::Duration {
                months: b,
                seconds: y,
            },
        ) if x == y => Some(a.cmp(b)),
        (
            Value::Money {
                amount: a,
                currency: ac,
            },
            Value::Money {
                amount: b,
                currency: bc,
            },
        ) => {
            if ac != bc {
                return Err(site.error("Compare or combine amounts in the same currency"));
            }
            Some(a.cmp(b))
        }
        _ => None,
    })
}

pub(super) fn comparison(
    op: Op,
    left: &Value,
    right: &Value,
    site: Site,
) -> Result<Value, Diagnostic> {
    if matches!(left, Value::Empty) || matches!(right, Value::Empty) {
        return Ok(Value::Bool(match op {
            Op::Eq => left == right,
            Op::Ne => left != right,
            _ => false,
        }));
    }
    let order = compare(left, right, site)?;
    let equal = order.map_or(left == right, |order| order == Ordering::Equal);
    Ok(Value::Bool(match op {
        Op::Eq => equal,
        Op::Ne => !equal,
        Op::Lt => order == Some(Ordering::Less),
        Op::Le => matches!(order, Some(Ordering::Less | Ordering::Equal)),
        Op::Gt => order == Some(Ordering::Greater),
        Op::Ge => matches!(order, Some(Ordering::Greater | Ordering::Equal)),
        _ => unreachable!(),
    }))
}

pub(crate) fn aggregate(name: &str, values: Vec<Value>, site: Site) -> Result<Value, Diagnostic> {
    if name == "count" {
        return Ok(Value::Number(Decimal::integer(values.len() as i128)));
    }
    if name == "first" {
        return Ok(values
            .into_iter()
            .find(|value| *value != Value::Empty)
            .unwrap_or_default());
    }
    if name == "last" {
        return Ok(values.into_iter().next_back().unwrap_or_default());
    }
    let mut values = values.into_iter().filter(|value| *value != Value::Empty);
    let Some(first) = values.next() else {
        return Ok(if name == "sum" {
            Value::Number(Decimal::ZERO)
        } else {
            Value::Empty
        });
    };
    let mut value = first;
    let mut count = 1;
    for next in values {
        count += 1;
        match name {
            "min" | "max" => {
                let order = compare(&next, &value, site)?
                    .ok_or_else(|| site.error("These values cannot be ordered together"))?;
                if (name == "min" && order.is_lt()) || (name == "max" && order.is_gt()) {
                    value = next
                }
            }
            "sum" | "average" => value = math(Op::Add, value, next, site)?,
            _ => return Err(site.error("Unknown total")),
        }
    }
    if name == "average" {
        math(Op::Div, value, Value::Number(Decimal::integer(count)), site)
    } else {
        Ok(value)
    }
}

pub(super) fn math(op: Op, left: Value, right: Value, site: Site) -> Result<Value, Diagnostic> {
    let overflow =
        || site.error("The arithmetic result exceeds the supported range or divides by zero");
    let number = |a: Decimal, b: Decimal| {
        match op {
            Op::Add => a.add(b),
            Op::Sub => a.add(b.neg()),
            Op::Mul => a.mul(b),
            Op::Div => a.div(b),
            _ => None,
        }
        .ok_or_else(overflow)
    };
    let result = match (left, right) {
        (Value::Empty, _) | (_, Value::Empty) => Value::Empty,
        (Value::Number(a), Value::Number(b)) => Value::Number(number(a, b)?),
        (Value::Number(a), Value::Money { amount, currency })
            if op == Op::Mul || (op == Op::Sub && a == Decimal::ZERO) =>
        {
            Value::Money {
                amount: number(a, amount)?,
                currency,
            }
        }
        (Value::Money { amount, currency }, Value::Number(b))
            if matches!(op, Op::Mul | Op::Div) =>
        {
            Value::Money {
                amount: number(amount, b)?,
                currency,
            }
        }
        (
            Value::Money {
                amount: a,
                currency: ac,
            },
            Value::Money {
                amount: b,
                currency: bc,
            },
        ) => {
            if ac != bc {
                return Err(site.error("Use the same currency for both amounts"));
            }
            match op {
                Op::Add | Op::Sub => Value::Money {
                    amount: number(a, b)?,
                    currency: ac,
                },
                Op::Div => Value::Number(number(a, b)?),
                _ => return Err(site.error("Multiplying two money amounts is undefined")),
            }
        }
        (
            Value::Duration {
                months: a,
                seconds: x,
            },
            Value::Duration {
                months: b,
                seconds: y,
            },
        ) if matches!(op, Op::Add | Op::Sub) => {
            let (months, seconds) = if op == Op::Add {
                (a.checked_add(b), x.checked_add(y))
            } else {
                (a.checked_sub(b), x.checked_sub(y))
            };
            Value::Duration {
                months: months.ok_or_else(overflow)?,
                seconds: seconds.ok_or_else(overflow)?,
            }
        }
        (Value::Number(value), Value::Duration { months, seconds })
            if op == Op::Sub && value == Decimal::ZERO =>
        {
            Value::Duration {
                months: months.checked_neg().ok_or_else(overflow)?,
                seconds: seconds.checked_neg().ok_or_else(overflow)?,
            }
        }
        (Value::Date(date), Value::Duration { months, seconds })
            if matches!(op, Op::Add | Op::Sub) =>
        {
            let (months, seconds) = if op == Op::Add {
                (Some(months), Some(seconds))
            } else {
                (months.checked_neg(), seconds.checked_neg())
            };
            let day = dates::shifted_day(
                dates::day(&date).ok_or_else(overflow)?,
                months.ok_or_else(overflow)?,
                seconds.ok_or_else(overflow)?,
            );
            Value::Date(day.and_then(dates::render_day).ok_or_else(overflow)?)
        }
        (Value::Time(time), Value::Duration { months, seconds })
            if matches!(op, Op::Add | Op::Sub) =>
        {
            let (months, seconds) = if op == Op::Add {
                (Some(months), Some(seconds))
            } else {
                (months.checked_neg(), seconds.checked_neg())
            };
            Value::Time(
                dates::shifted_time(
                    &time,
                    months.ok_or_else(overflow)?,
                    seconds.ok_or_else(overflow)?,
                )
                .ok_or_else(overflow)?,
            )
        }
        (Value::Date(a), Value::Date(b)) if op == Op::Sub => Value::Duration {
            months: 0,
            seconds: (dates::day(&a).ok_or_else(overflow)?
                - dates::day(&b).ok_or_else(overflow)?)
                * 86400,
        },
        (Value::Time(a), Value::Time(b)) if op == Op::Sub => {
            let (a, ao) = dates::time(&a).ok_or_else(overflow)?;
            let (b, bo) = dates::time(&b).ok_or_else(overflow)?;
            Value::Duration {
                months: 0,
                seconds: (a - i64::from(ao)) - (b - i64::from(bo)),
            }
        }
        _ => return Err(site.error("These values do not support this arithmetic operation")),
    };
    Ok(result)
}
