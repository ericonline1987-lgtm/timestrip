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
        Box::new(|_cc| Ok(Box::new(WindowApp(draw)))),
    )
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
