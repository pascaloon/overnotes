# Overnotes

Pin notes over your game. Overnotes is a Windows 11 overlay app for taking and
connecting notes while playing — a Miro-like canvas (sticky notes, freehand
drawing, images) that floats over the game window with adjustable transparency.

Built with Rust + [Dioxus 0.7](https://dioxuslabs.com/) (WebView2).

## Windows

- **Launcher** — pick a running game and a notes document, then *Open Window*
  or *Open Overlay*. Shows attach status and a *Detach overlay* button.
- **Overlay** — borderless, transparent, topmost window glued to the game's
  client area.
  - *Edit mode*: full interaction — toolbar (select / note / draw / paste
    image), hamburger menu (document name, load document, transparency
    sliders), bottom bar (region screenshot, close).
  - *Overview mode*: click-through, chrome hidden, overview transparency.
  - `Ctrl+Shift+E` (global) toggles between the two.
- **Standalone** — the same editor in a regular window, for a second monitor
  or when the overlay isn't wanted.

## Canvas

Pan (drag), zoom (wheel), sticky notes with editable text and colors, freehand
strokes with color/width options, images pasted from the clipboard or captured
from the game with the region screenshot tool (Windows Graphics Capture).
Objects can be moved, resized (8 handles), and rotated. Documents autosave as
JSON to `%APPDATA%\overnotes\data\documents\<game>\<doc-id>\doc.json`, with
images stored alongside as PNG files.

## Overlay technique

Same family as the revamped Discord overlay / Xbox Game Bar: a separate
DWM-composited window, no game hooking.

- `WS_POPUP` borderless + DWM transparency + `WS_EX_TOPMOST`
- `WS_EX_TOOLWINDOW` keeps it out of the taskbar / Alt-Tab
- Overview mode adds `WS_EX_TRANSPARENT` + `WS_EX_LAYERED` (click-through)
  and `WS_EX_NOACTIVATE` (never steals game focus)
- A `SetWinEventHook` watcher thread keeps the overlay glued to the game's
  client rect, hides it when the game loses foreground or minimizes, and
  closes it when the game exits

Works over windowed and borderless-fullscreen games (not true exclusive
fullscreen, same limitation as Discord/Game Bar).

## Build & run

```powershell
cargo build

# Launcher
cargo run

# Direct launches
cargo run -- --window <game-exe>                 # standalone editor
cargo run -- --overlay <title-substring-or-hwnd> # overlay onto a running window

# Test target: a fake game window (animated scene, logs input to its title)
cargo run --bin dummy_game
```

## Testing helpers (`scripts/`)

PowerShell/Python utilities used to exercise the app end-to-end:

- `cdp.py` — drives the WebView2 UI over the Chrome DevTools Protocol
  (launch overnotes with
  `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222 --remote-allow-origins=*"`).
- `input.ps1`, `probe.ps1` — real `SendInput` mouse/keyboard injection
  (click-through validation).
- `capwin.ps1`, `caprect.ps1` — window/region screen capture.
- `movewin.ps1`, `showwin.ps1`, `winrect.ps1`, `winstyle.ps1`, `winpid.ps1`,
  `hittest.ps1`, `overlaytest.ps1`, `clipimg.ps1` — window management and
  inspection helpers.
