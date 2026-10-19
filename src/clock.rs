//! Time zone math and formatting.

use crate::config::{Config, DateMode, Zone};
use chrono::format::{Item, StrftimeItems};
use chrono::{DateTime, Offset, TimeZone, Utc};
use chrono_tz::Tz;

static LOCAL_OVERRIDE: std::sync::Mutex<Option<Tz>> = std::sync::Mutex::new(None);

/// Uses `name` as "Local" instead of the system time zone (empty = system).
pub fn set_local_override(name: &str) {
    let tz = name.trim().parse::<Tz>().ok();
    *LOCAL_OVERRIDE.lock().unwrap() = tz;
}

/// The system's own time zone, falling back to UTC if it can't be determined.
pub fn system_tz() -> Tz {
    // CoreFoundation honors inherited TZ values, even when querying the system zone.
    // The macOS localtime link records the actual OS selection independently of TZ.
    #[cfg(target_os = "macos")]
    if let Some(tz) = std::fs::read_link("/etc/localtime").ok().and_then(|path| zoneinfo_tz(&path)) {
        return tz;
    }
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|name| name.parse::<Tz>().ok())
        .unwrap_or(Tz::UTC)
}

#[cfg(target_os = "macos")]
fn zoneinfo_tz(path: &std::path::Path) -> Option<Tz> {
    path.to_str()?.split_once("/zoneinfo/")?.1.parse().ok()
}

/// The time zone treated as "Local": your chosen override, or the system's.
pub fn local_tz() -> Tz {
    LOCAL_OVERRIDE.lock().unwrap().unwrap_or_else(system_tz)
}

/// "Los Angeles" for America/Los_Angeles.
pub fn city_name_of(tz: Tz) -> String {
    crate::config::city_name(tz.name())
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
    // Weekday/month names and AM/PM follow the chosen date language.
    let locale = crate::i18n::date_locale();
    let items: Vec<Item> = StrftimeItems::new_with_locale(pattern, locale).collect();
    if items.iter().any(|i| matches!(i, Item::Error)) {
        return None;
    }
    use std::fmt::Write;
    let mut out = String::new();
    write!(out, "{}", dt.format_localized_with_items(items.iter(), locale)).ok()?;
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
        return crate::i18n::t("same time").into();
    }
    let sign = if diff > 0 { "+" } else { "−" };
    let (h, m) = (diff.abs() / 3600, (diff.abs() % 3600) / 60);
    if m == 0 { format!("{sign}{h}h") } else { format!("{sign}{h}h {m}m") }
}

/// How one clock is drawn in the menu bar (global settings + that region's overrides).
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentStyle {
    pub font_family: String,
    pub font_weight: u16,
    pub font_size: f32,
    /// `None` = automatic (follows the light/dark menu bar).
    pub color: Option<[u8; 3]>,
}

/// One clock as it appears in the menu bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub label: Option<String>,
    pub time: String,
    pub date: Option<String>,
    pub style: SegmentStyle,
}

pub fn segments(now: DateTime<Utc>, local: Tz, c: &Config) -> Vec<Segment> {
    let local_zone = Zone::new(local.name(), crate::i18n::t("Local"));
    std::iter::once(&local_zone)
        .filter(|_| c.show_local_in_bar)
        .chain(c.zones.iter().filter(|z| z.show_in_bar))
        .map(|z| {
            let tz = parse_tz(&z.tz);
            let e = c.for_zone(z);
            Segment {
                label: e.show_labels.then(|| z.display_name()),
                time: time_string(now, tz, &e),
                date: bar_date(now, tz, local, &e),
                style: SegmentStyle {
                    font_family: e.font_family.clone(),
                    font_weight: e.font_weight,
                    font_size: e.font_size,
                    color: e.use_custom_color.then_some(e.color),
                },
            }
        })
        .collect()
}

/// One line for the dropdown menu, e.g. "TYO  9:35 AM · Fri Oct 16 · +16h".
pub fn menu_line(now: DateTime<Utc>, local: Tz, zone: &Zone, c: &Config) -> String {
    let tz = parse_tz(&zone.tz);
    let mut parts = vec![time_string(now, tz, &c.for_zone(zone))];
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
        time_string(now, tz, &c.for_zone(zone)),
        date_string(now, tz, "%a %b %-d")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn system_time_zone_ignores_process_override() {
        let path = std::fs::read_link("/etc/localtime").unwrap();
        let expected: Tz = path.to_str().unwrap().split_once("/zoneinfo/").unwrap().1.parse().unwrap();
        if std::env::var_os("TIMESTRIP_TZ_TEST_CHILD").is_some() {
            assert_eq!(system_tz(), expected);
            let now = at(2026, 10, 17, 6, 30);
            assert_eq!(now.with_timezone(&system_tz()).date_naive(), now.with_timezone(&expected).date_naive());
            assert_eq!(now.with_timezone(&system_tz()).time(), now.with_timezone(&expected).time());
            set_local_override("Asia/Tokyo");
            assert_eq!(local_tz(), chrono_tz::Asia::Tokyo);
            set_local_override("");
            assert_eq!(local_tz(), expected);
            return;
        }
        let injected = if expected == chrono_tz::Asia::Singapore { "America/Los_Angeles" } else { "Asia/Singapore" };
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "clock::tests::system_time_zone_ignores_process_override", "--nocapture"])
            .env("TZ", injected)
            .env("TIMESTRIP_TZ_TEST_CHILD", "1")
            .output().unwrap();
        assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_zoneinfo_paths_resolve_iana_names() {
        use std::path::Path;
        assert_eq!(zoneinfo_tz(Path::new("/var/db/timezone/zoneinfo/America/Los_Angeles")), Some(chrono_tz::America::Los_Angeles));
        assert_eq!(zoneinfo_tz(Path::new("/private/var/db/timezone/zoneinfo/Asia/Kathmandu")), Some(chrono_tz::Asia::Kathmandu));
        assert_eq!(zoneinfo_tz(Path::new("/var/db/timezone/zoneinfo/Unknown")), None);
        assert_eq!(zoneinfo_tz(Path::new("/tmp/localtime")), None);
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

    #[test]
    fn menu_bar_order_visibility_and_custom_formats() {
        let mut c = Config::default();
        c.show_local_in_bar = false;
        c.zones.swap(0, 2);
        c.zones[1].show_in_bar = false;
        c.custom_time_format = "%H.%M".into();
        c.zones[0].style.custom_time_format = Some("%H:%M:%S".into());
        let shown = segments(at(2026, 10, 16, 0, 35), Tz::UTC, &c);
        assert_eq!(shown.len(), 2);
        assert_eq!(shown[0].label.as_deref(), Some("TYO"));
        assert_eq!(shown[0].time, "09:35:00");
        assert_eq!(shown[1].label.as_deref(), Some("NYC"));
        assert_eq!(shown[1].time, "20.35");
        assert_eq!(c.zones.len(), 3);
    }

    #[test]
    fn local_clock_survives_region_removal_and_can_be_hidden() {
        let now = at(2026, 10, 16, 0, 35);
        let local = chrono_tz::Asia::Singapore;
        let mut c: Config = serde_json::from_str(r#"{"zones":[],"use_24h":true}"#).unwrap();
        let shown = segments(now, local, &c);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].label.as_deref(), Some("Local"));
        assert_eq!(shown[0].time, "08:35");
        c.zones.push(Zone::new("Asia/Tokyo", "TYO"));
        assert_eq!(segments(now, local, &c).len(), 2);
        c.zones.clear();
        assert_eq!(segments(now, local, &c), shown);
        c.show_local_in_bar = false;
        assert!(segments(now, local, &c).is_empty());
        assert_eq!(c.calendar_regions(), vec![""]);
        c.show_local_in_bar = true;
        assert_eq!(segments(now, local, &c), shown);
    }

    #[test]
    fn local_visibility_leaves_region_visibility_and_styles_unchanged() {
        let now = at(2026, 10, 16, 0, 35);
        let mut c = Config::default();
        c.zones[0].show_in_bar = false;
        c.zones[1].style.custom_time_format = Some("%H:%M:%S".into());
        let with_local = segments(now, Tz::UTC, &c);
        c.show_local_in_bar = false;
        assert_eq!(with_local[1..], segments(now, Tz::UTC, &c));
    }
}
