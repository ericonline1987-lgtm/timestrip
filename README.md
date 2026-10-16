# TimeStrip

A small macOS menu bar world clock, written in Rust. Like Clocker, but you control
exactly how it looks: size, font, weight, layout, spacing, color and date format.

```
NYC 8:35 PM   LON 1:35 AM Fri   TYO 9:35 AM Fri
```

## Features

**Menu bar**
- Any number of clocks; pick which ones appear in the bar (the rest live in the dropdown)
- Your own short labels (`NYC`, `Mum`, `HQ`…)
- Text size from 8 to 40 pt, any installed font, weights from Thin to Black
- Single-line or stacked layout (small label above a larger time)
- Fixed-width digits so the clock doesn't jiggle
- Spacing between clocks, letter spacing, an optional separator (`·`, `|`, `/`…)
- Vertical nudge for pixel-perfect alignment
- Dim or hide labels; custom color, or automatic light/dark
- 12/24-hour, AM/PM (upper or lower case), seconds, or your own time format
- Show the date never, always, or only when that clock is on a different day than you

**Dropdown menu**
- Every clock with its date and difference from your time
- Click a clock to copy its time; "Copy All Times" copies them all
- Settings…, Show Config File, Open at Login, Quit

**Settings window** with a live preview on light and dark menu bars. Changes are saved
instantly and the real menu bar updates within a second.

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
    { "tz": "Asia/Tokyo", "label": "TYO", "show_in_bar": true }
  ],
  "layout": "inline",
  "font_family": "Menlo",
  "font_size": 14.0,
  "font_weight": 500,
  "date_mode": "when_different",
  "date_format": "%a"
}
```

Any setting you leave out uses its default. Time zones use
[IANA names](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones); time and
date formats use [strftime syntax](https://docs.rs/chrono/latest/chrono/format/strftime/index.html).

## Notes

- The menu bar is about 22 pt tall. Larger text is scaled down to fit, so the stacked
  layout is the way to get bigger digits into the same space.
- Hold ⌘ and drag the clock to move it within the menu bar.
- "Open at Login" adds `~/Library/LaunchAgents/com.timestrip.app.plist`.

## How it works

| File | What it does |
| --- | --- |
| `src/tray.rs` | Menu bar item and dropdown ([tray-icon](https://github.com/tauri-apps/tray-icon) + [tao](https://github.com/tauri-apps/tao)); ticks once a second |
| `src/render.rs` | Draws the clocks into an image with [ab_glyph](https://github.com/alexheretic/ab-glyph), using any font found by [fontdb](https://github.com/RazrFalcon/fontdb) |
| `src/clock.rs` | Time zone math and formatting ([chrono-tz](https://github.com/chronotope/chrono-tz)) |
| `src/settings.rs` | Settings window ([egui](https://github.com/emilk/egui)), launched as `timestrip --settings` |
| `src/config.rs` | The JSON config file |
| `src/login.rs` | Open at Login |

## License

MIT
