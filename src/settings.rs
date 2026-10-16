//! The settings window. Every change is saved immediately, and the menu bar clock and panel
//! pick it up within a second, so you can watch the real menu bar while you tweak.

use crate::clock;
use crate::config::{self, city_name, ClickAction, Config, DateMode, EventRegions, Layout, Zone, ZoneStyle};
use crate::events::{Access, CalendarInfo, EventSource};
use crate::login;
use crate::render::{Renderer, SCALE};
use chrono::Utc;
use eframe::egui::{self, Color32, ColorImage, RichText, TextureHandle, TextureOptions};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Clocks,
    Appearance,
    TimeDate,
    Calendar,
    General,
}

struct SettingsApp {
    config: Config,
    saved: Config,
    save_error: Option<String>,
    renderer: Renderer,
    families: Vec<String>,
    all_zones: Vec<&'static str>,
    tab: Tab,
    zone_query: String,
    font_query: String,
    preview: Option<(String, TextureHandle, TextureHandle)>,
    source: EventSource,
    calendars: Option<Vec<CalendarInfo>>,
}

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TimeStrip Settings")
            .with_inner_size([680.0, 720.0])
            .with_min_inner_size([540.0, 440.0]),
        ..Default::default()
    };
    let renderer = Renderer::new();
    let config = config::load();
    let mut app = SettingsApp {
        saved: config.clone(),
        config,
        save_error: None,
        families: renderer.families(),
        renderer,
        all_zones: clock::all_zone_names(),
        tab: Tab::Clocks,
        zone_query: String::new(),
        font_query: String::new(),
        preview: None,
        source: EventSource::new(),
        calendars: None,
    };
    eframe::run_ui_native("TimeStrip Settings", options, move |ui, _frame| app.ui(ui))
}

const WEIGHT_NAMES: [&str; 9] =
    ["Thin", "Extra Light", "Light", "Regular", "Medium", "Semibold", "Bold", "Extra Bold", "Black"];

const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

const DATE_PRESETS: [(&str, &str); 6] = [
    ("%a", "Weekday"),
    ("%a %-d", "Weekday + day"),
    ("%b %-d", "Month + day"),
    ("%a %b %-d", "Weekday, month, day"),
    ("%-m/%-d", "Numeric (US)"),
    ("%-d/%-m", "Numeric (intl)"),
];

fn weight_name(w: u16) -> &'static str {
    WEIGHT_NAMES[((w / 100).clamp(1, 9) - 1) as usize]
}

fn weak(text: impl Into<String>) -> RichText {
    RichText::new(text).weak()
}

fn hint(text: impl Into<String>) -> RichText {
    RichText::new(text).small().weak()
}

impl SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.ctx().request_repaint_after(Duration::from_millis(500));
        if self.source.take_answer() {
            self.calendars = None;
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Clocks, "🌐 Regions");
                ui.selectable_value(&mut self.tab, Tab::Appearance, "🔠 Appearance");
                ui.selectable_value(&mut self.tab, Tab::TimeDate, "🕘 Time & Date");
                ui.selectable_value(&mut self.tab, Tab::Calendar, "📅 Calendar & Panel");
                ui.selectable_value(&mut self.tab, Tab::General, "⚙ General");
            });
            ui.separator();
            self.preview_ui(ui);
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
                Tab::Clocks => self.clocks_tab(ui),
                Tab::Appearance => self.appearance_tab(ui),
                Tab::TimeDate => self.time_tab(ui),
                Tab::Calendar => self.calendar_tab(ui),
                Tab::General => self.general_tab(ui),
            });
        });

        if self.config != self.saved {
            match config::save(&self.config) {
                Ok(()) => {
                    self.saved = self.config.clone();
                    self.save_error = None;
                }
                Err(e) => self.save_error = Some(format!("Couldn't save settings: {e}")),
            }
        }
    }

    /// Live preview on a light and a dark menu bar.
    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        let now = Utc::now();
        let segments = clock::segments(now, clock::local_tz(), &self.config);
        let key = format!("{segments:?}{:?}", self.config);
        let stale = self.preview.as_ref().is_none_or(|(k, _, _)| *k != key);
        if stale {
            let ctx = ui.ctx().clone();
            let mut texture = |auto: [u8; 3], white_template: bool, name: &str| -> Option<TextureHandle> {
                let img = self.renderer.render(&segments, &self.config, auto)?;
                let mut rgba = img.rgba;
                if img.template && white_template {
                    // Template images turn white on a dark menu bar.
                    for px in rgba.chunks_mut(4) {
                        px[..3].copy_from_slice(&[255, 255, 255]);
                    }
                }
                let image = ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &rgba);
                Some(ctx.load_texture(name, image, TextureOptions::LINEAR))
            };
            let light = texture([0, 0, 0], false, "preview-light-bar");
            let dark = texture([255, 255, 255], true, "preview-dark-bar");
            self.preview = light.zip(dark).map(|(l, d)| (key, l, d));
        }

        ui.label(hint("Preview"));
        match &self.preview {
            Some((_, on_light, on_dark)) => {
                for (tex, bg) in [(on_light, Color32::from_gray(236)), (on_dark, Color32::from_gray(30))] {
                    let size = tex.size_vec2() / SCALE;
                    egui::Frame::new()
                        .fill(bg)
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(10, 4))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.image(egui::load::SizedTexture::new(tex.id(), size));
                        });
                    ui.add_space(4.0);
                }
                ui.label(hint("The menu bar is about 22 pt tall; anything taller is scaled down to fit."));
            }
            None => {
                ui.label("No regions are shown in the menu bar. Tick “Menu bar” on the Regions tab.");
            }
        }
        if let Some(err) = &self.save_error {
            ui.colored_label(Color32::from_rgb(220, 60, 60), err);
        }
    }

    fn clocks_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(weak(
            "Each region can have its own label, size, font, color and time format. \
             Anything not set here uses the Appearance and Time & Date tabs.",
        ));
        ui.add_space(6.0);

        enum Action {
            Up(usize),
            Down(usize),
            Remove(usize),
        }
        let mut action = None;
        let count = self.config.zones.len();
        let global = self.config.clone();
        let families = &self.families;

        for (i, z) in self.config.zones.iter_mut().enumerate() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    if ui.add_enabled(i > 0, egui::Button::new("▲").small()).clicked() {
                        action = Some(Action::Up(i));
                    }
                    if ui.add_enabled(i + 1 < count, egui::Button::new("▼").small()).clicked() {
                        action = Some(Action::Down(i));
                    }
                    ui.add(egui::TextEdit::singleline(&mut z.label).hint_text(z.city()).desired_width(90.0));
                    ui.vertical(|ui| {
                        ui.label(z.city());
                        ui.label(hint(z.tz.clone()));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Remove").clicked() {
                            action = Some(Action::Remove(i));
                        }
                        ui.checkbox(&mut z.show_in_bar, "Menu bar");
                    });
                });
                let title = if z.style.is_empty() {
                    "Style for this region (using defaults)".to_string()
                } else {
                    "Style for this region (customized)".to_string()
                };
                egui::CollapsingHeader::new(title).id_salt(("zone-style", i)).show(ui, |ui| {
                    zone_style_ui(ui, &mut z.style, &global, families, i);
                });
            });
            ui.add_space(4.0);
        }

        match action {
            Some(Action::Up(i)) => self.config.zones.swap(i, i - 1),
            Some(Action::Down(i)) => self.config.zones.swap(i, i + 1),
            Some(Action::Remove(i)) => {
                self.config.zones.remove(i);
            }
            None => {}
        }

        ui.add_space(12.0);
        ui.heading("Add a region");
        ui.add(
            egui::TextEdit::singleline(&mut self.zone_query)
                .hint_text("Search a city or region, e.g. Tokyo, Paris, Kolkata")
                .desired_width(f32::INFINITY),
        );
        let q = self.zone_query.trim().to_lowercase();
        if !q.is_empty() {
            let matches: Vec<&'static str> = self
                .all_zones
                .iter()
                .copied()
                .filter(|name| name.to_lowercase().replace('_', " ").contains(&q))
                .take(40)
                .collect();
            if matches.is_empty() {
                ui.label(weak("No matching time zones."));
            }
            let now = Utc::now();
            let local = clock::local_tz();
            for name in matches {
                let tz = clock::parse_tz(name);
                let text = format!(
                    "{}   —   {}   ({})",
                    name.replace('_', " "),
                    clock::time_string(now, tz, &self.config),
                    clock::offset_string(now, tz, local)
                );
                if ui.button(text).clicked() {
                    let label: String = city_name(name).chars().filter(|c| c.is_alphabetic()).take(3).collect();
                    self.config.zones.push(Zone::new(name, &label.to_uppercase()));
                    self.zone_query.clear();
                }
            }
        }
    }

    fn appearance_tab(&mut self, ui: &mut egui::Ui) {
        let c = &mut self.config;
        ui.label(weak("Defaults for every region. Individual regions can override these on the Regions tab."));
        ui.add_space(6.0);

        ui.heading("Size");
        ui.add(egui::Slider::new(&mut c.font_size, 8.0..=40.0).step_by(0.5).text("Text size (pt)"));
        if c.layout == Layout::Stacked {
            ui.add(egui::Slider::new(&mut c.label_scale, 0.3..=1.2).step_by(0.05).text("Label size (× text size)"));
        }
        ui.add(egui::Slider::new(&mut c.spacing, 0.0..=60.0).step_by(1.0).text("Space between regions (pt)"));
        ui.add(egui::Slider::new(&mut c.letter_spacing, -2.0..=6.0).step_by(0.1).text("Letter spacing (pt)"));
        ui.add(egui::Slider::new(&mut c.vertical_offset, -8.0..=8.0).step_by(0.5).text("Vertical nudge (pt, + is up)"));

        ui.add_space(10.0);
        ui.heading("Layout");
        ui.horizontal(|ui| {
            ui.radio_value(&mut c.layout, Layout::Inline, "Single line");
            ui.radio_value(&mut c.layout, Layout::Stacked, "Stacked (label above time)");
        });

        ui.add_space(10.0);
        ui.heading("Font");
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.add(
                egui::TextEdit::singleline(&mut self.font_query)
                    .hint_text("e.g. mono, menlo, avenir")
                    .desired_width(160.0),
            );
        });
        font_combo(ui, "Family", "font-family", &mut c.font_family, &self.families, &self.font_query);
        ui.add(
            egui::Slider::new(&mut c.font_weight, 100..=900)
                .step_by(100.0)
                .text(format!("Weight ({})", weight_name(c.font_weight))),
        );
        ui.checkbox(&mut c.tabular_digits, "Fixed-width digits (stops the clock from jiggling)");

        ui.add_space(10.0);
        ui.heading("Style");
        ui.checkbox(&mut c.show_labels, "Show labels");
        ui.horizontal(|ui| {
            ui.checkbox(&mut c.dim_labels, "Dim labels and dates");
            ui.add_enabled(c.dim_labels, egui::Slider::new(&mut c.dim_amount, 0.15..=1.0).text("opacity"));
        });
        ui.horizontal(|ui| {
            ui.label("Separator between regions:");
            ui.add(egui::TextEdit::singleline(&mut c.separator).hint_text("none — try · | /").desired_width(70.0));
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut c.use_custom_color, "Custom color");
            if c.use_custom_color {
                ui.color_edit_button_srgb(&mut c.color);
            } else {
                ui.label(weak("(follows light/dark menu bar)"));
            }
        });

        ui.add_space(12.0);
        if ui.button("Reset appearance to defaults").clicked() {
            c.reset_appearance();
        }
    }

    fn time_tab(&mut self, ui: &mut egui::Ui) {
        let c = &mut self.config;
        let now = Utc::now();
        let local = clock::local_tz();
        ui.label(weak("Defaults for every region. Individual regions can override these on the Regions tab."));
        ui.add_space(6.0);

        ui.heading("Time");
        let custom = !c.custom_time_format.trim().is_empty();
        ui.add_enabled_ui(!custom, |ui| {
            ui.checkbox(&mut c.use_24h, "24-hour clock");
            ui.add_enabled_ui(!c.use_24h, |ui| {
                ui.checkbox(&mut c.show_ampm, "Show AM/PM");
                ui.checkbox(&mut c.lowercase_ampm, "Lowercase am/pm");
            });
            ui.checkbox(&mut c.show_seconds, "Show seconds");
        });
        ui.horizontal(|ui| {
            ui.label("Custom format:");
            ui.add(egui::TextEdit::singleline(&mut c.custom_time_format).hint_text("e.g. %H.%M").desired_width(120.0));
            ui.label(weak(format!("→ {}", clock::time_string(now, local, c))));
        });
        ui.label(hint(
            "Leave empty to use the options above. %H hour (24h), %-I hour (12h), %M minute, %S second, %p AM/PM.",
        ));

        ui.add_space(10.0);
        ui.heading("Date in the menu bar");
        ui.radio_value(&mut c.date_mode, DateMode::Never, "Never");
        ui.radio_value(&mut c.date_mode, DateMode::WhenDifferent, "Only when that region is on a different day than you");
        ui.radio_value(&mut c.date_mode, DateMode::Always, "Always");
        ui.add_enabled_ui(c.date_mode != DateMode::Never, |ui| {
            date_format_ui(ui, "date-format", &mut c.date_format);
        });

        ui.add_space(10.0);
        ui.heading("Right-click menu");
        ui.checkbox(&mut c.menu_show_dates, "Show dates");
        ui.checkbox(&mut c.menu_show_offsets, "Show difference from your time");
    }

    fn calendar_tab(&mut self, ui: &mut egui::Ui) {
        let c = &mut self.config;

        ui.heading("Panel");
        ui.label(weak("The panel opens when you click the clock in the menu bar."));
        ui.horizontal(|ui| {
            ui.label("Clicking the clock opens:");
            ui.radio_value(&mut c.panel.left_click, ClickAction::Panel, "Clocks & calendar panel");
            ui.radio_value(&mut c.panel.left_click, ClickAction::Menu, "Menu");
        });
        ui.label(hint("Right-click always opens the menu (when the panel is chosen above)."));
        ui.add(egui::Slider::new(&mut c.panel.zoom, 0.7..=2.0).step_by(0.05).text("Panel size (zoom)"));
        ui.add(egui::Slider::new(&mut c.panel.width, 280.0..=600.0).step_by(10.0).text("Panel width (pt)"));
        ui.checkbox(&mut c.panel.show_clocks, "Show every region's time");
        ui.checkbox(&mut c.panel.show_time_slider, "Show the time slider (see other times across regions)");
        ui.checkbox(&mut c.panel.show_calendar, "Show the month calendar and events");

        ui.add_space(10.0);
        ui.heading("Month view");
        egui::ComboBox::from_label("Week starts on")
            .selected_text(WEEKDAYS[c.calendar.first_weekday as usize % 7])
            .show_ui(ui, |ui| {
                for (i, name) in WEEKDAYS.iter().enumerate() {
                    ui.selectable_value(&mut c.calendar.first_weekday, i as u8, *name);
                }
            });
        let region_label = |tz: &str| -> String {
            if tz.is_empty() {
                "Your time zone".into()
            } else {
                c.zones.iter().find(|z| z.tz == tz).map(|z| z.display_name()).unwrap_or_else(|| city_name(tz))
            }
        };
        let current_region = region_label(&c.calendar.default_region);
        let zone_choices: Vec<(String, String)> =
            c.zones.iter().map(|z| (z.tz.clone(), format!("{} ({})", z.display_name(), z.city()))).collect();
        egui::ComboBox::from_label("Calendar opens on")
            .selected_text(current_region)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut c.calendar.default_region, String::new(), "Your time zone");
                for (tz, name) in zone_choices {
                    ui.selectable_value(&mut c.calendar.default_region, tz, name);
                }
            });
        ui.checkbox(&mut c.calendar.show_week_numbers, "Show week numbers");
        ui.checkbox(&mut c.calendar.highlight_weekends, "Dim weekends");
        ui.checkbox(&mut c.calendar.show_adjacent_months, "Show days from the previous and next month");
        ui.label(hint("In the panel, click a region's clock (or its name above the calendar) to see that region's dates."));

        ui.add_space(10.0);
        ui.heading("Events");
        ui.checkbox(&mut c.calendar.show_events, "Show events from the Calendar app");
        if !c.calendar.show_events {
            return;
        }
        ui.label("Show each event's time in:");
        ui.radio_value(&mut c.calendar.event_regions, EventRegions::All, "Every region");
        ui.radio_value(&mut c.calendar.event_regions, EventRegions::MenuBar, "Regions shown in the menu bar");
        ui.radio_value(&mut c.calendar.event_regions, EventRegions::Selected, "Only the region the calendar is showing");
        ui.add(egui::Slider::new(&mut c.calendar.upcoming_days, 0..=14).text("Upcoming days listed under the selected day"));

        ui.add_space(6.0);
        match self.source.access() {
            Access::Granted => {
                let cals = self.calendars.get_or_insert_with(|| self.source.calendars());
                if cals.is_empty() {
                    ui.label(weak("No calendars found."));
                } else {
                    ui.label("Calendars to show:");
                }
                for cal in cals.iter() {
                    let mut shown = !c.calendar.hidden_calendars.contains(&cal.id);
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 5.0, Color32::from_rgb(cal.color[0], cal.color[1], cal.color[2]));
                        if ui.checkbox(&mut shown, cal.title.as_str()).changed() {
                            if shown {
                                c.calendar.hidden_calendars.retain(|id| *id != cal.id);
                            } else {
                                c.calendar.hidden_calendars.push(cal.id.clone());
                            }
                        }
                    });
                }
                if ui.small_button("Refresh list").clicked() {
                    self.calendars = None;
                }
            }
            Access::NotDetermined => {
                ui.label(weak("TimeStrip needs permission to read your calendars."));
                if ui.button("Allow access to calendars…").clicked() {
                    self.source.request_access();
                }
            }
            Access::Denied => {
                ui.label(weak(
                    "Calendar access is turned off. Turn it on in System Settings → Privacy & Security → Calendars.",
                ));
                if cfg!(target_os = "macos") && ui.button("Open Privacy Settings").clicked() {
                    let _ = std::process::Command::new("open")
                        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars")
                        .spawn();
                }
            }
            Access::Unavailable(why) => {
                ui.label(hint(why));
            }
        }
    }

    fn general_tab(&mut self, ui: &mut egui::Ui) {
        if login::supported() {
            let mut on = login::is_enabled();
            if ui.checkbox(&mut on, "Open TimeStrip at login").changed() {
                if let Err(e) = login::set_enabled(on) {
                    self.save_error = Some(format!("Couldn't change login setting: {e}"));
                }
            }
            ui.add_space(8.0);
        }
        ui.label("Settings are saved automatically to:");
        ui.label(RichText::new(config::config_path().display().to_string()).monospace().small());
        if ui.button("Show in Finder").clicked() {
            crate::tray::reveal(&config::config_path());
        }
        ui.add_space(8.0);
        ui.label(weak("Tip: hold ⌘ and drag the clock in the menu bar to move it."));
    }
}

fn font_combo(ui: &mut egui::Ui, label: &str, id: impl std::hash::Hash, value: &mut String, families: &[String], filter: &str) {
    let fq = filter.trim().to_lowercase();
    let current = if value.is_empty() { "System font".to_string() } else { value.clone() };
    egui::ComboBox::new(id, label).selected_text(current).height(320.0).show_ui(ui, |ui| {
        ui.selectable_value(value, String::new(), "System font");
        for f in families.iter().filter(|f| fq.is_empty() || f.to_lowercase().contains(&fq)) {
            ui.selectable_value(value, f.clone(), f.as_str());
        }
    });
}

fn date_format_ui(ui: &mut egui::Ui, id: impl std::hash::Hash, format: &mut String) {
    let now = Utc::now();
    let local = clock::local_tz();
    egui::ComboBox::new(id, "Date format")
        .selected_text(clock::date_string(now, local, format))
        .show_ui(ui, |ui| {
            for (pattern, name) in DATE_PRESETS {
                let example = clock::date_string(now, local, pattern);
                ui.selectable_value(format, pattern.to_string(), format!("{example}   ({name})"));
            }
        });
    ui.horizontal(|ui| {
        ui.label("or custom:");
        ui.add(egui::TextEdit::singleline(format).desired_width(120.0));
    });
    ui.label(hint("%a weekday, %b month, %-d day, %-m month number, %Y year."));
}

/// A "use default / override" control for an optional setting.
fn override_toggle<T: Clone>(ui: &mut egui::Ui, label: &str, value: &mut Option<T>, default: T) -> bool {
    let mut on = value.is_some();
    if ui.checkbox(&mut on, label).changed() {
        *value = if on { Some(default) } else { None };
    }
    value.is_some()
}

fn zone_style_ui(ui: &mut egui::Ui, s: &mut ZoneStyle, g: &Config, families: &[String], i: usize) {
    ui.label(hint("Tick a setting to override it for this region only."));

    ui.horizontal(|ui| {
        if override_toggle(ui, "Text size", &mut s.font_size, g.font_size) {
            if let Some(v) = s.font_size.as_mut() {
                ui.add(egui::Slider::new(v, 8.0..=40.0).step_by(0.5).suffix(" pt"));
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, "Font", &mut s.font_family, g.font_family.clone()) {
            if let Some(v) = s.font_family.as_mut() {
                font_combo(ui, "", ("zone-font", i), v, families, "");
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, "Weight", &mut s.font_weight, g.font_weight) {
            if let Some(v) = s.font_weight.as_mut() {
                let name = weight_name(*v);
                ui.add(egui::Slider::new(v, 100..=900).step_by(100.0).text(name));
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, "Color", &mut s.color, g.color) {
            if let Some(v) = s.color.as_mut() {
                ui.color_edit_button_srgb(v);
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, "Label", &mut s.show_label, g.show_labels) {
            if let Some(v) = s.show_label.as_mut() {
                ui.checkbox(v, "show");
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, "24-hour", &mut s.use_24h, g.use_24h) {
            if let Some(v) = s.use_24h.as_mut() {
                ui.checkbox(v, "on");
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, "AM/PM", &mut s.show_ampm, g.show_ampm) {
            if let Some(v) = s.show_ampm.as_mut() {
                ui.checkbox(v, "show");
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, "Seconds", &mut s.show_seconds, g.show_seconds) {
            if let Some(v) = s.show_seconds.as_mut() {
                ui.checkbox(v, "show");
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, "Time format", &mut s.custom_time_format, clock::time_pattern(g)) {
            if let Some(v) = s.custom_time_format.as_mut() {
                ui.add(egui::TextEdit::singleline(v).desired_width(110.0));
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, "Date", &mut s.date_mode, g.date_mode) {
            if let Some(v) = s.date_mode.as_mut() {
                ui.radio_value(v, DateMode::Never, "never");
                ui.radio_value(v, DateMode::WhenDifferent, "when different");
                ui.radio_value(v, DateMode::Always, "always");
            }
        }
    });
    if override_toggle(ui, "Date format", &mut s.date_format, g.date_format.clone()) {
        if let Some(v) = s.date_format.as_mut() {
            ui.indent(("zone-date-format", i), |ui| date_format_ui(ui, ("zone-date", i), v));
        }
    }
    if !s.is_empty() && ui.small_button("Clear all overrides").clicked() {
        *s = ZoneStyle::default();
    }
}
