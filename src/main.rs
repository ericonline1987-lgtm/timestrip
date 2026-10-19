//! TimeStrip — a customizable menu bar world clock and calendar.
//!
//! `timestrip`                  runs the menu bar clock.
//! `timestrip --panel <x> <y>`  opens the clock & calendar panel at a screen position.
//! `timestrip --settings`       opens the settings window.
//!
//! The menu bar app launches the panel and settings for you.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod calendar;
mod clock;
mod config;
mod events;
mod i18n;
mod login;
mod panel;
mod render;
mod settings;
mod tray;
mod window;

use std::process::{Child, Command};

/// Starts another copy of this program with the given arguments.
pub fn spawn_self(args: &[&str]) -> Option<Child> {
    let exe = std::env::current_exe().ok()?;
    Command::new(exe).args(args).spawn().ok()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).map(String::as_str);
    let result = match arg(0) {
        Some("--settings") => settings::run(),
        Some("--panel") => {
            let x = arg(1).and_then(|v| v.parse().ok()).unwrap_or(600.0);
            let y = arg(2).and_then(|v| v.parse().ok()).unwrap_or(30.0);
            panel::run(x, y)
        }
        Some("--version") => {
            println!("timestrip {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help") | Some("-h") => {
            println!("timestrip                  run the menu bar clock");
            println!("timestrip --panel <x> <y>  open the clock & calendar panel");
            println!("timestrip --settings       open the settings window");
            println!("config file: {}", config::config_path().display());
            Ok(())
        }
        _ => tray::run(),
    };
    if let Err(e) = result {
        eprintln!("TimeStrip: {e}");
        std::process::exit(1);
    }
}
