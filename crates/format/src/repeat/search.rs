use super::{Frequency, RepeatRule, Stamp};
use crate::{Diagnostic, dates, lines::Site};

const DAY: i64 = 86400;

impl RepeatRule {
    pub(super) fn search(
        &self,
        seed: &Stamp,
        lower: i64,
        upper: i64,
        steps: &mut usize,
        site: Site,
        mut emit: impl FnMut(i64) -> bool,
    ) -> Result<(), Diagnostic> {
        let mut charge = || {
            *steps = steps
                .checked_sub(1)
                .ok_or_else(|| site.error("The recurrence is too costly"))?;
            Ok::<_, Diagnostic>(())
        };
        if !seed.timed
            && (self.frequency < Frequency::Daily
                || !self.hours.is_empty()
                || !self.minutes.is_empty()
                || !self.seconds.is_empty())
        {
            return Err(site.error("Use a timestamp seed for a recurrence with clock fields"));
        }
        let upper = self.until.as_ref().map_or(upper, |until| {
            let wall = if until.timed {
                until.at
                    + until
                        .offset
                        .map_or(0, |offset| i64::from(seed.offset.unwrap_or(0) - offset))
            } else {
                until.at + DAY - 1
            };
            upper.min(wall + 1)
        });
        if upper <= lower || seed.at >= upper {
            return Ok(());
        }
        let mut remaining = self.count.unwrap_or(i64::MAX) - 1;
        if lower <= seed.at && !emit(seed.at) || remaining == 0 {
            return Ok(());
        }
        let seed_day = seed.at.div_euclid(DAY);
        let effective_lower = lower.max(seed.at + 1);
        let first = self.index(seed.at, effective_lower);
        let mut index = if self.count.is_none() { first } else { 0 };
        if self.count.is_some() && self.simple_fixed() {
            remaining -= first.saturating_sub(1);
            index = first;
        }
        let mut positioned = Vec::new();
        while remaining > 0 {
            let (start, end) = self.bucket(seed.at, index);
            if start >= upper {
                break;
            }
            charge()?;
            let mut visit = |at| {
                if at <= seed.at || at >= upper {
                    return true;
                }
                remaining -= 1;
                (at < effective_lower || emit(at)) && remaining > 0
            };
            let mut keep_going = true;
            let mut candidates = |at| {
                if self.positions.is_empty() {
                    keep_going = visit(at);
                } else {
                    positioned.push(at);
                }
                keep_going
            };
            for day in start.div_euclid(DAY)..=(end - 1).div_euclid(DAY) {
                charge()?;
                if !self.matches_day(seed_day, day) {
                    if self.frequency < Frequency::Daily && day == start.div_euclid(DAY) {
                        // Jump over this date's remaining subdaily buckets.
                        index = index.max(self.index(seed.at, (day + 1) * DAY) - 1);
                    }
                    continue;
                }
                if !self.clocks(seed, day, start, end, &mut charge, &mut candidates)? {
                    break;
                }
            }
            if !self.positions.is_empty() {
                let length = positioned.len() as i64;
                let mut positions: Vec<_> = self
                    .positions
                    .iter()
                    .filter_map(|&position| {
                        let index = if position > 0 {
                            position - 1
                        } else {
                            length + position
                        };
                        positioned.get(index as usize).copied()
                    })
                    .collect();
                positions.sort_unstable();
                positions.dedup();
                for at in positions {
                    if !visit(at) {
                        keep_going = false;
                        break;
                    }
                }
                positioned.clear();
            }
            if !keep_going {
                break;
            }
            index += 1;
        }
        Ok(())
    }

    fn simple_fixed(&self) -> bool {
        self.frequency <= Frequency::Weekly
            && [
                &self.month_days,
                &self.months,
                &self.year_days,
                &self.week_numbers,
                &self.hours,
                &self.minutes,
                &self.seconds,
                &self.positions,
            ]
            .into_iter()
            .all(Vec::is_empty)
            && self.weekdays.is_empty()
    }

    fn fixed_unit(&self) -> i64 {
        match self.frequency {
            Frequency::Secondly => 1,
            Frequency::Minutely => 60,
            Frequency::Hourly => 3600,
            Frequency::Daily => DAY,
            Frequency::Weekly => DAY * 7,
            _ => unreachable!(),
        }
    }

    fn index(&self, seed: i64, at: i64) -> i64 {
        let seed_day = seed.div_euclid(DAY);
        let day = at.div_euclid(DAY);
        let (year, month, _) = dates::civil(day);
        let (seed_year, seed_month, _) = dates::civil(seed_day);
        let period = match self.frequency {
            Frequency::Yearly if !self.week_numbers.is_empty() => {
                week_year(year, day, self.week_start)
                    - week_year(seed_year, seed_day, self.week_start)
            }
            Frequency::Yearly => year - seed_year,
            Frequency::Monthly => 12 * (year - seed_year) + month - seed_month,
            Frequency::Weekly => (day - week_floor(seed_day, self.week_start)).div_euclid(7),
            _ => {
                let unit = self.fixed_unit();
                at.div_euclid(unit) - seed.div_euclid(unit)
            }
        };
        period.div_euclid(self.interval).max(0)
    }

    fn bucket(&self, seed: i64, index: i64) -> (i64, i64) {
        let seed_day = seed.div_euclid(DAY);
        let (year, month, _) = dates::civil(seed_day);
        let offset = index * self.interval;
        match self.frequency {
            Frequency::Yearly => {
                let year = if self.week_numbers.is_empty() {
                    year
                } else {
                    week_year(year, seed_day, self.week_start)
                } + offset;
                let start_of = |year| {
                    if self.week_numbers.is_empty() {
                        dates::day_number(year, 1, 1)
                    } else {
                        week_one(year, self.week_start)
                    }
                };
                (start_of(year) * DAY, start_of(year + 1) * DAY)
            }
            Frequency::Monthly => {
                let serial = year * 12 + month - 1 + offset;
                let month_start = |serial: i64| {
                    dates::day_number(serial.div_euclid(12), serial.rem_euclid(12) + 1, 1) * DAY
                };
                (month_start(serial), month_start(serial + 1))
            }
            Frequency::Weekly => {
                let start = (week_floor(seed_day, self.week_start) + 7 * offset) * DAY;
                (start, start + DAY * 7)
            }
            _ => {
                let unit = self.fixed_unit();
                let start = (seed.div_euclid(unit) + offset) * unit;
                (start, start + unit)
            }
        }
    }

    fn matches_day(&self, seed_day: i64, day: i64) -> bool {
        let (_, seed_month, seed_date) = dates::civil(seed_day);
        let (year, month, date) = dates::civil(day);
        let month_length = dates::days_in_month(year, month);
        let year_start = dates::day_number(year, 1, 1);
        let year_end = dates::day_number(year + 1, 1, 1);
        let ordinal = day - year_start + 1;
        let weekday = weekday(day);
        if !includes(&self.months, month, 12)
            || !includes(&self.month_days, date, month_length)
            || !includes(&self.year_days, ordinal, year_end - year_start)
        {
            return false;
        }
        if !self.week_numbers.is_empty() {
            let start = week_one(year, self.week_start);
            let next = week_one(year + 1, self.week_start);
            let (start, end) = if day < start {
                (week_one(year - 1, self.week_start), start)
            } else if day >= next {
                (next, week_one(year + 2, self.week_start))
            } else {
                (start, next)
            };
            if !includes(
                &self.week_numbers,
                (day - start).div_euclid(7) + 1,
                (end - start) / 7,
            ) {
                return false;
            }
        }
        if !self.weekdays.is_empty()
            && !self.weekdays.iter().any(|&(want, n)| {
                if want != weekday {
                    return false;
                }
                if n == 0 {
                    return true;
                }
                let (since, until) =
                    if self.frequency == Frequency::Yearly && self.months.is_empty() {
                        (ordinal - 1, year_end - day - 1)
                    } else {
                        (date - 1, month_length - date)
                    };
                n == since / 7 + 1 || n == -until / 7 - 1
            })
        {
            return false;
        }
        if self.frequency == Frequency::Weekly && self.weekdays.is_empty() {
            return weekday == self::weekday(seed_day);
        }
        if self.frequency >= Frequency::Monthly
            && self.weekdays.is_empty()
            && self.month_days.is_empty()
            && self.year_days.is_empty()
            && self.week_numbers.is_empty()
        {
            return date == seed_date
                && (self.frequency != Frequency::Yearly
                    || !self.months.is_empty()
                    || month == seed_month);
        }
        true
    }

    fn clocks(
        &self,
        seed: &Stamp,
        day: i64,
        start: i64,
        end: i64,
        charge: &mut impl FnMut() -> Result<(), Diagnostic>,
        mut emit: impl FnMut(i64) -> bool,
    ) -> Result<bool, Diagnostic> {
        let seed_time = seed.at.rem_euclid(DAY);
        let fixed = start.rem_euclid(DAY);
        let (hour, minute, second) = (fixed / 3600, fixed / 60 % 60, fixed % 60);
        let Some(hours) = parts(
            &self.hours,
            seed_time / 3600,
            (self.frequency <= Frequency::Hourly).then_some(hour),
        ) else {
            return Ok(true);
        };
        let Some(minutes) = parts(
            &self.minutes,
            seed_time / 60 % 60,
            (self.frequency <= Frequency::Minutely).then_some(minute),
        ) else {
            return Ok(true);
        };
        let Some(seconds) = parts(
            &self.seconds,
            seed_time % 60,
            (self.frequency == Frequency::Secondly).then_some(second),
        ) else {
            return Ok(true);
        };
        for &hour in hours.values() {
            for &minute in minutes.values() {
                for &second in seconds.values() {
                    charge()?;
                    let at = day * DAY + hour * 3600 + minute * 60 + second;
                    if at >= start && at < end && !emit(at) {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }
}

fn weekday(day: i64) -> i64 {
    (day + 3).rem_euclid(7)
}

fn week_floor(day: i64, week_start: i64) -> i64 {
    day - (weekday(day) - week_start).rem_euclid(7)
}

fn week_year(year: i64, day: i64, start: i64) -> i64 {
    if day < week_one(year, start) {
        year - 1
    } else if day >= week_one(year + 1, start) {
        year + 1
    } else {
        year
    }
}

fn week_one(year: i64, week_start: i64) -> i64 {
    week_floor(dates::day_number(year, 1, 4), week_start)
}

fn includes(values: &[i64], value: i64, period: i64) -> bool {
    values.is_empty() || values.contains(&value) || values.contains(&(value - period - 1))
}

enum Parts<'a> {
    One(i64),
    Many(&'a [i64]),
}

impl Parts<'_> {
    fn values(&self) -> &[i64] {
        match self {
            Self::One(value) => std::slice::from_ref(value),
            Self::Many(values) => values,
        }
    }
}

fn parts(allowed: &[i64], fallback: i64, fixed: Option<i64>) -> Option<Parts<'_>> {
    if let Some(value) = fixed {
        (allowed.is_empty() || allowed.contains(&value)).then_some(Parts::One(value))
    } else if allowed.is_empty() {
        Some(Parts::One(fallback))
    } else {
        Some(Parts::Many(allowed))
    }
}
