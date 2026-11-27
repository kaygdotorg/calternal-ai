//! Civil period labels and authorized historical formula inputs.

use std::collections::BTreeMap;

use crate::{
    Diagnostic, EvalContext, Field, Fields, Row, Value, dates, expr::eval::evaluate_field,
};

/// The granularity of a period-specific field or total.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Period {
    /// The civil day.
    Day,
    /// The ISO week beginning on Monday.
    Week,
    /// The civil calendar month.
    Month,
    /// The civil calendar year.
    Year,
}

impl Period {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "day" => Self::Day,
            "week" => Self::Week,
            "month" => Self::Month,
            "year" => Self::Year,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
        }
    }

    /// Render the period containing the supplied civil date, in formula format.
    pub fn label(self, date: &str) -> Option<String> {
        dates::day(date)?;
        if self == Self::Day {
            Some(date.into())
        } else {
            dates::period(date, self.name())
        }
    }

    pub(crate) fn precedes(self, previous: &str, current: &str) -> bool {
        let Some(current) = dates::day(current) else {
            return false;
        };
        let prior_day = self.floor(current) - 1;
        dates::render_day(prior_day)
            .and_then(|date| self.label(&date))
            .is_some_and(|expected| self.label(previous).as_ref() == Some(&expected))
    }

    fn floor(self, date: i64) -> i64 {
        let (year, month, _) = dates::civil(date);
        match self {
            Self::Day => date,
            Self::Week => date - (date + 3).rem_euclid(7),
            Self::Month => dates::day_number(year, month, 1),
            Self::Year => dates::day_number(year, 1, 1),
        }
    }

    /// List period starts from the oldest eligible item through the current
    /// period. The supplied history limit must be between zero and 20 years.
    pub fn starts(self, oldest: &str, today: &str, years: u8) -> Result<Vec<String>, Diagnostic> {
        let invalid = || {
            crate::lines::Site::START
                .error("Supply valid period dates and at most a 20-year history")
        };
        let oldest = dates::day(oldest).ok_or_else(invalid)?;
        let today = dates::day(today)
            .filter(|_| years <= 20)
            .ok_or_else(invalid)?;
        if oldest > today {
            return Ok(Vec::new());
        }
        let cutoff = dates::shifted_day(today, -12 * i32::from(years), 0).ok_or_else(invalid)?;
        let mut date = self.floor(oldest.max(cutoff));
        let mut result = Vec::new();
        while date <= today {
            if let Some(rendered) = dates::render_day(date) {
                result.push(rendered);
            }
            date = match self {
                Self::Day => date + 1,
                Self::Week => date + 7,
                Self::Month => dates::shifted_day(date, 1, 0).ok_or_else(invalid)?,
                Self::Year => dates::shifted_day(date, 12, 0).ok_or_else(invalid)?,
            };
        }
        Ok(result)
    }
}

/// Already-authorized, typed whole-kind candidates, indexed by the name used
/// in the formula. Missing keys mean unavailable. An empty vector means empty.
pub type KindRows<'a> = BTreeMap<String, Vec<Row<'a>>>;

/// One civil period's authorized data for an item or collection header.
/// Formulas evaluated through `previous()` use only this date, fields and
/// authorized candidate rows. Other ambient inputs are unavailable.
#[derive(Clone, Copy, Debug, Default)]
pub struct PeriodSnapshot<'a> {
    /// Any civil date in the selected period.
    pub date: &'a str,
    /// Historical row or header fields, when available.
    pub fields: Option<&'a Fields>,
    /// Historical kind candidate rows for all(), when available.
    pub kinds: Option<&'a KindRows<'a>>,
    /// Already-authorized reverse rows for the historical item.
    pub reverse: Option<&'a KindRows<'a>>,
    /// Immediately preceding snapshot. Omit an unavailable or failed period.
    pub previous: Option<&'a Self>,
}

/// Evaluate one declared item period field with a fixed 50,000 step budget.
/// Use the current row for stored inputs and already-computed dependencies.
pub fn evaluate_period_field(
    field: &Field,
    row: Row<'_>,
    context: &EvalContext<'_>,
) -> Result<Value, Diagnostic> {
    if field.period.is_none() {
        return Err(field.site.error("Choose a declared per-period formula"));
    }
    evaluate_field(field, row, context, &mut 50_000, false)
}
