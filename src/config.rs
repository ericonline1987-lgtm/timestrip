//! Settings, stored as JSON so they can also be edited by hand.
//!
//! Location: `~/Library/Application Support/TimeStrip/config.json` on macOS.
//! Every field has a default, so older or hand-trimmed files keep working.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Zone {
    /// IANA time zone name, e.g. "Asia/Tokyo".
    pub tz: String,
    /// Your own short label, e.g. "TYO". Empty = city name.
    pub label: String,
    /// Show in the menu bar (otherwise only in the dropdown menu).
    pub show_in_bar: bool,
}

impl Default for Zone {
    fn default() -> Self {
        Zone { tz: "UTC".into(), label: String::new(), show_in_bar: true }
    }
}

impl Zone {
    pub fn new(tz: &str, label: &str) -> Self {
        Zone { tz: tz.into(), label: label.into(), show_in_bar: true }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub zones: Vec<Zone>,

    // ---- Menu bar appearance ----
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

    // ---- Time & date ----
    pub use_24h: bool,
    pub show_ampm: bool,
    pub lowercase_ampm: bool,
    pub show_seconds: bool,
    /// Optional strftime-style time format that overrides the options above (e.g. "%H.%M").
    pub custom_time_format: String,
    pub date_mode: DateMode,
    /// strftime-style date format, e.g. "%a" (Fri) or "%a %b %-d" (Fri Oct 16).
    pub date_format: String,

    // ---- Dropdown menu ----
    pub menu_show_offsets: bool,
    pub menu_show_dates: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            zones: vec![
                Zone::new("America/New_York", "NYC"),
                Zone::new("Europe/London", "LON"),
                Zone::new("Asia/Tokyo", "TYO"),
            ],
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
            menu_show_offsets: true,
            menu_show_dates: true,
        }
    }
}

impl Config {
    /// Default appearance, keeping the current clocks.
    pub fn reset_appearance(&mut self) {
        let zones = std::mem::take(&mut self.zones);
        *self = Config { zones, ..Config::default() };
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
        self
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("TimeStrip")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

/// Loads the config, falling back to defaults if the file is missing or unreadable.
pub fn load() -> Config {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Config>(&s).ok())
        .unwrap_or_default()
        .sanitized()
}

/// Writes atomically (temp file + rename) so a reader never sees half a file.
pub fn save(config: &Config) -> std::io::Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(config)?;
    let tmp = dir.join("config.json.tmp");
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
        let c: Config = serde_json::from_str(r#"{ "font_size": 18 }"#).unwrap();
        assert_eq!(c.font_size, 18.0);
        assert_eq!(c.zones.len(), 3);
        assert_eq!(c.date_mode, DateMode::WhenDifferent);
    }

    #[test]
    fn round_trip() {
        let c = Config::default();
        let s = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Config>(&s).unwrap(), c);
    }

    #[test]
    fn city_names() {
        assert_eq!(city_name("America/Los_Angeles"), "Los Angeles");
        assert_eq!(city_name("America/Argentina/Buenos_Aires"), "Buenos Aires");
        assert_eq!(city_name("UTC"), "UTC");
    }
}
