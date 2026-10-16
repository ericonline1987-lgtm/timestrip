//! The settings window. Every change is saved immediately, and the menu bar clock
//! picks it up within a second, so you can watch the real menu bar while you tweak.

use crate::clock;
use crate::config::{self, city_name, Config, DateMode, Layout, Zone};
use crate::login;
use crate::render::{Renderer, SCALE};
use chrono::Utc;
use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Clocks,
    Appearance,
    TimeDate,
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
}

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TimeStrip Settings")
            .with_inner_size([640.0, 660.0])
            .with_min_inner_size([520.0, 420.0]),
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
    };
    eframe::run_ui_native("TimeStrip Settings", options, move |ui, _frame| app.ui(ui))
}

const WEIGHT_NAMES: [&str; 9] =
    ["Thin", "Extra Light", "Light", "Regular", "Medium", "Semibold", "Bold", "Extra Bold", "Black"];

const DATE_PRESETS: [(&str, &str); 6] = [
    ("%a", "Weekday"),
    ("%a %-d", "Weekday + day"),
    ("%b %-d", "Month + day"),
    ("%a %b %-d", "Weekday, month, day"),
    ("%-m/%-d", "Numeric (US)"),
    ("%-d/%-m", "Numeric (intl)"),
];

impl SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.ctx().request_repaint_after(Duration::from_millis(500));

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Clocks, "🌐 Clocks");
                ui.selectable_value(&mut self.tab, Tab::Appearance, "🔠 Appearance");
                ui.selectable_value(&mut self.tab, Tab::TimeDate, "📅 Time & Date");
                ui.selectable_value(&mut self.tab, Tab::General, "⚙ General");
            });
            ui.separator();
            self.preview_ui(ui);
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
                Tab::Clocks => self.clocks_tab(ui),
                Tab::Appearance => self.appearance_tab(ui),
                Tab::TimeDate => self.time_tab(ui),
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
            self.preview = self.renderer.render(&segments, &self.config).map(|img| {
                let size = [img.width as usize, img.height as usize];
                let dark_text = ColorImage::from_rgba_unmultiplied(size, &img.rgba);
                // Template images turn white on a dark menu bar; custom colors stay as they are.
                let mut light = img.rgba.clone();
                if !self.config.use_custom_color {
                    for px in light.chunks_mut(4) {
                        px[0] = 255;
                        px[1] = 255;
                        px[2] = 255;
                    }
                }
                let light_text = ColorImage::from_rgba_unmultiplied(size, &light);
                let ctx = ui.ctx();
                (
                    key,
                    ctx.load_texture("preview-light-bar", dark_text, TextureOptions::LINEAR),
                    ctx.load_texture("preview-dark-bar", light_text, TextureOptions::LINEAR),
                )
            });
        }

        ui.label(egui::RichText::new("Preview").small().weak());
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
                ui.label(
                    egui::RichText::new(
                        "The menu bar is about 22 pt tall; anything taller is scaled down to fit.",
                    )
                    .small()
                    .weak(),
                );
            }
            None => {
                ui.label("No clocks are shown in the menu bar. Tick “Menu bar” on the Clocks tab.");
            }
        }
        if let Some(err) = &self.save_error {
            ui.colored_label(Color32::from_rgb(220, 60, 60), err);
        }
    }

    fn clocks_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(
                "The label is what appears in the menu bar; leave it empty to use the city name.",
            )
            .weak(),
        );
        ui.add_space(6.0);

        enum Action {
            Up(usize),
            Down(usize),
            Remove(usize),
        }
        let mut action = None;
        let count = self.config.zones.len();

        egui::Grid::new("zones").num_columns(5).striped(true).spacing([10.0, 6.0]).show(ui, |ui| {
            for (i, z) in self.config.zones.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    if ui.add_enabled(i > 0, egui::Button::new("▲").small()).clicked() {
                        action = Some(Action::Up(i));
                    }
                    if ui.add_enabled(i + 1 < count, egui::Button::new("▼").small()).clicked() {
                        action = Some(Action::Down(i));
                    }
                });
                ui.add(egui::TextEdit::singleline(&mut z.label).hint_text(z.city()).desired_width(90.0));
                ui.vertical(|ui| {
                    ui.label(z.city());
                    ui.label(egui::RichText::new(&z.tz).small().weak());
                });
                ui.checkbox(&mut z.show_in_bar, "Menu bar");
                if ui.button("Remove").clicked() {
                    action = Some(Action::Remove(i));
                }
                ui.end_row();
            }
        });

        match action {
            Some(Action::Up(i)) => self.config.zones.swap(i, i - 1),
            Some(Action::Down(i)) => self.config.zones.swap(i, i + 1),
            Some(Action::Remove(i)) => {
                self.config.zones.remove(i);
            }
            None => {}
        }

        ui.add_space(12.0);
        ui.heading("Add a clock");
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
                ui.label(egui::RichText::new("No matching time zones.").weak());
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

        ui.heading("Size");
        ui.add(egui::Slider::new(&mut c.font_size, 8.0..=40.0).step_by(0.5).text("Text size (pt)"));
        if c.layout == Layout::Stacked {
            ui.add(egui::Slider::new(&mut c.label_scale, 0.3..=1.2).step_by(0.05).text("Label size (× text size)"));
        }
        ui.add(egui::Slider::new(&mut c.spacing, 0.0..=60.0).step_by(1.0).text("Space between clocks (pt)"));
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
        let current = if c.font_family.is_empty() { "System font".to_string() } else { c.font_family.clone() };
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.add(egui::TextEdit::singleline(&mut self.font_query).hint_text("e.g. mono, menlo, avenir").desired_width(160.0));
        });
        let fq = self.font_query.trim().to_lowercase();
        let families = &self.families;
        egui::ComboBox::from_label("Family")
            .selected_text(current)
            .height(320.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut c.font_family, String::new(), "System font");
                for f in families.iter().filter(|f| fq.is_empty() || f.to_lowercase().contains(&fq)) {
                    ui.selectable_value(&mut c.font_family, f.clone(), f.as_str());
                }
            });
        let weight_name = WEIGHT_NAMES[((c.font_weight / 100).clamp(1, 9) - 1) as usize];
        ui.add(egui::Slider::new(&mut c.font_weight, 100..=900).step_by(100.0).text(format!("Weight ({weight_name})")));
        ui.checkbox(&mut c.tabular_digits, "Fixed-width digits (stops the clock from jiggling)");

        ui.add_space(10.0);
        ui.heading("Style");
        ui.checkbox(&mut c.show_labels, "Show labels");
        ui.horizontal(|ui| {
            ui.checkbox(&mut c.dim_labels, "Dim labels and dates");
            ui.add_enabled(c.dim_labels, egui::Slider::new(&mut c.dim_amount, 0.15..=1.0).text("opacity"));
        });
        ui.horizontal(|ui| {
            ui.label("Separator between clocks:");
            ui.add(egui::TextEdit::singleline(&mut c.separator).hint_text("none — try · | /").desired_width(70.0));
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut c.use_custom_color, "Custom color");
            if c.use_custom_color {
                ui.color_edit_button_srgb(&mut c.color);
            } else {
                ui.label(egui::RichText::new("(follows light/dark menu bar)").weak());
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
            ui.label(egui::RichText::new(format!("→ {}", clock::time_string(now, local, c))).weak());
        });
        ui.label(
            egui::RichText::new("Leave empty to use the options above. %H hour (24h), %-I hour (12h), %M minute, %S second, %p AM/PM.")
                .small()
                .weak(),
        );

        ui.add_space(10.0);
        ui.heading("Date in the menu bar");
        ui.radio_value(&mut c.date_mode, DateMode::Never, "Never");
        ui.radio_value(&mut c.date_mode, DateMode::WhenDifferent, "Only when that clock is on a different day than you");
        ui.radio_value(&mut c.date_mode, DateMode::Always, "Always");
        ui.add_enabled_ui(c.date_mode != DateMode::Never, |ui| {
            egui::ComboBox::from_label("Date format")
                .selected_text(clock::date_string(now, local, &c.date_format))
                .show_ui(ui, |ui| {
                    for (pattern, name) in DATE_PRESETS {
                        let example = clock::date_string(now, local, pattern);
                        ui.selectable_value(&mut c.date_format, pattern.to_string(), format!("{example}   ({name})"));
                    }
                });
            ui.horizontal(|ui| {
                ui.label("or custom:");
                ui.add(egui::TextEdit::singleline(&mut c.date_format).desired_width(120.0));
            });
            ui.label(egui::RichText::new("%a weekday, %b month, %-d day, %-m month number, %Y year.").small().weak());
        });

        ui.add_space(10.0);
        ui.heading("Dropdown menu");
        ui.checkbox(&mut c.menu_show_dates, "Show dates");
        ui.checkbox(&mut c.menu_show_offsets, "Show difference from your time");
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
        ui.label(egui::RichText::new(config::config_path().display().to_string()).monospace().small());
        if ui.button("Show in Finder").clicked() {
            crate::tray::reveal(&config::config_path());
        }
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Tip: hold ⌘ and drag the clock in the menu bar to move it.").weak(),
        );
    }
}
