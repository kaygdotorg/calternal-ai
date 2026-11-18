use super::*;

#[test]
fn phrases_have_a_canonical_round_trip() {
    for (words, stored) in [
        ("daily at 7:15", "*-*-* 07:15:00"),
        ("  Every weekday at 9 ", "Mon..Fri *-*-* 09:00:00"),
        ("every Monday 8:30", "Mon *-*-* 08:30:00"),
        ("monthly at 9", "*-*-01 09:00:00"),
        ("every month on the 31st at 5pm", "*-*-31 17:00:00"),
        ("every day at 12 am", "*-*-* 00:00:00"),
    ] {
        let schedule = Schedule::parse(words).unwrap();
        assert_eq!(schedule.expression(), stored);
        assert_eq!(Schedule::from_expression(stored), Some(schedule));
    }
}

#[test]
fn words_are_never_partly_matched_or_guessed() {
    for words in [
        "every 2 weeks at 9",
        "every Monday at 8 and Friday",
        "daily",
        "daily 9",
        "monthly on the 40th at 9",
        "monthly on the 23th at 9",
        "daily at 23pm",
        "daily at 8:61",
        "daily at 9 tomorrow",
    ] {
        assert!(Schedule::parse(words).is_err(), "{words}");
    }
}

#[test]
fn next_skips_gaps_and_uses_the_first_fold() {
    let schedule = Schedule::parse("daily at 2:30").unwrap();
    let timestamp = |value: &str| value.parse::<jiff::Timestamp>().unwrap().as_millisecond();
    assert_eq!(
        schedule.next("America/New_York", timestamp("2026-03-08T00:00Z")),
        Some(timestamp("2026-03-09T06:30Z"))
    );
    let schedule = Schedule::parse("daily at 1:30").unwrap();
    assert_eq!(
        schedule.next("America/New_York", timestamp("2026-11-01T00:00Z")),
        Some(timestamp("2026-11-01T05:30Z"))
    );
    assert_eq!(
        schedule.next("America/New_York", timestamp("2026-11-01T05:30Z")),
        Some(timestamp("2026-11-02T06:30Z"))
    );
}

#[test]
fn previously_due_occurrences_are_found_without_replaying_the_outage() {
    let s = Schedule::parse("every month on the 31st at 17:00").unwrap();
    let timestamp = |value: &str| value.parse::<jiff::Timestamp>().unwrap().as_millisecond();
    assert_eq!(
        s.latest("UTC", timestamp("2026-03-01T00:00Z")),
        Some(timestamp("2026-01-31T17:00Z"))
    );
    assert_eq!(
        s.next("UTC", timestamp("2026-03-01T00:00Z")),
        Some(timestamp("2026-03-31T17:00Z"))
    );
}
