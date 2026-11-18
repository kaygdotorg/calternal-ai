use jiff::{Timestamp, tz::TimeZone};

pub(crate) fn named(name: &str) -> Option<TimeZone> {
    if name.len() > 128
        || !name.split('/').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+'))
        })
    {
        return None;
    }
    let zone = TimeZone::get(name).ok()?;
    zone.iana_name()?;
    Some(zone)
}

/// Resolve a bounded IANA name and return the database spelling.
/// Uses Jiff's shared database, which caches loaded zone rules.
pub fn canonical(name: &str) -> Option<String> {
    Some(named(name)?.iana_name()?.to_owned())
}

/// Resolve a named zone's civil offset at an exact sign-in or message instant.
/// Missing rules or an out-of-range instant return no offset.
pub fn offset_seconds(name: &str, epoch_seconds: i64) -> Option<i32> {
    Some(
        named(name)?
            .to_offset(Timestamp::from_second(epoch_seconds).ok()?)
            .seconds(),
    )
}

/// Discover this device's named zone. Call once before entering an async
/// runtime because discovery may consult local operating-system files.
pub fn device() -> Option<String> {
    Some(TimeZone::system().iana_name()?.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_offset_follows_daylight_saving_at_the_instant() {
        assert_eq!(canonical("asia/kolkata").as_deref(), Some("Asia/Kolkata"));
        assert_eq!(canonical("Etc/Unknown"), None);
        assert_eq!(canonical("../../etc/passwd"), None);
        assert_eq!(
            offset_seconds("America/New_York", 1_767_225_600),
            Some(-18_000)
        );
        assert_eq!(
            offset_seconds("America/New_York", 1_782_864_000),
            Some(-14_400)
        );
    }
}
