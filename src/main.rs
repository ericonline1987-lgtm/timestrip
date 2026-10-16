//! TimeStrip — a customizable menu bar world clock.
//!
//! `timestrip`             runs the menu bar clock.
//! `timestrip --settings`  opens the settings window (the menu bar app launches this for you).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clock;
mod config;
mod login;
mod render;
mod settings;
mod tray;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--settings") => {
            if let Err(e) = settings::run() {
                eprintln!("TimeStrip settings: {e}");
                std::process::exit(1);
            }
        }
        Some("--version") => println!("timestrip {}", env!("CARGO_PKG_VERSION")),
        Some("--help") | Some("-h") => {
            println!("timestrip             run the menu bar clock");
            println!("timestrip --settings  open the settings window");
            println!("config file: {}", config::config_path().display());
        }
        _ => tray::run(),
    }
}
