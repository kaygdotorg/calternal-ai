use super::*;

fn day(day: &str) -> Value {
    Value::Date(day.into())
}

#[test]
fn picker_rules_keep_the_seed_phase() {
    for (text, seed, after, next) in [
        (
            "FREQ=WEEKLY;BYDAY=MO,TH,FR",
            "2026-11-23",
            "2026-11-24",
            "2026-11-26",
        ),
        (
            "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,FR",
            "2026-11-23",
            "2026-11-30",
            "2026-12-07",
        ),
        (
            "FREQ=DAILY;INTERVAL=3",
            "2026-11-23",
            "2026-11-24",
            "2026-11-26",
        ),
        ("FREQ=MONTHLY", "2026-01-31", "2026-01-31", "2026-03-31"),
        (
            "FREQ=MONTHLY;BYDAY=-1FR",
            "2026-01-30",
            "2026-02-01",
            "2026-02-27",
        ),
        (
            "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=29",
            "2024-02-29",
            "2024-03-01",
            "2028-02-29",
        ),
        (
            "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1",
            "2026-01-30",
            "2026-02-01",
            "2026-02-27",
        ),
        (
            "FREQ=YEARLY;BYYEARDAY=-1",
            "2025-12-31",
            "2026-01-01",
            "2026-12-31",
        ),
        (
            "FREQ=YEARLY;BYWEEKNO=1;BYDAY=MO",
            "2025-12-29",
            "2026-01-01",
            "2027-01-04",
        ),
    ] {
        let rule = parse_repeat(text).unwrap();
        assert_eq!(
            next_occurrence(&rule, seed, after).unwrap(),
            Some(day(next)),
            "{text}"
        );
    }
}

#[test]
fn short_windows_apply_exclusions_and_detached_slots() {
    let rule = parse_repeat("FREQ=DAILY;COUNT=5").unwrap();
    let occurrences = expand_repeat(
        &rule,
        "2026-11-23",
        "2026-11-24",
        "2026-11-28",
        &["2026-11-25"],
        &[
            OccurrenceOverride {
                original: "2026-11-23",
                start: "2026-11-26",
            },
            OccurrenceOverride {
                original: "2026-11-27",
                start: "2026-11-29",
            },
        ],
    )
    .unwrap();
    assert_eq!(
        occurrences,
        [
            Occurrence {
                original: day("2026-11-24"),
                start: day("2026-11-24")
            },
            Occurrence {
                original: day("2026-11-23"),
                start: day("2026-11-26")
            },
            Occurrence {
                original: day("2026-11-26"),
                start: day("2026-11-26")
            },
        ]
    );
    for rule in [
        "FREQ=DAILY;COUNT=5",
        "FREQ=DAILY;UNTIL=20261127",
        "FREQ=DAILY;INTERVAL=4000",
    ] {
        assert_eq!(
            next_occurrence(&parse_repeat(rule).unwrap(), "2026-11-23", "2026-11-27").unwrap(),
            None
        );
    }
}

#[test]
fn timed_rules_keep_wall_time_and_check_utc_until() {
    let rule = parse_repeat("FREQ=MINUTELY;INTERVAL=15;UNTIL=20261127T084500Z").unwrap();
    assert_eq!(
        next_occurrence(
            &rule,
            "2026-11-27T09:00:00+01:00",
            "2026-11-27T09:41:00+01:00"
        )
        .unwrap(),
        Some(Value::Time("2026-11-27T09:45:00+01:00".into()))
    );
    assert_eq!(
        next_occurrence(
            &rule,
            "2026-11-27T09:00:00+01:00",
            "2026-11-27T09:45:00+01:00"
        )
        .unwrap(),
        None
    );
    assert_eq!(
        next_occurrence(
            &parse_repeat("FREQ=SECONDLY;COUNT=1000000").unwrap(),
            "2026-11-20T09:00",
            "2026-11-27T09:00"
        )
        .unwrap(),
        Some(Value::Time("2026-11-27T09:00:01".into()))
    );
    for bad in [
        "FREQ=WEEKLY;BYDAY=1MO",
        "FREQ=DAILY;COUNT=0",
        "FREQ=DAILY;UNTIL=20261127junk",
        "FREQ=MONTHLY;BYMONTH=13",
        "FREQ=YEARLY;BYSETPOS=1",
        "FREQ=DAILY;COUNT=2;UNTIL=20261127",
    ] {
        assert!(parse_repeat(bad).is_err(), "{bad}");
    }
}

#[test]
fn recurrence_charges_exceptions_before_parsing_and_deduplicates_weekdays() {
    let rule = parse_repeat("FREQ=DAILY;BYDAY=MO,MO,TU,MO").unwrap();
    assert_eq!(rule.weekdays.len(), 2);
    let window = Window::parse("2026-11-27", "2026-11-28").unwrap();
    let error = expand_series(&rule, "2026-11-27", window, &["bad date"], &[], &mut 0).unwrap_err();
    assert_eq!(error.message, "The recurrence is too costly");
}

#[test]
fn moved_occurrences_hit_the_visible_limit_before_scanning_the_window() {
    let rule = parse_repeat("FREQ=DAILY").unwrap();
    let seed = dates::day("2020-01-01").unwrap();
    let originals = (0..1_001)
        .map(|day| dates::render_day(seed + day).unwrap())
        .collect::<Vec<_>>();
    let overrides = originals
        .iter()
        .map(|date| OccurrenceOverride {
            original: date,
            start: "2026-11-27",
        })
        .collect::<Vec<_>>();
    let site = Site::START;
    let seed = parse_stamp("2020-01-01").unwrap();
    let mut validation_steps = 100_000;
    for original in &originals {
        let original = parse_stamp(original).unwrap().at;
        rule.search(
            &seed,
            original,
            original + 1,
            &mut validation_steps,
            site,
            |_| false,
        )
        .unwrap();
    }
    let mut steps = 100_000 - validation_steps + overrides.len();
    let window = Window::parse("2026-11-27", "2026-11-28").unwrap();
    let error =
        expand_series(&rule, "2020-01-01", window, &[], &overrides, &mut steps).unwrap_err();
    assert!(error.message.contains("more than 1,000 occurrences"));
}
