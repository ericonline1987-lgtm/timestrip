//! Month-grid math for the calendar panel (no UI, so it's easy to test).

use chrono::{Datelike, Duration, NaiveDate, Weekday};

/// Number of days from Sunday (0 = Sunday … 6 = Saturday).
pub fn weekday_index(w: Weekday) -> u8 {
    w.num_days_from_sunday() as u8
}

/// Short weekday names (in the date language) starting at `first_weekday` (0 = Sunday).
pub fn weekday_labels(first_weekday: u8) -> [String; 7] {
    let locale = crate::i18n::date_locale();
    // 2026-02-01 was a Sunday.
    let sunday = NaiveDate::from_ymd_opt(2026, 2, 1).expect("valid date");
    std::array::from_fn(|i| {
        let d = sunday + Duration::days(((first_weekday as usize + i) % 7) as i64);
        let name = d.format_localized("%a", locale).to_string();
        // Keep English labels compact: "Su", "Mo", …
        if crate::i18n::date_lang() == crate::i18n::Lang::En {
            name.chars().take(2).collect()
        } else {
            name.trim_end_matches('.').to_string()
        }
    })
}

/// Always six weeks (so the panel doesn't change height between months), each starting on
/// `first_weekday`, covering the whole of `year`/`month`.
pub fn month_grid(year: i32, month: u32, first_weekday: u8) -> Vec<[NaiveDate; 7]> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("valid month");
    let back = (weekday_index(first.weekday()) as i64 - first_weekday as i64).rem_euclid(7);
    let start = first - Duration::days(back);
    (0..6)
        .map(|w| std::array::from_fn(|d| start + Duration::days(w * 7 + d as i64)))
        .collect()
}

/// Moves a (year, month) pair by `delta` months.
pub fn shift_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let index = year * 12 + month as i32 - 1 + delta;
    (index.div_euclid(12), (index.rem_euclid(12) + 1) as u32)
}

/// "October 2026", "2026年10月", "2026년 10월", "octubre 2026" …
pub fn month_title(year: i32, month: u32) -> String {
    use crate::i18n::Lang;
    let d = NaiveDate::from_ymd_opt(year, month, 1).expect("valid month");
    let pattern = match crate::i18n::date_lang() {
        Lang::ZhHans | Lang::ZhHant | Lang::Ja => "%Y年%-m月",
        Lang::Ko => "%Y년 %-m월",
        _ => "%B %Y",
    };
    let title = d.format_localized(pattern, crate::i18n::date_locale()).to_string();
    // Spanish and French month names are lowercase; capitalize for a title.
    let mut chars = title.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => title,
    }
}

/// A long date like "Thursday, October 15" in the date language.
pub fn long_date(d: NaiveDate) -> String {
    use crate::i18n::Lang;
    let pattern = match crate::i18n::date_lang() {
        Lang::ZhHans | Lang::ZhHant | Lang::Ja => "%-m月%-d日 %A",
        Lang::Ko => "%-m월 %-d일 %A",
        Lang::En => "%A, %B %-d",
        _ => "%A %-d %B",
    };
    d.format_localized(pattern, crate::i18n::date_locale()).to_string()
}

/// ISO-8601 week number of the row (taken from its Thursday, per the standard).
pub fn week_number(row: &[NaiveDate; 7]) -> u32 {
    row.iter()
        .find(|d| d.weekday() == Weekday::Thu)
        .unwrap_or(&row[0])
        .iso_week()
        .week()
}

pub fn is_weekend(d: NaiveDate) -> bool {
    matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn grid_starts_on_requested_weekday() {
        // October 1, 2026 is a Thursday.
        let sun = month_grid(2026, 10, 0);
        assert_eq!(sun[0][0], d(2026, 9, 27));
        assert_eq!(sun.len(), 6);
        let mon = month_grid(2026, 10, 1);
        assert_eq!(mon[0][0], d(2026, 9, 28));
        assert!(mon.iter().flatten().any(|x| *x == d(2026, 10, 31)));
    }

    #[test]
    fn month_starting_on_first_weekday() {
        // February 2026 starts on a Sunday.
        assert_eq!(month_grid(2026, 2, 0)[0][0], d(2026, 2, 1));
    }

    #[test]
    fn shifting_months() {
        assert_eq!(shift_month(2026, 12, 1), (2027, 1));
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(shift_month(2026, 10, -22), (2024, 12));
    }

    #[test]
    fn labels_rotate() {
        assert_eq!(weekday_labels(1)[0], "Mo");
        assert_eq!(weekday_labels(1)[6], "Su");
        assert_eq!(month_title(2026, 10), "October 2026");
        assert_eq!(long_date(d(2026, 10, 15)), "Thursday, October 15");
    }

    #[test]
    fn iso_weeks() {
        let grid = month_grid(2026, 10, 1);
        assert_eq!(week_number(&grid[0]), 40); // Mon Sep 28 – Sun Oct 4, 2026
    }
}
