//! The settings window. Every change is saved immediately, and the menu bar clock and panel
//! pick it up within a second, so you can watch the real menu bar while you tweak.

use crate::clock;
use crate::config::{self, city_name, CalendarConfig, ClickAction, Config, DateMode, EventRegions, Layout, Zone, ZoneStyle};
use crate::events::{self, Access, CalendarInfo, EventSource};
use crate::i18n::{self, t, tf};
use crate::login;
use crate::render::{Renderer, SCALE};
use chrono::{Datelike, Utc};
use eframe::egui::{self, Color32, ColorImage, RichText, TextureHandle, TextureOptions};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Clocks,
    Appearance,
    TimeDate,
    Calendar,
    Language,
    General,
}

impl Tab {
    fn title(self) -> &'static str {
        t(match self {
            Self::Clocks => "Regions",
            Self::Appearance => "Appearance",
            Self::TimeDate => "Time & Date",
            Self::Calendar => "Calendar & Panel",
            Self::Language => "Language",
            Self::General => "General",
        })
    }
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
    zone_to_add: Option<&'static str>,
    local_tz_query: String,
    font_query: String,
    preview: Option<(String, TextureHandle, TextureHandle)>,
    source: EventSource,
    calendars: Option<Vec<CalendarInfo>>,
    holiday_region: String,
    subscription_url: String,
    subscription_error: Option<String>,
}

pub fn run() -> eframe::Result {
    // Load first so the window title is in the chosen language.
    let config = config::load();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(t("TimeStrip Settings"))
            .with_inner_size([860.0, 720.0])
            .with_min_inner_size([720.0, 520.0]),
        ..Default::default()
    };
    let renderer = Renderer::new();
    let mut app = SettingsApp {
        saved: config.clone(),
        config,
        save_error: None,
        families: renderer.families(),
        renderer,
        all_zones: clock::all_zone_names(),
        tab: Tab::Clocks,
        zone_query: String::new(),
        zone_to_add: None,
        local_tz_query: String::new(),
        font_query: String::new(),
        preview: None,
        source: EventSource::new(),
        calendars: None,
        holiday_region: String::new(),
        subscription_url: String::new(),
        subscription_error: None,
    };
    crate::window::run("TimeStrip Settings", options, move |ui, _frame| app.ui(ui))
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
    t(WEIGHT_NAMES[((w / 100).clamp(1, 9) - 1) as usize])
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

        egui::Panel::left("settings-navigation")
            .exact_size(152.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(ui.visuals().faint_bg_color).inner_margin(12))
            .show(ui, |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("TimeStrip").strong().size(17.0));
                ui.add_space(18.0);
                for tab in [Tab::Clocks, Tab::Appearance, Tab::TimeDate, Tab::Calendar, Tab::Language, Tab::General] {
                    let button = egui::Button::new(tab.title())
                        .selected(self.tab == tab)
                        .min_size(egui::vec2(ui.available_width(), 34.0));
                    if ui.add(button).clicked() {
                        self.tab = tab;
                    }
                }
            });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(20)).show(ui, |ui| {
            ui.heading(self.tab.title());
            ui.add_space(4.0);
            self.preview_ui(ui);
            ui.add_space(4.0);
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
                Tab::Clocks => self.clocks_tab(ui),
                Tab::Appearance => self.appearance_tab(ui),
                Tab::TimeDate => self.time_tab(ui),
                Tab::Calendar => self.calendar_tab(ui),
                Tab::Language => self.language_tab(ui),
                Tab::General => self.general_tab(ui),
            });
        });

        if self.config != self.saved {
            if self.config.language != self.saved.language
                || self.config.date_language != self.saved.date_language
                || self.config.local_time_zone != self.saved.local_time_zone
            {
                self.config.apply_globals();
                self.preview = None;
                self.calendars = None;
            }
            match config::save(&self.config) {
                Ok(()) => {
                    self.saved = self.config.clone();
                    self.save_error = None;
                }
                Err(e) => self.save_error = Some(tf("Couldn't save settings: {}", &[&e.to_string()])),
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

        ui.label(hint(t("Preview")));
        match &self.preview {
            Some((_, on_light, on_dark)) => {
                ui.columns(2, |columns| {
                    for (column, (tex, bg)) in columns.iter_mut().zip([(on_light, Color32::from_gray(236)), (on_dark, Color32::from_gray(30))]) {
                        egui::Frame::new().fill(bg).corner_radius(8.0).inner_margin(10).show(column, |ui| {
                            ui.set_min_width(ui.available_width());
                            egui::ScrollArea::horizontal().id_salt(tex.id()).show(ui, |ui| {
                                ui.image(egui::load::SizedTexture::new(tex.id(), tex.size_vec2() / SCALE));
                            });
                        });
                    }
                });
            }
            None => {
                ui.label(t("No regions are shown in the menu bar. Tick “Menu bar” on the Regions tab."));
            }
        }
        if let Some(err) = &self.save_error {
            ui.colored_label(Color32::from_rgb(220, 60, 60), err);
        }
    }

    fn clocks_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(hint(t("Each region can have its own label, size, font, color and time format. Anything not set here uses the Appearance and Time & Date tabs.")));
        ui.add_space(6.0);

        local_clock_ui(ui, &mut self.config);
        ui.add_space(4.0);

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
            egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(10.0).inner_margin(12).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    if ui.add_enabled(i > 0, egui::Button::new("▲").small()).on_hover_text("Move region up").clicked() {
                        action = Some(Action::Up(i));
                    }
                    if ui.add_enabled(i + 1 < count, egui::Button::new("▼").small()).on_hover_text("Move region down").clicked() {
                        action = Some(Action::Down(i));
                    }
                    let city = z.city();
                    ui.add(egui::TextEdit::singleline(&mut z.label).hint_text(city).desired_width(90.0));
                    ui.vertical(|ui| {
                        ui.label(z.city());
                        ui.label(hint(z.tz.clone()));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(t("Remove")).clicked() {
                            action = Some(Action::Remove(i));
                        }
                    });
                });
                ui.checkbox(&mut z.show_in_bar, t("Menu bar"));
                let title = if z.style.is_empty() {
                    t("Style for this region (using defaults)").to_string()
                } else {
                    t("Style for this region (customized)").to_string()
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
        ui.heading(t("Add a region"));
        add_region_ui(ui, &mut self.config.zones, &self.all_zones, &mut self.zone_query, &mut self.zone_to_add);
    }

    fn appearance_tab(&mut self, ui: &mut egui::Ui) {
        let c = &mut self.config;
        ui.label(weak(t("Defaults for every region. Individual regions can override these on the Regions tab.")));
        ui.add_space(6.0);

        ui.heading(t("Size"));
        ui.add(egui::Slider::new(&mut c.font_size, 8.0..=40.0).step_by(0.5).text("Text size (pt)"));
        if c.layout == Layout::Stacked {
            ui.add(egui::Slider::new(&mut c.label_scale, 0.3..=1.2).step_by(0.05).text("Label size (× text size)"));
        }
        ui.add(egui::Slider::new(&mut c.spacing, 0.0..=60.0).step_by(1.0).text("Space between regions (pt)"));
        ui.add(egui::Slider::new(&mut c.letter_spacing, -2.0..=6.0).step_by(0.1).text("Letter spacing (pt)"));
        ui.add(egui::Slider::new(&mut c.vertical_offset, -8.0..=8.0).step_by(0.5).text("Vertical nudge (pt, + is up)"));

        ui.add_space(10.0);
        ui.heading(t("Layout"));
        ui.horizontal(|ui| {
            ui.radio_value(&mut c.layout, Layout::Inline, t("Single line"));
            ui.radio_value(&mut c.layout, Layout::Stacked, t("Stacked (label above time)"));
        });

        ui.add_space(10.0);
        ui.heading(t("Font"));
        ui.horizontal(|ui| {
            ui.label(t("Filter:"));
            ui.add(
                egui::TextEdit::singleline(&mut self.font_query)
                    .hint_text("e.g. mono, menlo, avenir")
                    .desired_width(160.0),
            );
        });
        font_combo(ui, t("Family"), "font-family", &mut c.font_family, &self.families, &self.font_query);
        let weight_label = format!("{} ({})", t("Weight"), weight_name(c.font_weight));
        ui.add(egui::Slider::new(&mut c.font_weight, 100..=900).step_by(100.0).text(weight_label));
        ui.checkbox(&mut c.tabular_digits, t("Fixed-width digits (stops the clock from jiggling)"));

        ui.add_space(10.0);
        ui.heading(t("Style"));
        ui.checkbox(&mut c.show_labels, t("Show labels"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut c.dim_labels, t("Dim labels and dates"));
            ui.add_enabled(c.dim_labels, egui::Slider::new(&mut c.dim_amount, 0.15..=1.0).text("opacity"));
        });
        ui.horizontal(|ui| {
            ui.label(t("Separator between regions:"));
            ui.add(egui::TextEdit::singleline(&mut c.separator).hint_text("none — try · | /").desired_width(70.0));
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut c.use_custom_color, t("Custom color"));
            if c.use_custom_color {
                ui.color_edit_button_srgb(&mut c.color);
            } else {
                ui.label(weak(t("(follows light/dark menu bar)")));
            }
        });

        ui.add_space(12.0);
        if ui.button(t("Reset appearance to defaults")).clicked() {
            c.reset_appearance();
        }
    }

    fn time_tab(&mut self, ui: &mut egui::Ui) {
        let c = &mut self.config;
        let now = Utc::now();
        let local = clock::local_tz();
        ui.label(weak(t("Defaults for every region. Individual regions can override these on the Regions tab.")));
        ui.add_space(6.0);

        menu_bar_preset_ui(ui, c);
        ui.add_space(10.0);
        ui.heading(t("Time"));
        let custom = !c.custom_time_format.trim().is_empty();
        ui.add_enabled_ui(!custom, |ui| {
            ui.checkbox(&mut c.use_24h, t("24-hour clock"));
            ui.add_enabled_ui(!c.use_24h, |ui| {
                ui.checkbox(&mut c.show_ampm, t("Show AM/PM"));
                ui.checkbox(&mut c.lowercase_ampm, t("Lowercase am/pm"));
            });
            ui.checkbox(&mut c.show_seconds, t("Show seconds"));
        });
        ui.horizontal(|ui| {
            ui.label(t("Custom format:"));
            ui.add(egui::TextEdit::singleline(&mut c.custom_time_format).hint_text("e.g. %H.%M").desired_width(120.0));
            ui.label(weak(format!("→ {}", clock::time_string(now, local, c))));
        });
        ui.label(hint(
            t("Leave empty to use the options above. %H hour (24h), %-I hour (12h), %M minute, %S second, %p AM/PM."),
        ));

        ui.add_space(10.0);
        ui.heading(t("Date in the menu bar"));
        ui.radio_value(&mut c.date_mode, DateMode::Never, t("Never"));
        ui.radio_value(&mut c.date_mode, DateMode::WhenDifferent, t("Only when that region is on a different day than you"));
        ui.radio_value(&mut c.date_mode, DateMode::Always, t("Always"));
        ui.add_enabled_ui(c.date_mode != DateMode::Never, |ui| {
            date_format_ui(ui, "date-format", &mut c.date_format);
            ui.add_enabled(c.layout == Layout::Inline, egui::Checkbox::new(&mut c.date_before_time, "Date before time"))
                .on_hover_text("Applies to the single-line menu bar layout.");
        });

        ui.add_space(10.0);
        ui.heading(t("Right-click menu"));
        ui.checkbox(&mut c.menu_show_dates, t("Show dates"));
        ui.checkbox(&mut c.menu_show_offsets, t("Show difference from your time"));
    }

    fn calendar_tab(&mut self, ui: &mut egui::Ui) {
        let region_choices: Vec<(String, String)> = self.config.calendar_regions().into_iter().map(|key| {
            let label = if key.is_empty() { t("Local").into() } else {
                self.config.zones.iter().find(|z| z.tz == key).unwrap().display_name()
            };
            (key, label)
        }).collect();
        let c = &mut self.config;

        ui.heading(t("Panel"));
        ui.label(weak(t("The panel opens when you click the clock in the menu bar.")));
        ui.horizontal(|ui| {
            ui.label(t("Clicking the clock opens:"));
            ui.radio_value(&mut c.panel.left_click, ClickAction::Panel, t("Clocks & calendar panel"));
            ui.radio_value(&mut c.panel.left_click, ClickAction::Menu, t("Menu"));
        });
        ui.label(hint("Right-click always opens the menu (when the panel is chosen above)."));
        ui.add(egui::Slider::new(&mut c.panel.zoom, 0.7..=2.0).step_by(0.05).text("Panel size (zoom)"));
        ui.add(egui::Slider::new(&mut c.panel.width, 280.0..=600.0).step_by(10.0).text("Panel width (pt)"));
        ui.checkbox(&mut c.panel.show_clocks, t("Show every region's time"));
        ui.checkbox(&mut c.panel.show_time_slider, t("Show the time slider (see other times across regions)"));
        ui.checkbox(&mut c.panel.show_calendar, t("Show the month calendar and events"));

        ui.add_space(10.0);
        ui.heading(t("Month view"));
        egui::ComboBox::from_label(t("Week starts on"))
            .selected_text(t(WEEKDAYS[c.calendar.first_weekday as usize % 7]))
            .show_ui(ui, |ui| {
                for (i, name) in WEEKDAYS.iter().enumerate() {
                    ui.selectable_value(&mut c.calendar.first_weekday, i as u8, t(name));
                }
            });
        let region_label = |tz: &str| -> String {
            if tz.is_empty() {
                t("Your time zone").into()
            } else {
                c.zones.iter().find(|z| z.tz == tz).map(|z| z.display_name()).unwrap_or_else(|| city_name(tz))
            }
        };
        let current_region = region_label(&c.calendar.default_region);
        let zone_choices: Vec<(String, String)> =
            c.zones.iter().map(|z| (z.tz.clone(), format!("{} ({})", z.display_name(), z.city()))).collect();
        egui::ComboBox::from_label(t("Calendar opens on"))
            .selected_text(current_region)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut c.calendar.default_region, String::new(), t("Your time zone"));
                for (tz, name) in zone_choices {
                    ui.selectable_value(&mut c.calendar.default_region, tz, name);
                }
            });
        ui.checkbox(&mut c.calendar.show_week_numbers, t("Show week numbers"));
        ui.checkbox(&mut c.calendar.highlight_weekends, t("Dim weekends"));
        ui.checkbox(&mut c.calendar.show_adjacent_months, t("Show days from the previous and next month"));
        ui.label(hint("In the panel, click a region's clock (or its name above the calendar) to see that region's dates."));

        calendar_order_ui(ui, &mut c.calendar, &region_choices);

        ui.add_space(10.0);
        ui.heading("Add a holiday source");
        ui.label(hint("Enter an ICS subscription URL, confirm in Calendar, then return and refresh the list below."));
        ui.add(egui::TextEdit::singleline(&mut self.subscription_url)
            .hint_text("https://example.com/holidays.ics or webcal://...")
            .desired_width(f32::INFINITY));
        if ui.add_enabled(!self.subscription_url.trim().is_empty(), egui::Button::new("Subscribe in Calendar…")).clicked() {
            self.subscription_error = events::subscribe(&self.subscription_url).err();
        }
        if let Some(error) = &self.subscription_error {
            ui.colored_label(Color32::from_rgb(220, 60, 60), error);
        }

        ui.add_space(10.0);
        ui.heading(t("Events"));
        ui.checkbox(&mut c.calendar.show_events, t("Show events from the Calendar app"));
        if !c.calendar.show_events {
            ui.label(hint("Enable calendar events to choose and display holiday sources."));
            return;
        }
        ui.label(t("Show each event's time in:"));
        ui.radio_value(&mut c.calendar.event_regions, EventRegions::All, t("Every region"));
        ui.radio_value(&mut c.calendar.event_regions, EventRegions::MenuBar, t("Regions shown in the menu bar"));
        ui.radio_value(&mut c.calendar.event_regions, EventRegions::Selected, t("Only the region the calendar is showing"));
        ui.add(egui::Slider::new(&mut c.calendar.upcoming_days, 0..=14).text("Upcoming days listed under the selected day"));

        ui.add_space(6.0);
        match self.source.access() {
            Access::Granted => {
                let cals = self.calendars.get_or_insert_with(|| self.source.calendars());
                if cals.is_empty() {
                    ui.label(weak(t("No calendars found.")));
                } else {
                    ui.label(t("Calendars to show:"));
                }
                for cal in cals.iter().filter(|cal| !c.calendar.holiday_calendars.values().any(|ids| ids.contains(&cal.id))) {
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
                holiday_sources_ui(ui, &mut c.calendar, cals, &region_choices, &mut self.holiday_region);
                if ui.small_button(t("Refresh list")).clicked() {
                    self.source = EventSource::new();
                    self.calendars = None;
                }
            }
            Access::NotDetermined => {
                ui.label(weak(t("TimeStrip needs permission to read your calendars.")));
                if ui.button(t("Allow access to calendars…")).clicked() {
                    self.source.request_access();
                }
            }
            Access::Denied => {
                ui.label(weak(
                    t("Calendar access is turned off. Turn it on in System Settings → Privacy & Security → Calendars."),
                ));
                if cfg!(target_os = "macos") && ui.button(t("Open Privacy Settings")).clicked() {
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

    fn language_tab(&mut self, ui: &mut egui::Ui) {
        let now = Utc::now();
        let c = &mut self.config;

        ui.heading(t("Language"));
        let lang_name = |code: &str, empty: &'static str| -> String {
            if code.is_empty() {
                t(empty).to_string()
            } else {
                i18n::LANGS.iter().find(|l| l.0 == code).map(|l| l.1.to_string()).unwrap_or_else(|| code.to_string())
            }
        };
        let current = lang_name(&c.language, "Same as system");
        egui::ComboBox::from_label(t("Interface language")).selected_text(current).show_ui(ui, |ui| {
            ui.selectable_value(&mut c.language, String::new(), t("Same as system"));
            for (code, native, _) in i18n::LANGS {
                ui.selectable_value(&mut c.language, code.to_string(), native);
            }
        });
        let current = lang_name(&c.date_language, "Same as interface");
        egui::ComboBox::from_label(t("Language of weekday and month names")).selected_text(current).show_ui(ui, |ui| {
            ui.selectable_value(&mut c.date_language, String::new(), t("Same as interface"));
            for (code, native, _) in i18n::LANGS {
                ui.selectable_value(&mut c.date_language, code.to_string(), native);
            }
        });
        let local = clock::local_tz();
        let example = format!(
            "{}  ·  {}  ·  {}",
            crate::calendar::long_date(now.with_timezone(&local).date_naive()),
            clock::time_string(now, local, c),
            crate::calendar::month_title(now.with_timezone(&local).year(), now.with_timezone(&local).month()),
        );
        ui.horizontal(|ui| {
            ui.label(t("Example:"));
            ui.label(RichText::new(example).strong());
        });

        ui.add_space(14.0);
        ui.heading(t("Your time zone"));
        ui.horizontal(|ui| {
            ui.label(t("Your time zone (detected):"));
            ui.label(RichText::new(clock::system_tz().name()).monospace());
        });
        let mut custom = !c.local_time_zone.is_empty();
        if ui.checkbox(&mut custom, t("Use a different time zone for “Local”")).changed() {
            c.local_time_zone = if custom { clock::system_tz().name().to_string() } else { String::new() };
            self.local_tz_query.clear();
        }
        if custom {
            local_time_zone_ui(ui, &mut c.local_time_zone, &self.all_zones, &mut self.local_tz_query);
        }
        ui.label(hint(t("Time zones are set per region on the Regions tab.")));
    }

    fn general_tab(&mut self, ui: &mut egui::Ui) {
        if login::supported() {
            let mut on = login::is_enabled();
            if ui.checkbox(&mut on, t("Open TimeStrip at login")).changed() {
                if let Err(e) = login::set_enabled(on) {
                    self.save_error = Some(tf("Couldn't change login setting: {}", &[&e.to_string()]));
                }
            }
            ui.add_space(8.0);
        }
        ui.label(t("Settings are saved automatically to:"));
        ui.label(RichText::new(config::config_path().display().to_string()).monospace().small());
        if ui.button(t("Show in Finder")).clicked() {
            crate::tray::reveal(&config::config_path());
        }
        ui.add_space(8.0);
        ui.label(weak(t("Tip: hold ⌘ and drag the clock in the menu bar to move it.")));
    }
}

fn local_clock_ui(ui: &mut egui::Ui, config: &mut Config) {
    egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(10.0).inner_margin(12).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(t("Local")).strong());
        ui.label(hint(clock::local_tz().name()));
        ui.checkbox(&mut config.show_local_in_bar, t("Menu bar"));
    });
}

fn local_time_zone_ui(ui: &mut egui::Ui, selected: &mut String, choices: &[&str], query: &mut String) {
    egui::ComboBox::from_id_salt("local-tz")
        .selected_text(selected.replace('_', " "))
        .height(360.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            ui.add(egui::TextEdit::singleline(query)
                .hint_text(t("Search time zones (optional)"))
                .desired_width(f32::INFINITY));
            let filter = query.trim().to_lowercase().replace('_', " ");
            let matches: Vec<_> = choices.iter().copied()
                .filter(|name| name.to_lowercase().replace('_', " ").contains(&filter))
                .collect();
            if matches.is_empty() {
                ui.label(weak(t("No matching time zones.")));
            }
            egui::ScrollArea::vertical().id_salt("local-time-zone-choices").max_height(300.0)
                .auto_shrink([false, false]).show(ui, |ui| {
                    for name in matches {
                        if ui.selectable_value(selected, name.to_string(), name.replace('_', " ")).clicked() {
                            query.clear();
                            ui.close();
                        }
                    }
                });
        });
}

fn menu_bar_preset_ui(ui: &mut egui::Ui, config: &mut Config) {
    ui.heading("Menu bar preset");
    ui.label(hint("Show the weekday, month, day and time without opening the panel. Example: NYC Fri Oct 16 21:44."));
    if ui.button("Apple-style date & time").clicked() {
        config.apply_date_time_preset();
    }
    ui.label(hint("Applies to shared defaults. Per-region overrides still apply; date names follow your date language."));
}

fn add_region_ui(ui: &mut egui::Ui, zones: &mut Vec<Zone>, choices: &[&'static str], query: &mut String, selected: &mut Option<&'static str>) {
    ui.label(hint(t("Choose a time zone from the list. Search only filters the available choices.")));
    egui::ComboBox::from_id_salt("add-region-zone")
        .selected_text(selected.map(|name| name.replace('_', " ")).unwrap_or_else(|| t("Choose a time zone…").into()))
        .width(330.0)
        .height(260.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            if ui.add(egui::TextEdit::singleline(query)
                .hint_text(t("Search time zones (optional)"))
                .desired_width(f32::INFINITY)).changed() {
                *selected = None;
            }
            let filter = query.trim().to_lowercase();
            let matches: Vec<_> = choices.iter().copied()
                .filter(|name| name.to_lowercase().replace('_', " ").contains(&filter))
                .collect();
            if matches.is_empty() {
                ui.label(weak(t("No matching time zones.")));
            }
            egui::ScrollArea::vertical().id_salt("time-zone-choices").max_height(210.0)
                .auto_shrink([false, false]).show(ui, |ui| {
                    for name in matches {
                        if ui.selectable_value(selected, Some(name), name.replace('_', " ")).clicked() {
                            ui.close();
                        }
                    }
                });
        });
    if ui.add_enabled(selected.is_some(), egui::Button::new(t("Add region"))).clicked() {
        if let Some(name) = selected.take() {
            let label: String = city_name(name).chars().filter(|c| c.is_alphabetic()).take(3).collect();
            zones.push(Zone::new(name, &label.to_uppercase()));
            query.clear();
        }
    }
}

fn calendar_order_ui(ui: &mut egui::Ui, config: &mut CalendarConfig, choices: &[(String, String)]) {
    ui.add_space(10.0);
    ui.heading("Calendar region order");
    ui.label(hint("Independent of the menu bar and expanded clocks. Local can also be moved."));
    let mut order: Vec<String> = choices.iter().map(|(key, _)| key.clone()).collect();
    let mut moved = None;
    for (i, (_, label)) in choices.iter().enumerate() {
        ui.horizontal(|ui| {
            if ui.add_enabled(i > 0, egui::Button::new("▲").small()).on_hover_text("Move calendar region up").clicked() {
                moved = Some((i, i - 1));
            }
            if ui.add_enabled(i + 1 < order.len(), egui::Button::new("▼").small()).on_hover_text("Move calendar region down").clicked() {
                moved = Some((i, i + 1));
            }
            ui.label(label);
        });
    }
    if let Some((from, to)) = moved {
        order.swap(from, to);
        config.region_order = order;
    }
    if ui.small_button("Follow clock order").clicked() {
        config.region_order.clear();
    }
}

fn holiday_sources_ui(ui: &mut egui::Ui, config: &mut CalendarConfig, calendars: &[CalendarInfo], choices: &[(String, String)], region: &mut String) {
    ui.add_space(10.0);
    ui.heading("Holiday sources by region");
    ui.label(hint("Selected sources appear only in their associated regions, with names and colored date markers. A source can be linked to several regions."));
    if !choices.iter().any(|(key, _)| key == region) {
        region.clear();
    }
    let selected_label = &choices.iter().find(|(key, _)| key == region).unwrap().1;
    egui::ComboBox::from_label("Region for holiday sources").selected_text(selected_label).show_ui(ui, |ui| {
        for (key, label) in choices {
            ui.selectable_value(region, key.clone(), label);
        }
    });
    let mut linked = config.holiday_calendars.get(region).cloned().unwrap_or_default();
    for cal in calendars {
        let mut selected = linked.contains(&cal.id);
        if ui.checkbox(&mut selected, &cal.title).changed() {
            if selected {
                linked.push(cal.id.clone());
                // Removing the last association must not expose the source in every region.
                if !config.hidden_calendars.contains(&cal.id) {
                    config.hidden_calendars.push(cal.id.clone());
                }
            } else {
                linked.retain(|id| *id != cal.id);
            }
        }
    }
    for id in linked.iter().filter(|id| !calendars.iter().any(|cal| cal.id == **id)) {
        ui.label(hint(format!("Linked source unavailable: {id}. Restore the calendar or clear the links.")));
    }
    if !linked.is_empty() && ui.small_button("Clear holiday links for this region").clicked() {
        linked.clear();
    }
    if linked.is_empty() {
        config.holiday_calendars.remove(region);
    } else {
        config.holiday_calendars.insert(region.clone(), linked);
    }
}

fn font_combo(ui: &mut egui::Ui, label: &str, id: impl std::hash::Hash + std::fmt::Debug, value: &mut String, families: &[String], filter: &str) {
    let fq = filter.trim().to_lowercase();
    let current = if value.is_empty() { t("System font").to_string() } else { value.clone() };
    egui::ComboBox::new(id, label).selected_text(current).height(320.0).show_ui(ui, |ui| {
        ui.selectable_value(value, String::new(), t("System font"));
        for f in families.iter().filter(|f| fq.is_empty() || f.to_lowercase().contains(&fq)) {
            ui.selectable_value(value, f.clone(), f.as_str());
        }
    });
}

fn date_format_ui(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, format: &mut String) {
    let now = Utc::now();
    let local = clock::local_tz();
    egui::ComboBox::new(id, t("Date format"))
        .selected_text(clock::date_string(now, local, format))
        .show_ui(ui, |ui| {
            for (pattern, name) in DATE_PRESETS {
                let example = clock::date_string(now, local, pattern);
                ui.selectable_value(format, pattern.to_string(), format!("{example}   ({})", t(name)));
            }
        });
    ui.horizontal(|ui| {
        ui.label(t("or custom:"));
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
        if override_toggle(ui, t("Text size"), &mut s.font_size, g.font_size) {
            if let Some(v) = s.font_size.as_mut() {
                ui.add(egui::Slider::new(v, 8.0..=40.0).step_by(0.5).suffix(" pt"));
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, t("Font"), &mut s.font_family, g.font_family.clone()) {
            if let Some(v) = s.font_family.as_mut() {
                font_combo(ui, "", ("zone-font", i), v, families, "");
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, t("Weight"), &mut s.font_weight, g.font_weight) {
            if let Some(v) = s.font_weight.as_mut() {
                let name = weight_name(*v);
                ui.add(egui::Slider::new(v, 100..=900).step_by(100.0).text(name));
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, t("Color"), &mut s.color, g.color) {
            if let Some(v) = s.color.as_mut() {
                ui.color_edit_button_srgb(v);
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, t("Label"), &mut s.show_label, g.show_labels) {
            if let Some(v) = s.show_label.as_mut() {
                ui.checkbox(v, t("show"));
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, t("24-hour"), &mut s.use_24h, g.use_24h) {
            if let Some(v) = s.use_24h.as_mut() {
                ui.checkbox(v, t("on"));
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, t("AM/PM"), &mut s.show_ampm, g.show_ampm) {
            if let Some(v) = s.show_ampm.as_mut() {
                ui.checkbox(v, t("show"));
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, t("Seconds"), &mut s.show_seconds, g.show_seconds) {
            if let Some(v) = s.show_seconds.as_mut() {
                ui.checkbox(v, t("show"));
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, t("Time format"), &mut s.custom_time_format, clock::time_pattern(g)) {
            if let Some(v) = s.custom_time_format.as_mut() {
                ui.add(egui::TextEdit::singleline(v).desired_width(110.0));
            }
        }
    });
    ui.horizontal(|ui| {
        if override_toggle(ui, t("Date"), &mut s.date_mode, g.date_mode) {
            if let Some(v) = s.date_mode.as_mut() {
                ui.radio_value(v, DateMode::Never, t("never"));
                ui.radio_value(v, DateMode::WhenDifferent, t("when different"));
                ui.radio_value(v, DateMode::Always, t("always"));
            }
        }
    });
    if override_toggle(ui, t("Date format"), &mut s.date_format, g.date_format.clone()) {
        if let Some(v) = s.date_format.as_mut() {
            ui.indent(("zone-date-format", i), |ui| date_format_ui(ui, ("zone-date", i), v));
        }
    }
    if !s.is_empty() && ui.small_button(t("Clear all overrides")).clicked() {
        *s = ZoneStyle::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, events: Vec<egui::Event>, show: impl FnMut(&mut egui::Ui)) -> Vec<(String, egui::Pos2)> {
        let mut output = ctx.run_ui(egui::RawInput { events, ..Default::default() }, show);
        // Headless UI tests intentionally consume no GPU textures.
        output.textures_delta.clear();
        output.shapes.iter().filter_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape {
                Some((text.galley.text().to_string(), text.pos + text.galley.size() / 2.0))
            } else { None }
        }).collect()
    }

    fn click(ctx: &egui::Context, pos: egui::Pos2, mut show: impl FnMut(&mut egui::Ui)) {
        for pressed in [true, false] {
            frame(ctx, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE,
            }], &mut show);
        }
    }

    #[test]
    fn local_clock_has_a_visibility_toggle_without_a_remove_action() {
        let ctx = egui::Context::default();
        let mut config = Config::default();
        config.local_time_zone = "Asia/Singapore".into();
        let original = config.clone();
        let text = frame(&ctx, vec![], |ui| local_clock_ui(ui, &mut config));
        assert!(text.iter().any(|(label, _)| label == "Local"));
        assert!(!text.iter().any(|(label, _)| label == "Remove"));
        let toggle = text.iter().find(|(label, _)| label == "Menu bar").unwrap().1;
        click(&ctx, toggle, |ui| local_clock_ui(ui, &mut config));
        assert!(!config.show_local_in_bar);
        config.show_local_in_bar = true;
        assert_eq!(config, original, "the toggle must preserve all other settings");
        config.zones.clear();
        let text = frame(&ctx, vec![], |ui| local_clock_ui(ui, &mut config));
        assert!(text.iter().any(|(label, _)| label == "Local"));
        let toggle = text.iter().find(|(label, _)| label == "Menu bar").unwrap().1;
        click(&ctx, toggle, |ui| local_clock_ui(ui, &mut config));
        assert!(!config.show_local_in_bar);
        click(&ctx, toggle, |ui| local_clock_ui(ui, &mut config));
        assert!(config.show_local_in_bar);
        assert!(config.zones.is_empty());
    }

    #[test]
    fn local_time_zone_search_filters_without_changing_the_selection() {
        let ctx = egui::Context::default();
        let choices = ["America/New_York", "Asia/Tokyo", "America/Los_Angeles"];
        let mut selected = "America/New_York".to_string();
        let mut query = String::new();
        let text = frame(&ctx, vec![], |ui| local_time_zone_ui(ui, &mut selected, &choices, &mut query));
        let picker = text.iter().find(|(label, _)| label == "America/New York").unwrap().1;
        click(&ctx, picker, |ui| local_time_zone_ui(ui, &mut selected, &choices, &mut query));
        let text = frame(&ctx, vec![], |ui| local_time_zone_ui(ui, &mut selected, &choices, &mut query));
        for name in choices {
            assert!(text.iter().any(|(label, _)| *label == name.replace('_', " ")));
        }
        let search = text.iter().find(|(label, _)| label == "Search time zones (optional)").unwrap().1;
        click(&ctx, search, |ui| local_time_zone_ui(ui, &mut selected, &choices, &mut query));
        let text = frame(&ctx, vec![egui::Event::Text("LoS aNgElEs".into())], |ui| {
            local_time_zone_ui(ui, &mut selected, &choices, &mut query);
        });
        assert_eq!(selected, "America/New_York", "typing must not change the configured time zone");
        assert!(text.iter().any(|(label, _)| label == "America/Los Angeles"));
        assert!(!text.iter().any(|(label, _)| label == "Asia/Tokyo"));
        let choice = text.iter().find(|(label, _)| label == "America/Los Angeles").unwrap().1;
        click(&ctx, choice, |ui| local_time_zone_ui(ui, &mut selected, &choices, &mut query));
        assert_eq!(selected, "America/Los_Angeles");
        assert!(query.is_empty(), "reopening the picker should show all choices");
    }

    #[test]
    fn local_time_zone_search_handles_iana_names_no_matches_and_clearing() {
        let ctx = egui::Context::default();
        let choices = ["America/New_York", "Asia/Tokyo", "America/Los_Angeles"];
        let mut selected = "Asia/Tokyo".to_string();
        let mut query = String::new();
        let text = frame(&ctx, vec![], |ui| local_time_zone_ui(ui, &mut selected, &choices, &mut query));
        let picker = text.iter().find(|(label, _)| label == "Asia/Tokyo").unwrap().1;
        click(&ctx, picker, |ui| local_time_zone_ui(ui, &mut selected, &choices, &mut query));
        query = "aMeRiCa/LoS_AnGeLeS".into();
        let text = frame(&ctx, vec![], |ui| local_time_zone_ui(ui, &mut selected, &choices, &mut query));
        assert!(text.iter().any(|(label, _)| label == "America/Los Angeles"));
        assert!(!text.iter().any(|(label, _)| label == "America/New York"));
        query = "no such time zone".into();
        let text = frame(&ctx, vec![], |ui| local_time_zone_ui(ui, &mut selected, &choices, &mut query));
        assert!(text.iter().any(|(label, _)| label == "No matching time zones."));
        assert_eq!(selected, "Asia/Tokyo");
        let search = text.iter().find(|(label, _)| label == "no such time zone").unwrap().1;
        click(&ctx, search, |ui| local_time_zone_ui(ui, &mut selected, &choices, &mut query));
        let backspace = egui::Event::Key {
            key: egui::Key::Backspace, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE,
        };
        let mut events = vec![egui::Event::Key {
            key: egui::Key::End, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE,
        }];
        events.extend(vec![backspace; query.len()]);
        let text = frame(&ctx, events, |ui| {
            local_time_zone_ui(ui, &mut selected, &choices, &mut query);
        });
        assert!(query.is_empty());
        for name in choices {
            assert!(text.iter().any(|(label, _)| *label == name.replace('_', " ")));
        }
        assert_eq!(selected, "Asia/Tokyo");
    }

    #[test]
    fn date_time_preset_shows_the_date_even_on_the_local_day() {
        use chrono::TimeZone;
        let ctx = egui::Context::default();
        let mut config = Config::default();
        config.show_local_in_bar = false;
        config.zones = vec![Zone::new("UTC", "UTC")];
        config.custom_time_format = "%H.%M".into();
        config.date_mode = DateMode::Never;
        config.calendar.region_order = vec![String::new()];
        let text = frame(&ctx, vec![], |ui| menu_bar_preset_ui(ui, &mut config));
        let preset = text.iter().find(|(label, _)| label == "Apple-style date & time").unwrap().1;
        click(&ctx, preset, |ui| menu_bar_preset_ui(ui, &mut config));
        let now = Utc.with_ymd_and_hms(2026, 10, 16, 21, 44, 0).unwrap();
        let shown = clock::segments(now, chrono_tz::Tz::UTC, &config);
        assert_eq!(shown[0].date.as_deref(), Some("Fri Oct 16"));
        assert_eq!(shown[0].time, "21:44");
        assert!(config.date_before_time);
        assert_eq!(config.layout, Layout::Inline);
        assert_eq!(config.calendar.region_order, vec![String::new()]);
        config.zones[0].style.date_mode = Some(DateMode::Never);
        assert!(clock::segments(now, chrono_tz::Tz::UTC, &config)[0].date.is_none());
    }

    #[test]
    fn region_picker_browses_without_typing_and_adds_only_after_selection() {
        let ctx = egui::Context::default();
        let mut zones = Vec::new();
        let choices = ["America/New_York", "Asia/Tokyo", "Europe/London"];
        let mut query = String::new();
        let mut selected = None;
        let text = frame(&ctx, vec![], |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        let add = text.iter().find(|(label, _)| label == "Add region").unwrap().1;
        click(&ctx, add, |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        assert!(zones.is_empty(), "adding requires a selected time zone");
        let text = frame(&ctx, vec![], |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        let picker = text.iter().find(|(label, _)| label == "Choose a time zone…").unwrap().1;
        click(&ctx, picker, |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        let text = frame(&ctx, vec![], |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        for name in choices {
            assert!(text.iter().any(|(label, _)| *label == name.replace('_', " ")), "missing choice: {name}");
        }
        let tokyo = text.iter().find(|(label, _)| label == "Asia/Tokyo").unwrap().1;
        click(&ctx, tokyo, |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        assert_eq!(selected, Some("Asia/Tokyo"));
        assert!(zones.is_empty(), "selecting a time zone must not add it immediately");
        let text = frame(&ctx, vec![], |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        let add = text.iter().find(|(label, _)| label == "Add region").unwrap().1;
        click(&ctx, add, |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].tz, "Asia/Tokyo");
        assert_eq!(zones[0].label, "TOK");
        assert_eq!(selected, None);
        assert!(query.is_empty());
    }

    #[test]
    fn region_search_only_filters_and_clearing_restores_choices() {
        let ctx = egui::Context::default();
        let mut zones = Vec::new();
        let choices = ["America/New_York", "Asia/Tokyo", "Europe/London"];
        let mut query = String::new();
        let mut selected = None;
        let text = frame(&ctx, vec![], |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        let picker = text.iter().find(|(label, _)| label == "Choose a time zone…").unwrap().1;
        click(&ctx, picker, |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        let text = frame(&ctx, vec![], |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        let search = text.iter().find(|(label, _)| label == "Search time zones (optional)").unwrap().1;
        click(&ctx, search, |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        let text = frame(&ctx, vec![egui::Event::Text("tOkYo".into())], |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        assert_eq!(query, "tOkYo");
        assert!(text.iter().any(|(label, _)| label == "Asia/Tokyo"));
        assert!(!text.iter().any(|(label, _)| label == "Europe/London"));
        assert!(selected.is_none());
        assert!(zones.is_empty());
        let backspace = egui::Event::Key {
            key: egui::Key::Backspace, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, vec![backspace; query.len()], |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        assert!(query.is_empty());
        let text = frame(&ctx, vec![], |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        assert!(text.iter().any(|(label, _)| label == "Europe/London"), "restored choices: {text:?}");
        query = "not a time zone".into();
        let text = frame(&ctx, vec![], |ui| add_region_ui(ui, &mut zones, &choices, &mut query, &mut selected));
        assert!(text.iter().any(|(label, _)| label == "No matching time zones."));
        assert!(selected.is_none());
        assert!(zones.is_empty());
    }

    #[test]
    fn calendar_order_controls_move_local_and_restore_clock_order() {
        let ctx = egui::Context::default();
        let mut config = CalendarConfig::default();
        let choices = vec![(String::new(), "Local".into()), ("Asia/Tokyo".into(), "TYO".into())];
        let text = frame(&ctx, vec![], |ui| calendar_order_ui(ui, &mut config, &choices));
        let down = text.iter().find(|(label, _)| label == "▼").unwrap().1;
        click(&ctx, down, |ui| calendar_order_ui(ui, &mut config, &choices));
        assert_eq!(config.region_order, vec!["Asia/Tokyo", ""]);
        let text = frame(&ctx, vec![], |ui| calendar_order_ui(ui, &mut config, &choices));
        let reset = text.iter().find(|(label, _)| label == "Follow clock order").unwrap().1;
        click(&ctx, reset, |ui| calendar_order_ui(ui, &mut config, &choices));
        assert!(config.region_order.is_empty());
    }

    #[test]
    fn holiday_controls_link_and_unlink_without_leaking_to_other_regions() {
        let ctx = egui::Context::default();
        let mut config = CalendarConfig::default();
        let calendars = vec![CalendarInfo { id: "cn".into(), title: "China Holidays".into(), color: [255, 0, 0] }];
        let choices = vec![(String::new(), "Local".into()), ("Asia/Shanghai".into(), "SHA".into())];
        let mut region = "Asia/Shanghai".to_string();
        let text = frame(&ctx, vec![], |ui| holiday_sources_ui(ui, &mut config, &calendars, &choices, &mut region));
        assert!(config.holiday_calendars.is_empty(), "viewing settings must not add links");
        let source = text.iter().find(|(label, _)| label == "China Holidays").unwrap().1;
        click(&ctx, source, |ui| holiday_sources_ui(ui, &mut config, &calendars, &choices, &mut region));
        assert!(events::visible_in_region("cn", "Asia/Shanghai", &config));
        assert!(!events::visible_in_region("cn", "", &config));
        let text = frame(&ctx, vec![], |ui| holiday_sources_ui(ui, &mut config, &calendars, &choices, &mut region));
        let source = text.iter().find(|(label, _)| label == "China Holidays").unwrap().1;
        click(&ctx, source, |ui| holiday_sources_ui(ui, &mut config, &calendars, &choices, &mut region));
        assert!(config.holiday_calendars.is_empty());
        assert!(!events::visible_in_region("cn", "Asia/Shanghai", &config));
        assert!(!events::visible_in_region("cn", "", &config));
    }
}
