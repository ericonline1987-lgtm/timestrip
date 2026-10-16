//! The menu bar item: draws the clocks, keeps them ticking, opens the panel on click and
//! shows the dropdown menu on right-click.

use crate::clock;
use crate::config::{self, ClickAction, Config};
use crate::login;
use crate::render::Renderer;
use chrono::Utc;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopWindowTarget};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, Rect, TrayIcon, TrayIconBuilder, TrayIconEvent};

enum UserEvent {
    Menu(MenuEvent),
    Tray(TrayIconEvent),
}

struct MenuItems {
    zones: Vec<MenuItem>,
    open_panel: MenuItem,
    copy_all: MenuItem,
    settings: MenuItem,
    edit_file: MenuItem,
    login: Option<CheckMenuItem>,
    quit: MenuItem,
}

/// The clock & calendar panel runs as a child process; this tracks it so a click on the
/// clock can toggle it open and closed.
#[derive(Default, Clone)]
struct PanelProcess {
    child: Arc<Mutex<Option<Child>>>,
    closed_at: Arc<Mutex<Option<Instant>>>,
}

impl PanelProcess {
    fn is_open(&self) -> bool {
        let mut guard = self.child.lock().unwrap();
        match guard.as_mut() {
            Some(c) => matches!(c.try_wait(), Ok(None)),
            None => false,
        }
    }

    fn close(&self) {
        if let Some(mut c) = self.child.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        *self.closed_at.lock().unwrap() = Some(Instant::now());
    }

    /// Clicking the clock while the panel is open first makes the panel lose focus (so it
    /// closes itself) and then delivers the click; don't treat that click as "open again".
    fn just_closed(&self) -> bool {
        self.closed_at.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_millis(400))
    }

    fn open(&self, x: f64, y: f64) {
        let Some(child) = crate::spawn_self(&["--panel", &format!("{x:.0}"), &format!("{y:.0}")]) else {
            return;
        };
        *self.child.lock().unwrap() = Some(child);
        // Notice when the panel closes itself.
        let me = self.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(50));
            let mut guard = me.child.lock().unwrap();
            match guard.as_mut() {
                Some(c) => {
                    if !matches!(c.try_wait(), Ok(None)) {
                        *guard = None;
                        *me.closed_at.lock().unwrap() = Some(Instant::now());
                        break;
                    }
                }
                None => break,
            }
        });
    }
}

struct App {
    config: Config,
    config_mtime: Option<SystemTime>,
    renderer: Renderer,
    tray: TrayIcon,
    items: MenuItems,
    last_signature: String,
    settings_child: Option<Child>,
    panel: PanelProcess,
    dark_menu_bar: bool,
    appearance_checked: Option<Instant>,
    /// Points per physical pixel conversion and screen width (points), for placing the panel.
    scale: f64,
    screen_width: f64,
    last_rect: Option<Rect>,
}

pub fn run() -> ! {
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        // Menu bar only: no Dock icon, no app menu.
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }

    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |e| {
        let _ = proxy.send_event(UserEvent::Menu(e));
    }));
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |e| {
        let _ = proxy.send_event(UserEvent::Tray(e));
    }));

    let mut app: Option<App> = None;

    event_loop.run(move |event, target, control_flow| {
        match event {
            Event::NewEvents(StartCause::Init) => {
                // The tray icon must be created once the event loop is running.
                app = Some(App::new(target));
                #[cfg(target_os = "macos")]
                wake_run_loop();
            }
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                if let Some(app) = app.as_mut() {
                    app.tick(false);
                }
            }
            Event::UserEvent(UserEvent::Tray(e)) => {
                if let Some(app) = app.as_mut() {
                    app.handle_tray(e);
                }
            }
            Event::UserEvent(UserEvent::Menu(e)) => {
                if let Some(a) = app.as_mut() {
                    if a.handle_menu(&e) {
                        app = None; // drops the tray icon so it disappears immediately
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                }
            }
            _ => {}
        }
        *control_flow = ControlFlow::WaitUntil(next_whole_second());
    })
}

/// Wake up at the start of the next second so the display never lags the real clock.
fn next_whole_second() -> Instant {
    let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let into_second = Duration::from_nanos(since_epoch.subsec_nanos() as u64);
    Instant::now() + (Duration::from_secs(1) - into_second) + Duration::from_millis(5)
}

#[cfg(target_os = "macos")]
fn wake_run_loop() {
    use objc2_core_foundation::CFRunLoop;
    // Without this nudge the icon may not appear until the first timer fires.
    #[allow(unused_unsafe)]
    unsafe {
        if let Some(rl) = CFRunLoop::main() {
            CFRunLoop::wake_up(&rl);
        }
    }
}

/// Whether the system is in dark mode (used for clocks without a custom color when
/// another clock has one, so the image can't be a recolorable template).
fn system_is_dark() -> bool {
    #[cfg(target_os = "macos")]
    {
        Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().eq_ignore_ascii_case("dark"))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

impl App {
    fn new(target: &EventLoopWindowTarget<UserEvent>) -> Self {
        let config = config::load();
        // Create the file on first run so it's there to find and edit.
        if config::modified_time().is_none() {
            let _ = config::save(&config);
        }
        let (menu, items) = build_menu(&config);
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(config.panel.left_click == ClickAction::Menu)
            .with_tooltip("TimeStrip")
            .build()
            .expect("failed to create the menu bar item");
        let (scale, screen_width) = target
            .primary_monitor()
            .map(|m| {
                let s = m.scale_factor();
                (s, m.size().width as f64 / s)
            })
            .unwrap_or((2.0, 1440.0));
        let mut app = App {
            config,
            config_mtime: config::modified_time(),
            renderer: Renderer::new(),
            tray,
            items,
            last_signature: String::new(),
            settings_child: None,
            panel: PanelProcess::default(),
            dark_menu_bar: system_is_dark(),
            appearance_checked: Some(Instant::now()),
            scale,
            screen_width,
            last_rect: None,
        };
        app.tick(true);
        app
    }

    fn tick(&mut self, mut force: bool) {
        // Pick up changes from the settings window (or a hand-edited file).
        let mtime = config::modified_time();
        if mtime != self.config_mtime {
            self.config_mtime = mtime;
            let fresh = config::load();
            if fresh != self.config {
                let zones_changed = fresh.zones != self.config.zones;
                self.config = fresh;
                if zones_changed {
                    self.rebuild_menu();
                }
                self.tray.set_show_menu_on_left_click(self.config.panel.left_click == ClickAction::Menu);
                force = true;
            }
        }

        // Re-check light/dark mode every few seconds.
        if self.appearance_checked.is_none_or(|t| t.elapsed() > Duration::from_secs(5)) {
            self.appearance_checked = Some(Instant::now());
            let dark = system_is_dark();
            if dark != self.dark_menu_bar {
                self.dark_menu_bar = dark;
                force = true;
            }
        }

        let now = Utc::now();
        let local = clock::local_tz();
        let c = &self.config;
        let segments = clock::segments(now, local, c);

        let signature = format!("{segments:?}");
        if force || signature != self.last_signature {
            self.last_signature = signature;
            let auto = if self.dark_menu_bar { [255, 255, 255] } else { [0, 0, 0] };
            match self.renderer.render(&segments, c, auto) {
                Some(img) => {
                    let icon = Icon::from_rgba(img.rgba, img.width, img.height).ok();
                    let _ = if img.template {
                        self.tray.set_icon_templated(icon)
                    } else {
                        self.tray.set_icon(icon)
                    };
                    self.tray.set_title(None::<&str>);
                }
                None => {
                    // Nothing pinned to the menu bar: show a small symbol instead.
                    let _ = self.tray.set_icon(None);
                    self.tray.set_title(Some("🕒"));
                }
            }
        }

        // Keep the dropdown menu's times current.
        for (item, zone) in self.items.zones.iter().zip(&c.zones) {
            item.set_text(clock::menu_line(now, local, zone, c));
        }
        let tooltip: Vec<String> = c.zones.iter().map(|z| clock::copy_line(now, z, c)).collect();
        let _ = self.tray.set_tooltip(Some(tooltip.join("\n")));
    }

    fn rebuild_menu(&mut self) {
        let (menu, items) = build_menu(&self.config);
        self.tray.set_menu(Some(Box::new(menu)));
        self.items = items;
    }

    fn handle_tray(&mut self, e: TrayIconEvent) {
        if let TrayIconEvent::Click { rect, button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            self.last_rect = Some(rect);
            if self.config.panel.left_click == ClickAction::Panel {
                self.toggle_panel();
            }
        }
    }

    fn toggle_panel(&mut self) {
        if self.panel.is_open() {
            self.panel.close();
            return;
        }
        if self.panel.just_closed() {
            return;
        }
        // Center the panel under the clock, keeping it on screen.
        let (x, y) = match self.last_rect {
            Some(r) => (
                (r.position.x + r.size.width as f64 / 2.0) / self.scale,
                (r.position.y + r.size.height as f64) / self.scale + 4.0,
            ),
            None => (self.screen_width - 220.0, 30.0),
        };
        let half = (self.config.panel.width * self.config.panel.zoom) as f64 / 2.0;
        let x = x.clamp(half + 8.0, (self.screen_width - half - 8.0).max(half + 8.0));
        self.panel.open(x, y);
    }

    /// Returns true when the app should quit.
    fn handle_menu(&mut self, e: &MenuEvent) -> bool {
        let id = &e.id;
        let now = Utc::now();
        if id == self.items.quit.id() {
            if let Some(child) = self.settings_child.as_mut() {
                let _ = child.kill();
            }
            self.panel.close();
            return true;
        } else if id == self.items.open_panel.id() {
            self.toggle_panel();
        } else if id == self.items.settings.id() {
            self.open_settings();
        } else if id == self.items.edit_file.id() {
            reveal(&config::config_path());
        } else if id == self.items.copy_all.id() {
            let text: Vec<String> =
                self.config.zones.iter().map(|z| clock::copy_line(now, z, &self.config)).collect();
            copy(text.join("\n"));
        } else if let Some(login_item) = &self.items.login {
            if id == login_item.id() {
                let want = !login::is_enabled();
                let _ = login::set_enabled(want);
                login_item.set_checked(login::is_enabled());
            }
        }
        if let Some(i) = self.items.zones.iter().position(|item| item.id() == id) {
            if let Some(z) = self.config.zones.get(i) {
                copy(clock::copy_line(now, z, &self.config));
            }
        }
        false
    }

    fn open_settings(&mut self) {
        if let Some(child) = self.settings_child.as_mut() {
            if matches!(child.try_wait(), Ok(None)) {
                return; // already open
            }
        }
        self.settings_child = crate::spawn_self(&["--settings"]);
    }
}

fn build_menu(c: &Config) -> (Menu, MenuItems) {
    let menu = Menu::new();
    let now = Utc::now();
    let local = clock::local_tz();

    let zones: Vec<MenuItem> = c
        .zones
        .iter()
        .map(|z| MenuItem::new(clock::menu_line(now, local, z, c), true, None))
        .collect();
    for item in &zones {
        let _ = menu.append(item);
    }
    if zones.is_empty() {
        let _ = menu.append(&MenuItem::new("No clocks yet — add some in Settings", false, None));
    }
    let _ = menu.append(&PredefinedMenuItem::separator());

    let open_panel = MenuItem::new("Show Clocks & Calendar", true, None);
    let copy_all = MenuItem::new("Copy All Times", true, None);
    let settings = MenuItem::new("Settings…", true, None);
    let edit_file = MenuItem::new("Show Config File", true, None);
    let _ = menu.append(&open_panel);
    let _ = menu.append(&copy_all);
    let _ = menu.append(&settings);
    let _ = menu.append(&edit_file);

    let login = login::supported().then(|| {
        let item = CheckMenuItem::new("Open at Login", true, login::is_enabled(), None);
        let _ = menu.append(&item);
        item
    });

    let _ = menu.append(&PredefinedMenuItem::separator());
    let quit = MenuItem::new("Quit TimeStrip", true, None);
    let _ = menu.append(&quit);

    (menu, MenuItems { zones, open_panel, copy_all, settings, edit_file, login, quit })
}

fn copy(text: String) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text);
    }
}

/// Shows the file in Finder (or the platform's file manager).
pub fn reveal(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open").arg(path.parent().unwrap_or(path)).spawn();
}
