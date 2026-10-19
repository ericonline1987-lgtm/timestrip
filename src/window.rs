//! Runs the panel and settings in their own single-window processes.

use eframe::{egui, App, Frame, NativeOptions};

pub fn run(
    title: &str,
    options: NativeOptions,
    draw: impl FnMut(&mut egui::Ui, &mut Frame) + 'static,
) -> eframe::Result {
    eframe::run_native(
        title,
        options,
        Box::new(|cc| {
            apply_style(&cc.egui_ctx);
            Ok(Box::new(WindowApp(draw)))
        }),
    )
}

fn apply_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 28.0;
        style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        style.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        style.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        let v = &mut style.visuals;
        let (background, surface, text, secondary, border) = if v.dark_mode {
            (28, 40, 235, 160, 58)
        } else {
            (248, 237, 35, 105, 218)
        };
        v.panel_fill = egui::Color32::from_gray(background);
        v.window_fill = v.panel_fill;
        v.faint_bg_color = egui::Color32::from_gray(surface);
        v.widgets.noninteractive.fg_stroke.color = egui::Color32::from_gray(text);
        v.weak_text_color = Some(egui::Color32::from_gray(secondary));
        v.selection.bg_fill = egui::Color32::from_rgb(0, 122, 255);
        v.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        v.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(border));
        v.widgets.noninteractive.bg_stroke = v.window_stroke;
        v.widgets.inactive.bg_fill = v.faint_bg_color;
        v.widgets.inactive.weak_bg_fill = v.faint_bg_color;
        for widget in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
            widget.fg_stroke.color = egui::Color32::from_gray(text);
            widget.corner_radius = egui::CornerRadius::same(7);
            widget.bg_stroke = egui::Stroke::NONE;
        }
    });
}

struct WindowApp<F>(F);

impl<F: FnMut(&mut egui::Ui, &mut Frame)> App for WindowApp<F> {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut Frame) {
        (self.0)(ui, frame);
    }

    #[cfg(target_os = "macos")]
    fn on_exit(&mut self) {
        // eframe has saved state and called egui's exit hooks at this point.
        // Exit this child before AccessKit restores the view's class during teardown:
        // that discards AppKit's Touch Bar observers and crashes on the next display cycle.
        // https://github.com/AccessKit/accesskit/issues/823
        std::process::exit(0);
    }
}
