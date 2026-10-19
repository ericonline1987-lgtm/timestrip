//! Settings, stored as JSON so they can also be edited by hand.
//!
//! Location: `~/Library/Application Support/TimeStrip/config.json` on macOS.
//! Every field has a default, so older or hand-trimmed files keep working.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// Per-region overrides. Anything left as `None` uses the global setting.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ZoneStyle {
    pub font_size: Option<f32>,
    pub font_family: Option<String>,
    pub font_weight: Option<u16>,
    pub color: Option<[u8; 3]>,
    pub show_label: Option<bool>,
    pub use_24h: Option<bool>,
    pub show_seconds: Option<bool>,
    pub show_ampm: Option<bool>,
    pub custom_time_format: Option<String>,
    pub date_mode: Option<DateMode>,
    pub date_format: Option<String>,
}

impl ZoneStyle {
    pub fn is_empty(&self) -> bool {
        *self == ZoneStyle::default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Zone {
    /// IANA time zone name, e.g. "Asia/Tokyo".
    pub tz: String,
    /// Your own short label, e.g. "TYO". Empty = city name.
    pub label: String,
    /// Show in the menu bar (otherwise only in the panel and dropdown menu).
    pub show_in_bar: bool,
    /// Display settings for this region only.
    pub style: ZoneStyle,
}

impl Default for Zone {
    fn default() -> Self {
        Zone { tz: "UTC".into(), label: String::new(), show_in_bar: true, style: ZoneStyle::default() }
    }
}

impl Zone {
    pub fn new(tz: &str, label: &str) -> Self {
        Zone { tz: tz.into(), label: label.into(), ..Zone::default() }
    }

    pub fn city(&self) -> String {
        city_name(&self.tz)
    }

    pub fn display_name(&self) -> String {
        if self.label.trim().is_empty() { self.city() } else { self.label.clone() }
    }
}

pub fn city_name(tz: &str) -> String {
    tz.rsplit('/').next().unwrap_or(tz).replace('_', " ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// "NYC 8:35 PM   TYO 9:35 AM"
    Inline,
    /// Small label above a larger time, one column per clock.
    Stacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateMode {
    Never,
    /// Only when that zone is on a different calendar day than you.
    WhenDifferent,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickAction {
    /// Clock + calendar panel (right-click shows the menu).
    Panel,
    /// The dropdown menu.
    Menu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventRegions {
    /// Show each event's time in every region.
    All,
    /// Only in the regions pinned to the menu bar.
    MenuBar,
    /// Only in the region the calendar is showing.
    Selected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CalendarConfig {
    /// 0 = Sunday, 1 = Monday, … 6 = Saturday.
    pub first_weekday: u8,
    pub show_week_numbers: bool,
    pub highlight_weekends: bool,
    pub show_adjacent_months: bool,
    /// Region the calendar opens on: an IANA name, or empty for your own time zone.
    pub default_region: String,
    /// Independent region-tab order. Empty string means Local; omitted regions follow.
    pub region_order: Vec<String>,
    /// Calendar identifiers used as holiday sources, keyed by region (empty = Local).
    pub holiday_calendars: BTreeMap<String, Vec<String>>,
    /// Read events from the macOS Calendar app.
    pub show_events: bool,
    /// Calendar identifiers you've turned off.
    pub hidden_calendars: Vec<String>,
    pub event_regions: EventRegions,
    /// Days of upcoming events listed under the month (0 = only the selected day).
    pub upcoming_days: u32,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        CalendarConfig {
            first_weekday: 0,
            show_week_numbers: false,
            highlight_weekends: true,
            show_adjacent_months: true,
            default_region: String::new(),
            region_order: Vec::new(),
            holiday_calendars: BTreeMap::new(),
            show_events: true,
            hidden_calendars: Vec::new(),
            event_regions: EventRegions::All,
            upcoming_days: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelConfig {
    pub left_click: ClickAction,
    /// Overall size of the panel (1.0 = normal).
    pub zoom: f32,
    pub width: f32,
    pub show_clocks: bool,
    pub show_time_slider: bool,
    pub show_calendar: bool,
}

impl Default for PanelConfig {
    fn default() -> Self {
        PanelConfig {
            left_click: ClickAction::Panel,
            zoom: 1.0,
            width: 360.0,
            show_clocks: true,
            show_time_slider: true,
            show_calendar: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub zones: Vec<Zone>,
    /// Show the permanent Local clock in the menu bar.
    pub show_local_in_bar: bool,

    // ---- Language & time zone ----
    /// Interface language code ("en", "zh-Hans", "ja", …); empty = follow the system.
    pub language: String,
    /// Language for weekday and month names; empty = same as the interface.
    pub date_language: String,
    /// IANA time zone used as "Local"; empty = the system time zone.
    pub local_time_zone: String,

    // ---- Menu bar appearance (defaults for every region) ----
    pub layout: Layout,
    /// Font family name; empty = the system font.
    pub font_family: String,
    /// Text size in points (the menu bar is ~22 pt tall; larger sizes are scaled to fit).
    pub font_size: f32,
    /// 100 (thin) … 900 (black).
    pub font_weight: u16,
    /// Stacked layout: label size relative to the time.
    pub label_scale: f32,
    /// Give every digit the same width so the clock doesn't jiggle.
    pub tabular_digits: bool,
    /// Gap between clocks, in points.
    pub spacing: f32,
    /// Optional text drawn between clocks, e.g. "·" or "|".
    pub separator: String,
    /// Nudge the text up (+) or down (−), in points.
    pub vertical_offset: f32,
    /// Extra space between letters, in points.
    pub letter_spacing: f32,
    pub show_labels: bool,
    pub dim_labels: bool,
    /// How faint dimmed labels are (0 = invisible, 1 = same as the time).
    pub dim_amount: f32,
    pub use_custom_color: bool,
    /// sRGB color used when `use_custom_color` is on.
    pub color: [u8; 3],

    // ---- Time & date (defaults for every region) ----
    pub use_24h: bool,
    pub show_ampm: bool,
    pub lowercase_ampm: bool,
    pub show_seconds: bool,
    /// Optional strftime-style time format that overrides the options above (e.g. "%H.%M").
    pub custom_time_format: String,
    pub date_mode: DateMode,
    /// strftime-style date format, e.g. "%a" (Fri) or "%a %b %-d" (Fri Oct 16).
    pub date_format: String,

    /// In single-line menu bar clocks, place the date before the time.
    pub date_before_time: bool,

    // ---- Dropdown menu ----
    pub menu_show_offsets: bool,
    pub menu_show_dates: bool,

    // ---- Panel & calendar ----
    pub panel: PanelConfig,
    pub calendar: CalendarConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            zones: vec![
                Zone::new("America/New_York", "NYC"),
                Zone::new("Europe/London", "LON"),
                Zone::new("Asia/Tokyo", "TYO"),
            ],
            show_local_in_bar: true,
            language: String::new(),
            date_language: String::new(),
            local_time_zone: String::new(),
            layout: Layout::Inline,
            font_family: String::new(),
            font_size: 13.0,
            font_weight: 400,
            label_scale: 0.7,
            tabular_digits: true,
            spacing: 12.0,
            separator: String::new(),
            vertical_offset: 0.0,
            letter_spacing: 0.0,
            show_labels: true,
            dim_labels: true,
            dim_amount: 0.6,
            use_custom_color: false,
            color: [255, 159, 10],
            use_24h: false,
            show_ampm: true,
            lowercase_ampm: false,
            show_seconds: false,
            custom_time_format: String::new(),
            date_mode: DateMode::WhenDifferent,
            date_format: "%a".into(),
            date_before_time: false,
            menu_show_offsets: true,
            menu_show_dates: true,
            panel: PanelConfig::default(),
            calendar: CalendarConfig::default(),
        }
    }
}

impl Config {
    /// Compact date and time in the menu bar, keeping region overrides and other settings.
    pub fn apply_date_time_preset(&mut self) {
        self.layout = Layout::Inline;
        self.use_24h = true;
        self.show_ampm = false;
        self.show_seconds = false;
        self.custom_time_format.clear();
        self.date_mode = DateMode::Always;
        self.date_format = "%a %b %-d".into();
        self.date_before_time = true;
        self.dim_labels = false;
    }

    /// Calendar tabs always include Local and every configured region, once each.
    pub fn calendar_regions(&self) -> Vec<String> {
        let available = std::iter::once(String::new()).chain(self.zones.iter().map(|z| z.tz.clone()));
        let mut order = Vec::new();
        for key in self.calendar.region_order.iter().cloned().chain(available) {
            if (key.is_empty() || self.zones.iter().any(|z| z.tz == key)) && !order.contains(&key) {
                order.push(key);
            }
        }
        order
    }

    /// Default menu bar appearance, keeping clocks, per-region styles, panel and calendar settings.
    pub fn reset_appearance(&mut self) {
        let keep = std::mem::take(self);
        *self = Config {
            zones: keep.zones,
            show_local_in_bar: keep.show_local_in_bar,
            language: keep.language,
            date_language: keep.date_language,
            local_time_zone: keep.local_time_zone,
            panel: keep.panel,
            calendar: keep.calendar,
            ..Config::default()
        };
    }

    /// The effective settings for one region: global settings with its overrides applied.
    pub fn for_zone(&self, zone: &Zone) -> Config {
        let s = &zone.style;
        let mut c = self.clone();
        if let Some(v) = s.font_size {
            c.font_size = v;
        }
        if let Some(v) = &s.font_family {
            c.font_family = v.clone();
        }
        if let Some(v) = s.font_weight {
            c.font_weight = v;
        }
        if let Some(v) = s.color {
            c.use_custom_color = true;
            c.color = v;
        }
        if let Some(v) = s.show_label {
            c.show_labels = v;
        }
        if let Some(v) = s.use_24h {
            c.use_24h = v;
        }
        if let Some(v) = s.show_seconds {
            c.show_seconds = v;
        }
        if let Some(v) = s.show_ampm {
            c.show_ampm = v;
        }
        if let Some(v) = &s.custom_time_format {
            c.custom_time_format = v.clone();
        }
        if let Some(v) = s.date_mode {
            c.date_mode = v;
        }
        if let Some(v) = &s.date_format {
            c.date_format = v.clone();
        }
        c.sanitized()
    }

    /// Clamp values to sane ranges (protects against hand-edited files).
    pub fn sanitized(mut self) -> Self {
        self.font_size = self.font_size.clamp(6.0, 60.0);
        self.font_weight = self.font_weight.clamp(100, 900);
        self.label_scale = self.label_scale.clamp(0.3, 1.5);
        self.spacing = self.spacing.clamp(0.0, 80.0);
        self.vertical_offset = self.vertical_offset.clamp(-10.0, 10.0);
        self.letter_spacing = self.letter_spacing.clamp(-2.0, 6.0);
        self.dim_amount = self.dim_amount.clamp(0.0, 1.0);
        self.panel.zoom = self.panel.zoom.clamp(0.6, 2.5);
        self.panel.width = self.panel.width.clamp(260.0, 700.0);
        self.calendar.first_weekday %= 7;
        self.calendar.upcoming_days = self.calendar.upcoming_days.min(31);
        self
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("TimeStrip")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

/// Loads the config, falling back to defaults if the file is missing or unreadable,
/// and applies its language and time zone settings to this process.
pub fn load() -> Config {
    let c = std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Config>(&s).ok())
        .unwrap_or_default()
        .sanitized();
    c.apply_globals();
    c
}

impl Config {
    /// Language and "Local" time zone are process-wide; call after changing them.
    pub fn apply_globals(&self) {
        crate::i18n::apply(&self.language, &self.date_language);
        crate::clock::set_local_override(&self.local_time_zone);
    }
}

/// Writes atomically (temp file + rename) so a reader never sees half a file.
pub fn save(config: &Config) -> std::io::Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(config)?;
    let tmp = dir.join(format!("config.json.{}.tmp", std::process::id()));
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, config_path())
}

pub fn modified_time() -> Option<SystemTime> {
    std::fs::metadata(config_path()).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_json_uses_defaults() {
        let c: Config = serde_json::from_str(r#"{ "font_size": 18, "calendar": { "first_weekday": 1 } }"#).unwrap();
        assert_eq!(c.font_size, 18.0);
        assert_eq!(c.zones.len(), 3);
        assert_eq!(c.date_mode, DateMode::WhenDifferent);
        assert_eq!(c.calendar.first_weekday, 1);
        assert!(c.calendar.show_events);
        assert!(c.calendar.region_order.is_empty());
        assert!(c.calendar.holiday_calendars.is_empty());
        assert!(!c.date_before_time);
        assert!(c.show_local_in_bar);
    }

    #[test]
    fn round_trip() {
        let mut c = Config::default();
        c.zones[0].style.font_size = Some(18.0);
        c.date_before_time = true;
        c.show_local_in_bar = false;
        c.calendar.region_order = vec!["Asia/Tokyo".into(), String::new()];
        c.calendar.holiday_calendars.insert("Asia/Tokyo".into(), vec!["holidays".into()]);
        let s = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Config>(&s).unwrap(), c);
    }

    #[test]
    fn zone_overrides_apply() {
        let mut c = Config::default();
        c.zones[2].style = ZoneStyle { font_size: Some(20.0), color: Some([1, 2, 3]), use_24h: Some(true), ..Default::default() };
        let eff = c.for_zone(&c.zones[2]);
        assert_eq!(eff.font_size, 20.0);
        assert!(eff.use_custom_color && eff.use_24h);
        assert_eq!(eff.color, [1, 2, 3]);
        let plain = c.for_zone(&c.zones[0]);
        assert_eq!(plain.font_size, 13.0);
        assert!(!plain.use_custom_color);
    }

    #[test]
    fn resetting_appearance_preserves_local_visibility() {
        let mut c = Config::default();
        c.show_local_in_bar = false;
        c.reset_appearance();
        assert!(!c.show_local_in_bar);
    }

    #[test]
    fn city_names() {
        assert_eq!(city_name("America/Los_Angeles"), "Los Angeles");
        assert_eq!(city_name("America/Argentina/Buenos_Aires"), "Buenos Aires");
        assert_eq!(city_name("UTC"), "UTC");
    }

    #[test]
    fn calendar_order_is_independent_and_keeps_all_regions() {
        let mut c = Config::default();
        c.calendar.region_order = vec!["Asia/Tokyo".into(), "removed".into(), "Asia/Tokyo".into()];
        c.zones[0].show_in_bar = false;
        c.zones.swap(0, 1);
        assert_eq!(c.calendar_regions(), vec!["Asia/Tokyo", "", "Europe/London", "America/New_York"]);
        c.zones.retain(|z| z.tz != "Asia/Tokyo");
        assert_eq!(c.calendar_regions(), vec!["", "Europe/London", "America/New_York"]);
    }
}
