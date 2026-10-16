//! Time zone math and formatting.

use crate::config::{Config, DateMode, Zone};
use chrono::format::{Item, StrftimeItems};
use chrono::{DateTime, Offset, TimeZone, Utc};
use chrono_tz::Tz;

/// The time zone you're in, falling back to UTC if it can't be determined.
pub fn local_tz() -> Tz {
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|name| name.parse::<Tz>().ok())
        .unwrap_or(Tz::UTC)
}

pub fn parse_tz(name: &str) -> Tz {
    name.parse::<Tz>().unwrap_or(Tz::UTC)
}

/// All IANA zone names, sorted.
pub fn all_zone_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = chrono_tz::TZ_VARIANTS.iter().map(|tz| tz.name()).collect();
    v.sort_unstable();
    v
}

/// Formats with a strftime-style pattern, returning `None` if the pattern is invalid
/// (chrono would otherwise panic while printing).
pub fn safe_format<T: TimeZone>(dt: &DateTime<T>, pattern: &str) -> Option<String>
where
    T::Offset: std::fmt::Display,
{
    let items: Vec<Item> = StrftimeItems::new(pattern).collect();
    if items.iter().any(|i| matches!(i, Item::Error)) {
        return None;
    }
    use std::fmt::Write;
    let mut out = String::new();
    write!(out, "{}", dt.format_with_items(items.iter())).ok()?;
    Some(out)
}

pub fn time_pattern(c: &Config) -> String {
    if !c.custom_time_format.trim().is_empty() {
        return c.custom_time_format.clone();
    }
    let mut p = String::from(if c.use_24h { "%H:%M" } else { "%-I:%M" });
    if c.show_seconds {
        p.push_str(":%S");
    }
    if !c.use_24h && c.show_ampm {
        p.push_str(if c.lowercase_ampm { " %P" } else { " %p" });
    }
    p
}

pub fn time_string(now: DateTime<Utc>, tz: Tz, c: &Config) -> String {
    let dt = now.with_timezone(&tz);
    safe_format(&dt, &time_pattern(c)).unwrap_or_else(|| "bad time format".into())
}

pub fn date_string(now: DateTime<Utc>, tz: Tz, pattern: &str) -> String {
    let dt = now.with_timezone(&tz);
    safe_format(&dt, pattern).unwrap_or_else(|| "bad date format".into())
}

/// Whether the calendar day in `tz` differs from the day in `local`.
pub fn day_differs(now: DateTime<Utc>, tz: Tz, local: Tz) -> bool {
    now.with_timezone(&tz).date_naive() != now.with_timezone(&local).date_naive()
}

/// The date text for the menu bar, or `None` when it shouldn't be shown.
pub fn bar_date(now: DateTime<Utc>, tz: Tz, local: Tz, c: &Config) -> Option<String> {
    match c.date_mode {
        DateMode::Never => None,
        DateMode::Always => Some(date_string(now, tz, &c.date_format)),
        DateMode::WhenDifferent => {
            day_differs(now, tz, local).then(|| date_string(now, tz, &c.date_format))
        }
    }
}

fn offset_seconds(now: DateTime<Utc>, tz: Tz) -> i32 {
    now.with_timezone(&tz).offset().fix().local_minus_utc()
}

/// e.g. "+9h", "−3h 30m", "same time".
pub fn offset_string(now: DateTime<Utc>, tz: Tz, local: Tz) -> String {
    let diff = offset_seconds(now, tz) - offset_seconds(now, local);
    if diff == 0 {
        return "same time".into();
    }
    let sign = if diff > 0 { "+" } else { "−" };
    let (h, m) = (diff.abs() / 3600, (diff.abs() % 3600) / 60);
    if m == 0 { format!("{sign}{h}h") } else { format!("{sign}{h}h {m}m") }
}

/// One clock as it appears in the menu bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub label: Option<String>,
    pub time: String,
    pub date: Option<String>,
}

pub fn segments(now: DateTime<Utc>, local: Tz, c: &Config) -> Vec<Segment> {
    c.zones
        .iter()
        .filter(|z| z.show_in_bar)
        .map(|z| {
            let tz = parse_tz(&z.tz);
            Segment {
                label: c.show_labels.then(|| z.display_name()),
                time: time_string(now, tz, c),
                date: bar_date(now, tz, local, c),
            }
        })
        .collect()
}

/// One line for the dropdown menu, e.g. "TYO  9:35 AM · Fri Oct 16 · +16h".
pub fn menu_line(now: DateTime<Utc>, local: Tz, zone: &Zone, c: &Config) -> String {
    let tz = parse_tz(&zone.tz);
    let mut parts = vec![time_string(now, tz, c)];
    if c.menu_show_dates {
        parts.push(date_string(now, tz, "%a %b %-d"));
    }
    if c.menu_show_offsets {
        parts.push(offset_string(now, tz, local));
    }
    let name = if zone.label.trim().is_empty() {
        zone.city()
    } else {
        format!("{} ({})", zone.label.trim(), zone.city())
    };
    format!("{name}  —  {}", parts.join("  ·  "))
}

/// Plain-text line used when copying times to the clipboard.
pub fn copy_line(now: DateTime<Utc>, zone: &Zone, c: &Config) -> String {
    let tz = parse_tz(&zone.tz);
    format!(
        "{}: {}, {}",
        zone.display_name(),
        time_string(now, tz, c),
        date_string(now, tz, "%a %b %-d")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    #[test]
    fn twelve_and_twenty_four_hour() {
        let now = at(2026, 10, 16, 0, 35); // 09:35 in Tokyo
        let mut c = Config::default();
        assert_eq!(time_string(now, chrono_tz::Asia::Tokyo, &c), "9:35 AM");
        c.use_24h = true;
        assert_eq!(time_string(now, chrono_tz::Asia::Tokyo, &c), "09:35");
        c.show_seconds = true;
        assert_eq!(time_string(now, chrono_tz::Asia::Tokyo, &c), "09:35:00");
    }

    #[test]
    fn bad_patterns_do_not_panic() {
        let mut c = Config::default();
        c.custom_time_format = "%Q%".into();
        assert_eq!(time_string(Utc::now(), Tz::UTC, &c), "bad time format");
    }

    #[test]
    fn offsets_and_days() {
        let now = at(2026, 10, 16, 0, 35); // Oct 15 17:35 in Los Angeles
        let la = chrono_tz::America::Los_Angeles;
        assert_eq!(offset_string(now, chrono_tz::Asia::Tokyo, la), "+16h");
        assert_eq!(offset_string(now, chrono_tz::Asia::Kolkata, la), "+12h 30m");
        assert_eq!(offset_string(now, la, la), "same time");
        assert!(day_differs(now, chrono_tz::Asia::Tokyo, la));
        assert!(!day_differs(now, chrono_tz::America::New_York, la));
    }

    #[test]
    fn date_only_when_different() {
        let now = at(2026, 10, 16, 0, 35);
        let la = chrono_tz::America::Los_Angeles;
        let c = Config::default();
        assert_eq!(bar_date(now, chrono_tz::Asia::Tokyo, la, &c).as_deref(), Some("Fri"));
        assert_eq!(bar_date(now, chrono_tz::America::New_York, la, &c), None);
    }

    #[test]
    fn zone_list_is_complete() {
        let names = all_zone_names();
        assert!(names.contains(&"Asia/Tokyo"));
        assert!(names.len() > 300);
    }
}
