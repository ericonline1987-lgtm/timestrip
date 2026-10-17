# TimeStrip

A macOS menu bar world clock and calendar, written in Rust. Like Clocker, but you control
exactly how every region looks and behaves: size, font, weight, color, time and date
format, per region, plus a month calendar and your calendar events in every region's time.

```
NYC 8:35 PM   LON 1:35 AM Fri   TYO 9:35 AM Fri
```

## Features

**Menu bar**
- Any number of regions; pick which ones appear in the bar
- Move regions up or down in Settings → Regions; uncheck **Show in menu bar** to keep
  a region available in the expanded panel without pinning it to the bar
- Your own short labels (`NYC`, `Mum`, `HQ`…)
- Text size from 8 to 40 pt, any installed font, weights from Thin to Black
- Single-line or stacked layout (small label above a larger time)
- Fixed-width digits, spacing between regions, letter spacing, an optional separator
- Vertical nudge for pixel-perfect alignment
- Dim or hide labels; custom color, or automatic light/dark
- 12/24-hour, AM/PM, seconds, or your own time format
- Show the date never, always, or only when that region is on a different day than you

**Per-region styles** — any region can override the defaults: its own size, font,
weight, color, label, 24-hour, AM/PM, seconds, time format, date display and date format.
Make your home office big and bold and the rest small and dim.

**Panel** (click the clock)
- Every region's time, date and difference from you; click one to switch the calendar to it
- A time slider (±24 h) to see what time it will be everywhere
- A month calendar for any region: today is marked in *that* region's date, with week
  numbers, any first day of the week, and dimmed weekends
- Your Calendar app events, with colored dots on busy days; each event shows its time in
  the selected region and in your other regions (e.g. `NYC 8:00 PM · TYO 9:00 AM Fri`)
- An upcoming-days list, and buttons for Settings and the Calendar app
- Independently reorder the calendar's region tabs, including Local, in Settings →
  Calendar & Panel → Calendar region order
- Link holiday calendars to one or more regions; their colored date markers, hover
  titles and event names follow the selected region

**Right-click menu** — every region's time, date and offset (click to copy), Copy All
Times, Show Clocks & Calendar, Settings…, Open at Login, Quit.

**Language & time zone** — the interface is available in English, 简体中文, 繁體中文,
日本語, 한국어, Español, Français and Deutsch (or follows your system). Weekday and month
names can use the same language or a different one. "Local" follows your Mac's time zone,
or you can pick another one in Settings → Language.

**Settings window** with a live preview on light and dark menu bars. Changes are saved
instantly; the menu bar and panel update within a second.

## Install

### Download a build

Every push builds a universal (Apple Silicon + Intel) `TimeStrip.app`. Open the latest
run under **Actions → Build**, download `TimeStrip-macos`, unzip it, and drag
`TimeStrip.app` to Applications.

The app is not notarized, so the first time macOS will refuse to open it. Either
right-click it → **Open**, or run:

```bash
xattr -dr com.apple.quarantine /Applications/TimeStrip.app
```

### Build it yourself

Requires [Rust](https://rustup.rs) and the Xcode Command Line Tools (`xcode-select --install`).

```bash
git clone https://github.com/ericonline1987-lgtm/timestrip.git
cd timestrip
scripts/bundle-macos.sh --install
```

That builds `TimeStrip.app`, copies it to `/Applications` and launches it. For a quick
run without bundling: `cargo run --release`.

## Configuration

Everything in the settings window is stored in a JSON file you can also edit by hand
(the clock reloads it automatically):

```
~/Library/Application Support/TimeStrip/config.json
```

```json
{
  "zones": [
    { "tz": "America/New_York", "label": "NYC", "show_in_bar": true },
    { "tz": "Asia/Tokyo", "label": "TYO", "show_in_bar": true,
      "style": { "font_size": 16, "color": [255, 159, 10], "use_24h": true } }
  ],
  "layout": "inline",
  "font_family": "Menlo",
  "font_size": 14.0,
  "font_weight": 500,
  "date_mode": "when_different",
  "date_format": "%a",
  "calendar": { "first_weekday": 1, "show_week_numbers": true }
}
```

Any setting you leave out uses its default. Time zones use
[IANA names](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones); time and
date formats use [strftime syntax](https://docs.rs/chrono/latest/chrono/format/strftime/index.html).

### Holiday sources and calendar order

In **Calendar & Panel → Add a holiday source**, paste an `https://`, `http://`, or
`webcal://` ICS subscription URL and click **Subscribe in Calendar…**. Complete the
confirmation in macOS Calendar, return to TimeStrip and click **Refresh list**.
Calendar manages subscription updates and recurring events. You can also use holiday
calendars already present in Calendar, such as US Holidays or China Holidays.

Enable **Show events from the Calendar app** and grant calendar access, then use
**Holiday sources by region** to choose the region and check its sources. A linked
source appears only in its associated regions; ordinary calendars still use the
**Calendars to show** selection. Unlinking a source leaves it hidden from ordinary
events until you explicitly enable it there. All-day holidays retain their dates
when switching regions, including when Local uses a custom time zone.

The `calendar.region_order` array stores IANA zone names, with `""` representing
Local. Missing regions are appended automatically. `calendar.holiday_calendars`
maps the same region keys to arrays of macOS calendar identifiers. These optional
fields preserve older configs; **Follow clock order** restores the default tab order.

## Notes

- The menu bar is about 22 pt tall. Larger text is scaled down to fit, so the stacked
  layout is the way to get bigger digits into the same space.
- Hold ⌘ and drag the clock to move it within the menu bar.
- "Open at Login" adds `~/Library/LaunchAgents/com.timestrip.app.plist`.
- Calendar events are read-only. The first time you click "Show my calendar events",
  macOS asks for permission; you can change it later in System Settings → Privacy &
  Security → Calendars. Events need the bundled `TimeStrip.app` (not `cargo run`),
  because macOS only grants calendar access to apps that explain why they need it.

## How it works

| File | What it does |
| --- | --- |
| `src/tray.rs` | Menu bar item and dropdown ([tray-icon](https://github.com/tauri-apps/tray-icon) + [tao](https://github.com/tauri-apps/tao)); ticks once a second |
| `src/panel.rs` | Clock & calendar panel ([egui](https://github.com/emilk/egui)), launched as `timestrip --panel x y` |
| `src/calendar.rs` | Month-grid math |
| `src/events.rs` | Calendar events via EventKit ([objc2](https://github.com/madsmtm/objc2)) |
| `src/render.rs` | Draws the clocks into an image with [ab_glyph](https://github.com/alexheretic/ab-glyph), using any font found by [fontdb](https://github.com/RazrFalcon/fontdb) |
| `src/clock.rs` | Time zone math and formatting ([chrono-tz](https://github.com/chronotope/chrono-tz)) |
| `src/settings.rs` | Settings window ([egui](https://github.com/emilk/egui)), launched as `timestrip --settings` |
| `src/config.rs` | The JSON config file |
| `src/login.rs` | Open at Login |

## License

MIT
