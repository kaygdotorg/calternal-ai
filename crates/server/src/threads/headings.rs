//! Presentation-only labels and offset-aware timestamps.
use calternal_types::Error;
use time::{
    OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339, macros::format_description,
};

const HEADING_TIME: &[time::format_description::BorrowedFormatItem<'_>] = format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]"
);

/// Escape generated labels only. Replace reserved heading separators with entities.
pub(super) fn label(input: &str) -> String {
    let input = input.replace(char::is_control, " ");
    let mut escaped = String::with_capacity(input.len());
    for ch in input.trim().chars() {
        if "\\#[]*_~<>&!()`".contains(ch) {
            escaped.push('\\');
        }
        if ch == '·' {
            escaped.push_str("&#183;");
        } else {
            escaped.push(ch);
        }
    }
    escaped.replace(" via ", " &#118;ia ")
}

/// Resolve the caller's names and private zone together on the authority worker.
pub(super) fn profile<D: crate::auth::Directory>(
    state: &crate::api::Shared<D>,
    account: calternal_types::AccountId,
) -> Result<(String, Option<String>), Error> {
    Ok(state
        .directory
        .protected_spaces()
        .map(|spaces| {
            spaces
                .own_heading_profile(account)
                .map_err(crate::auth::auth_error)
        })
        .transpose()?
        .flatten()
        .unwrap_or_else(|| ("Person".into(), None)))
}

/// Format the write instant in the named zone, falling back to the device offset.
pub(super) fn now(zone: Option<&str>, offset_minutes: i64) -> Result<String, Error> {
    let offset = i32::try_from(offset_minutes)
        .ok()
        .and_then(|minutes| minutes.checked_mul(60))
        .and_then(|seconds| UtcOffset::from_whole_seconds(seconds).ok())
        .ok_or_else(|| Error::invalid_input("invalid client timezone offset"))?;
    let at = OffsetDateTime::now_utc();
    let zone_offset = zone
        .and_then(|zone| calternal_core::time_zone::offset_seconds(zone, at.unix_timestamp()))
        .and_then(|seconds| UtcOffset::from_whole_seconds(seconds).ok());
    let at = at.to_offset(zone_offset.unwrap_or(offset));
    // RFC 3339's Z is valid, but the note convention uses an explicit offset.
    let value = at
        .format(&Rfc3339)
        .map_err(|_| Error::internal("could not format thread time"))?;
    Ok(if let Some(utc) = value.strip_suffix('Z') {
        format!("{utc}+00:00")
    } else {
        value
    })
}

/// Format a saved instant using its historical offset.
pub(super) fn time(stamp: &str) -> Result<String, Error> {
    let at = OffsetDateTime::parse(stamp, &Rfc3339)
        .map_err(|_| Error::internal("invalid thread time"))?;
    at.format(HEADING_TIME)
        .map_err(|_| Error::internal("invalid thread time"))
}

/// Parse the exact timestamp field of a generated heading.
pub(super) fn parse_time(stamp: &str) -> Result<(String, OffsetDateTime), Error> {
    let at = OffsetDateTime::parse(stamp, HEADING_TIME)
        .map_err(|_| Error::internal("invalid thread heading timestamp"))?;
    at.format(&Rfc3339)
        .map(|formatted| (formatted, at))
        .map_err(|_| Error::internal("invalid thread time"))
}

/// Render known execution facts. A requested effort remains distinct from an
/// unreported effective effort. Missing provider metadata stays unknown.
pub(super) fn agent(
    harness: &str,
    provider: &str,
    model: &str,
    reasoning: Option<&str>,
    stamp: &str,
    timing: &serde_json::Value,
    continuation: &str,
) -> Result<String, Error> {
    let effort = reasoning.map_or_else(
        || "Provider default".into(),
        |level| format!("{} requested", label(level)),
    );
    let duration = match timing["duration_ms"].as_u64() {
        None if timing.is_null() => "In progress".into(),
        None => "Duration unavailable".into(),
        Some(duration) => {
            let seconds = duration / 1000;
            if seconds < 60 {
                format!("{seconds} s")
            } else {
                format!("{} min {} s", seconds / 60, seconds % 60)
            }
        }
    };
    let outcome = match timing["outcome"].as_str() {
        Some("failed") => " · Failed",
        Some("cancelled") => " · Cancelled",
        Some("interrupted" | "outcome_unknown") => " · Interrupted",
        _ => "",
    };
    let thought = timing["reasoning_ms"]
        .as_u64()
        .filter(|ms| *ms > 0)
        .map_or_else(String::new, |ms| {
            format!(" · Thought for {:.1} s", ms as f64 / 1_000.0)
        });
    Ok(format!(
        "{} via {} · {} · {effort} · {} · {duration}{outcome}{continuation}{thought}",
        label(harness),
        label(provider),
        label(model),
        time(stamp)?
    ))
}
