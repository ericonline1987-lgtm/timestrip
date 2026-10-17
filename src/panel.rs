//! The panel that opens when you click the menu bar clock: every region's time, a slider to
//! plan across time zones, a month calendar you can switch between regions, and your
//! calendar events shown in each region's time.

use crate::calendar;
use crate::clock;
use crate::config::{self, Config, EventRegions};
use crate::events::{self, Access, Event, EventSource};
use crate::i18n::{t, tf};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use eframe::egui::{self, Align, Align2, Color32, FontId, Margin, RichText, Sense, Stroke, Vec2};
use std::time::{Instant, SystemTime};

/// `None` = your own time zone, `Some(i)` = `config.zones[i]`.
type Region = Option<usize>;

struct PanelApp {
    config: Config,
    config_mtime: Option<SystemTime>,
    source: EventSource,
    region: Region,
    view: (i32, u32),
    selected: NaiveDate,
    slider_minutes: f32,
    events: Vec<Event>,
    events_key: String,
    events_fetched: Option<Instant>,
    was_focused: bool,
    last_height: f32,
    zoom_applied: f32,
}

pub fn run(x: f32, y: f32) -> eframe::Result {
    let config = config::load();
    let zoom = config.panel.zoom;
    let width = config.panel.width * zoom;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TimeStrip")
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_active(true)
            .with_position([(x - width / 2.0).max(8.0), y])
            .with_inner_size([width, 560.0]),
        ..Default::default()
    };
    let mut app = PanelApp::new(config);
    eframe::run_ui_native("TimeStrip", options, move |ui, _frame| app.ui(ui))
}

impl PanelApp {
    fn new(config: Config) -> Self {
        let region = config
            .zones
            .iter()
            .position(|z| !config.calendar.default_region.is_empty() && z.tz == config.calendar.default_region);
        let mut app = PanelApp {
            config_mtime: config::modified_time(),
            config,
            source: EventSource::new(),
            region,
            view: (2000, 1),
            selected: NaiveDate::default(),
            slider_minutes: 0.0,
            events: Vec::new(),
            events_key: String::new(),
            events_fetched: None,
            was_focused: false,
            last_height: 0.0,
            zoom_applied: 0.0,
        };
        app.go_to_today();
        app
    }

    fn local(&self) -> Tz {
        clock::local_tz()
    }

    fn region_tz(&self, r: Region) -> Tz {
        match r.and_then(|i| self.config.zones.get(i)) {
            Some(z) => clock::parse_tz(&z.tz),
            None => self.local(),
        }
    }

    fn region_name(&self, r: Region) -> String {
        match r.and_then(|i| self.config.zones.get(i)) {
            Some(z) => z.display_name(),
            None => t("Local").into(),
        }
    }

    /// Time formatting settings for a region (its own overrides, if any).
    fn region_config(&self, r: Region) -> Config {
        match r.and_then(|i| self.config.zones.get(i)) {
            Some(z) => self.config.for_zone(z),
            None => self.config.clone(),
        }
    }

    fn today(&self, r: Region) -> NaiveDate {
        Utc::now().with_timezone(&self.region_tz(r)).date_naive()
    }

    fn go_to_today(&mut self) {
        let today = self.today(self.region);
        self.view = (today.year(), today.month());
        self.selected = today;
    }

    fn set_region(&mut self, r: Region) {
        // Keep looking at the same moment, re-expressed as a date in the new region.
        let was_today = self.selected == self.today(self.region);
        self.region = r;
        if was_today {
            self.go_to_today();
        }
    }

    fn reload_config_if_changed(&mut self) {
        let m = config::modified_time();
        if m != self.config_mtime {
            self.config_mtime = m;
            self.apply_config(config::load());
        }
    }

    fn apply_config(&mut self, config: Config) {
        let selected_zone = self.region.and_then(|i| self.config.zones.get(i)).map(|z| z.tz.clone());
        let was_today = self.selected == self.today(self.region);
        self.config = config;
        self.region = selected_zone.and_then(|tz| self.config.zones.iter().position(|z| z.tz == tz));
        if was_today {
            self.go_to_today();
        }
        self.events_fetched = None;
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
        self.reload_config_if_changed();

        let zoom = self.config.panel.zoom;
        if (zoom - self.zoom_applied).abs() > f32::EPSILON {
            ctx.set_zoom_factor(zoom);
            self.zoom_applied = zoom;
        }

        // Close when you click elsewhere or press Esc, like a menu.
        let focused = ui.input(|i| i.viewport().focused);
        if focused == Some(true) {
            self.was_focused = true;
        }
        if (self.was_focused && focused == Some(false)) || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.source.take_answer() {
            self.events_fetched = None;
        }

        let frame = egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::same(12));
        let content_height = egui::CentralPanel::default()
            .frame(frame)
            .show(ui, |ui| ui.vertical(|ui| self.contents(ui)).response.rect.height())
            .inner;

        // Grow or shrink the window to fit the content.
        let wanted = ((content_height + 24.0) * zoom).min(1000.0).ceil();
        if (wanted - self.last_height).abs() > 1.0 {
            self.last_height = wanted;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
                self.config.panel.width * zoom,
                wanted,
            )));
        }
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        ui.set_width(self.config.panel.width - 24.0);
        let now = Utc::now() + Duration::minutes(self.slider_minutes as i64);

        if self.config.panel.show_clocks {
            self.clocks(ui, now);
        }
        if self.config.panel.show_time_slider {
            self.slider(ui);
        }
        if self.config.panel.show_calendar {
            ui.add_space(6.0);
            ui.separator();
            self.calendar(ui);
            ui.separator();
            self.events_section(ui);
        }
        ui.add_space(6.0);
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(t("Settings…")).clicked() {
                crate::spawn_self(&["--settings"]);
            }
            if cfg!(target_os = "macos") && ui.button(t("Open Calendar")).clicked() {
                let _ = std::process::Command::new("open").args(["-a", "Calendar"]).spawn();
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui.button(t("Close")).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }

    fn clocks(&mut self, ui: &mut egui::Ui, now: DateTime<Utc>) {
        let local = self.local();
        let mut rows: Vec<Region> = vec![None];
        rows.extend((0..self.config.zones.len()).map(Some));
        let mut clicked = None;
        for r in rows {
            let tz = self.region_tz(r);
            let rc = self.region_config(r);
            let city = match r.and_then(|i| self.config.zones.get(i)) {
                Some(z) => z.city(),
                None => clock::city_name_of(tz),
            };
            let selected = r == self.region;
            let fill = if selected {
                ui.visuals().selection.bg_fill.gamma_multiply(0.35)
            } else {
                Color32::TRANSPARENT
            };
            let resp = egui::Frame::new()
                .fill(fill)
                .corner_radius(6.0)
                .inner_margin(Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(self.region_name(r)).strong());
                            let detail = format!(
                                "{city} · {} · {}",
                                clock::date_string(now, tz, "%a %b %-d"),
                                clock::offset_string(now, tz, local)
                            );
                            ui.label(RichText::new(detail).small().weak());
                        });
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(clock::time_string(now, tz, &rc)).size(22.0));
                        });
                    });
                })
                .response;
            let id = egui::Id::new(("clock-row", r));
            if ui.interact(resp.rect, id, Sense::click()).on_hover_text(t("Show this region's calendar")).clicked() {
                clicked = Some(r);
            }
        }
        if let Some(r) = clicked {
            self.set_region(r);
        }
    }

    fn slider(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let m = self.slider_minutes.round() as i64;
            let text = if m == 0 {
                t("Drag to see other times").to_string()
            } else {
                let (h, mm) = (m.abs() / 60, m.abs() % 60);
                let amount = match (h, mm) {
                    (0, mm) => format!("{mm}m"),
                    (h, 0) => format!("{h}h"),
                    (h, mm) => format!("{h}h {mm}m"),
                };
                tf(if m > 0 { "{} from now" } else { "{} ago" }, &[&amount])
            };
            ui.label(RichText::new(text).small().weak());
            if m != 0 {
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button(t("Now")).clicked() {
                        self.slider_minutes = 0.0;
                    }
                });
            }
        });
        ui.spacing_mut().slider_width = ui.available_width();
        ui.add(
            egui::Slider::new(&mut self.slider_minutes, -1440.0..=1440.0)
                .step_by(15.0)
                .show_value(false),
        );
    }

    fn calendar(&mut self, ui: &mut egui::Ui) {
        // Which region's calendar.
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t("Calendar for")).small().weak());
            let mut choice = self.region;
            let options: Vec<Region> = self.config.calendar_regions().iter().map(|key| {
                self.config.zones.iter().position(|z| !key.is_empty() && z.tz == *key)
            }).collect();
            for r in options {
                if ui.selectable_label(choice == r, self.region_name(r)).clicked() {
                    choice = r;
                }
            }
            if choice != self.region {
                self.set_region(choice);
            }
        });
        ui.add_space(4.0);

        // Month header.
        let (y, m) = self.view;
        ui.horizontal(|ui| {
            if ui.button("◀").clicked() {
                self.view = calendar::shift_month(y, m, -1);
            }
            ui.label(RichText::new(calendar::month_title(y, m)).strong().size(15.0));
            if ui.button("▶").clicked() {
                self.view = calendar::shift_month(y, m, 1);
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui.button(t("Today")).clicked() {
                    self.go_to_today();
                }
            });
        });

        let cal = self.config.calendar.clone();
        let (y, m) = self.view;
        let grid = calendar::month_grid(y, m, cal.first_weekday);
        self.ensure_events(&grid);

        let tz = self.region_tz(self.region);
        // EventKit's floating all-day dates use the system zone, not the Local override.
        let local = clock::system_tz();
        let today = self.today(self.region);
        let cols = if cal.show_week_numbers { 8 } else { 7 };
        let cell_w = (ui.available_width() / cols as f32).floor();
        let cell = Vec2::new(cell_w, 30.0);
        let visuals = ui.visuals().clone();
        let accent = visuals.selection.bg_fill;
        let mut clicked: Option<NaiveDate> = None;

        // Weekday header.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if cal.show_week_numbers {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(cell_w, 18.0), Sense::hover());
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, t("Wk"), FontId::proportional(10.0), visuals.weak_text_color());
            }
            for label in calendar::weekday_labels(cal.first_weekday) {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(cell_w, 18.0), Sense::hover());
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(11.0), visuals.weak_text_color());
            }
        });

        // Days.
        for row in &grid {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if cal.show_week_numbers {
                    let (rect, _) = ui.allocate_exact_size(cell, Sense::hover());
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        calendar::week_number(row),
                        FontId::proportional(10.0),
                        visuals.weak_text_color(),
                    );
                }
                for &day in row {
                    let in_month = day.month() == m;
                    let sense = if in_month || cal.show_adjacent_months { Sense::click() } else { Sense::hover() };
                    let (rect, resp) = ui.allocate_exact_size(cell, sense);
                    if !in_month && !cal.show_adjacent_months {
                        continue;
                    }
                    let p = ui.painter();
                    let center = rect.center() - Vec2::new(0.0, 3.0);
                    let radius = 11.0;
                    let mut text_color = if !in_month {
                        visuals.weak_text_color().gamma_multiply(0.6)
                    } else if cal.highlight_weekends && calendar::is_weekend(day) {
                        visuals.weak_text_color()
                    } else {
                        visuals.text_color()
                    };
                    if day == today {
                        p.circle_filled(center, radius, accent);
                        text_color = Color32::WHITE;
                    } else if day == self.selected {
                        p.circle_stroke(center, radius, Stroke::new(1.5, accent));
                    } else if resp.hovered() {
                        p.circle_filled(center, radius, visuals.faint_bg_color);
                    }
                    if day == self.selected && day == today {
                        p.circle_stroke(center, radius + 2.0, Stroke::new(1.0, accent));
                    }
                    p.text(center, Align2::CENTER_CENTER, day.day(), FontId::proportional(13.0), text_color);

                    // Up to three dots, one per calendar color, for days with events.
                    let mut colors: Vec<[u8; 3]> = Vec::new();
                    let day_events: Vec<&Event> = self.events.iter().filter(|e| events::on_day(e, day, tz, local)).collect();
                    for e in &day_events {
                        if !colors.contains(&e.color) {
                            colors.push(e.color);
                        }
                        if colors.len() == 3 {
                            break;
                        }
                    }
                    let n = colors.len() as f32;
                    for (k, c) in colors.iter().enumerate() {
                        let x = rect.center().x + (k as f32 - (n - 1.0) / 2.0) * 5.0;
                        p.circle_filled(egui::pos2(x, rect.bottom() - 3.0), 1.8, Color32::from_rgb(c[0], c[1], c[2]));
                    }
                    if resp.clicked() {
                        clicked = Some(day);
                    }
                    if !day_events.is_empty() {
                        resp.on_hover_text(day_events.iter().map(|e| format!("{} · {}", e.title, e.calendar_title)).collect::<Vec<_>>().join("\n"));
                    }
                }
            });
        }
        if let Some(day) = clicked {
            self.selected = day;
            if day.month() != m {
                self.view = (day.year(), day.month());
            }
        }
    }

    /// Fetches events for the visible weeks (plus a margin for other time zones and the
    /// upcoming list), refreshing at most once a minute.
    fn ensure_events(&mut self, grid: &[[NaiveDate; 7]]) {
        if !self.config.calendar.show_events {
            self.events.clear();
            return;
        }
        let first = grid[0][0] - Duration::days(2);
        let last_grid = grid[grid.len() - 1][6] + Duration::days(2);
        let upcoming_end = Utc::now().date_naive() + Duration::days(self.config.calendar.upcoming_days as i64 + 2);
        let last = last_grid.max(upcoming_end);
        let first = first.min(Utc::now().date_naive() - Duration::days(1));
        let region_key = self.region.and_then(|i| self.config.zones.get(i)).map(|z| z.tz.as_str()).unwrap_or("");
        let key = format!("{first}{last}{region_key}{:?}", self.config.calendar);
        let fresh = self.events_fetched.is_some_and(|t| t.elapsed().as_secs() < 60);
        if key == self.events_key && fresh {
            return;
        }
        let start = Utc.from_utc_datetime(&first.and_hms_opt(0, 0, 0).unwrap());
        let end = Utc.from_utc_datetime(&last.and_hms_opt(0, 0, 0).unwrap());
        self.events = self.source.events(start, end, &[]);
        self.events.retain(|event| events::visible_in_region(&event.calendar_id, region_key, &self.config.calendar));
        self.events_key = key;
        self.events_fetched = Some(Instant::now());
    }

    fn events_section(&mut self, ui: &mut egui::Ui) {
        if !self.config.calendar.show_events {
            return;
        }
        match self.source.access() {
            Access::Granted => {}
            Access::NotDetermined => {
                ui.label(RichText::new(t("See your calendar events here, shown in every region's time.")).weak());
                if ui.button(t("Show my calendar events")).clicked() {
                    self.source.request_access();
                }
                return;
            }
            Access::Denied => {
                ui.label(RichText::new(t(
                    "TimeStrip isn't allowed to read your calendars. Turn it on in System Settings → Privacy & Security → Calendars.",
                )).weak());
                if cfg!(target_os = "macos") && ui.button(t("Open Privacy Settings")).clicked() {
                    let _ = std::process::Command::new("open")
                        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars")
                        .spawn();
                }
                return;
            }
            Access::Unavailable(why) => {
                ui.label(RichText::new(why).small().weak());
                return;
            }
        }

        let tz = self.region_tz(self.region);
        let local = clock::system_tz();
        let day = self.selected;
        let heading = format!("{} · {}", calendar::long_date(day), self.region_name(self.region));
        ui.label(RichText::new(heading).strong());
        let todays: Vec<Event> = self.events.iter().filter(|e| events::on_day(e, day, tz, local)).cloned().collect();
        if todays.is_empty() {
            ui.label(RichText::new(t("No events")).weak());
        }
        for e in &todays {
            self.event_row(ui, e);
        }

        // Upcoming days after the selected one.
        for k in 1..=self.config.calendar.upcoming_days {
            let d = day + Duration::days(k as i64);
            let list: Vec<Event> = self
                .events
                .iter()
                .filter(|e| events::on_day(e, d, tz, local) && !events::on_day(e, day, tz, local))
                .cloned()
                .collect();
            if list.is_empty() {
                continue;
            }
            ui.add_space(6.0);
            ui.label(RichText::new(calendar::long_date(d)).strong().small());
            for e in &list {
                self.event_row(ui, e);
            }
        }
    }

    fn event_row(&self, ui: &mut egui::Ui, e: &Event) {
        let primary = self.region;
        let others: Vec<Region> = match self.config.calendar.event_regions {
            EventRegions::Selected => Vec::new(),
            EventRegions::All => std::iter::once(None)
                .chain((0..self.config.zones.len()).map(Some))
                .filter(|r| *r != primary)
                .collect(),
            EventRegions::MenuBar => (0..self.config.zones.len())
                .filter(|&i| self.config.zones[i].show_in_bar)
                .map(Some)
                .filter(|r| *r != primary)
                .collect(),
        };
        let color = Color32::from_rgb(e.color[0], e.color[1], e.color[2]);

        ui.horizontal(|ui| {
            let (bar, _) = ui.allocate_exact_size(Vec2::new(3.0, 34.0), Sense::hover());
            ui.painter().rect_filled(bar, 1.5, color);
            ui.vertical(|ui| {
                ui.label(RichText::new(&e.title).strong());
                ui.label(RichText::new(&e.calendar_title).small().weak());
                let main = if e.all_day {
                    t("All day").to_string()
                } else {
                    self.range_in(e, primary)
                };
                ui.label(RichText::new(main).small());
                if !e.all_day && !others.is_empty() {
                    let line: Vec<String> = others
                        .iter()
                        .map(|&r| format!("{} {}", self.region_name(r), self.start_in(e, r, primary)))
                        .collect();
                    ui.label(RichText::new(line.join("  ·  ")).small().weak());
                }
                if let Some(loc) = &e.location {
                    ui.label(RichText::new(loc).small().weak());
                }
            });
        });
        ui.add_space(2.0);
    }

    /// "5:00 PM – 6:00 PM" in a region (with the weekday when it ends on another day).
    fn range_in(&self, e: &Event, r: Region) -> String {
        let tz = self.region_tz(r);
        let rc = self.region_config(r);
        let start = clock::time_string(e.start, tz, &rc);
        let mut end = clock::time_string(e.end, tz, &rc);
        if e.end.with_timezone(&tz).date_naive() != e.start.with_timezone(&tz).date_naive() {
            end = format!("{} {end}", clock::date_string(e.end, tz, "%a"));
        }
        format!("{start} – {end}")
    }

    /// Start time in another region, with its weekday when that's a different day.
    fn start_in(&self, e: &Event, r: Region, primary: Region) -> String {
        let tz = self.region_tz(r);
        let t = clock::time_string(e.start, tz, &self.region_config(r));
        let here = e.start.with_timezone(&self.region_tz(primary)).date_naive();
        if e.start.with_timezone(&tz).date_naive() != here {
            format!("{t} {}", clock::date_string(e.start, tz, "%a"))
        } else {
            t
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_calendar_survives_reordering_and_falls_back_on_removal() {
        let mut config = Config::default();
        config.calendar.default_region = "America/New_York".into();
        let mut app = PanelApp::new(config.clone());
        config.zones.swap(0, 2);
        app.apply_config(config.clone());
        assert_eq!(app.region, Some(2));
        assert_eq!(app.region_tz(app.region), chrono_tz::America::New_York);
        config.zones.pop();
        app.apply_config(config);
        assert_eq!(app.region, None);
    }

    #[test]
    fn expanded_clocks_render_every_region_even_when_all_are_unpinned() {
        let mut config = Config::default();
        for zone in &mut config.zones {
            zone.show_in_bar = false;
        }
        let mut app = PanelApp::new(config);
        let mut output = egui::Context::default().run_ui(egui::RawInput::default(), |ui| {
            app.clocks(ui, Utc.with_ymd_and_hms(2026, 10, 16, 0, 35, 0).unwrap());
        });
        // This headless render has no GPU backend to consume texture updates.
        output.textures_delta.clear();
        let text: Vec<String> = output.shapes.iter().filter_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape { Some(text.galley.text().to_string()) } else { None }
        }).collect();
        for label in ["Local", "NYC", "LON", "TYO"] {
            assert!(text.iter().any(|text| text == label), "missing expanded clock: {label}");
        }
    }
}
