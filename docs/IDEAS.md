# Ideas & follow-ups

A working backlog for beam. Nothing here is committed to — each item should
earn its place. Sizes: **S** (< 1h), **M** (an evening), **L** (multi-session).
Keep the YAGNI rule when pulling from this list: one item at a time, vertical
slice, shipped.

## Infrastructure

### Run at logon ("service" mode) — M
Beam must run inside the **interactive user session** to inject keystrokes, so
a classic Windows Service (session 0) cannot work. The correct pattern is
*logon autostart in the user session*:

- **Windows**: Task Scheduler logon trigger (`schtasks /create /tn beam /sc onlogon /tr "<path>\beam.exe"`) or a Startup-folder shortcut.
- **macOS**: `LaunchAgent` plist in `~/Library/LaunchAgents` with `RunAtLoad`.
- **Linux**: systemd user unit wanted by `graphical-session.target`.

Nice version: a `beam install` / `beam uninstall` subcommand that registers
the platform-specific task idempotently, plus a tray icon showing the URL and
a "quit" action.

### Opt-in pairing token — M
beam is unauthenticated by design (see README security note), which is only
okay on trusted Wi-Fi. Optional hardening: generate a random token at startup,
print `http://<ip>:<port>/?k=<token>`, and reject procedures/pages that don't
carry it. Cheap to build on top of Topcoat's request context; keep it opt-in
(`--token`) so the LAN-trusted case stays frictionless.

### Host-side awareness — S
A small toast/tray notification on the host when a device connects or sends
input. It should never be possible for another device to type into the host
*silently*.

## UI / UX

### Style rework — M
Current CSS is a functional placeholder. Direction: keep zero build step
(embedded CSS), but extract tokens (spacing, radius, palette) into CSS
custom properties, add a light/dark theme via `prefers-color-scheme`, make
touch targets ≥ 44px and the textarea the visual hero. The Topcoat `tailwind`
feature is an alternative if we want utility classes — but that pulls the
asset pipeline; only adopt if the plain CSS starts hurting.

### Client feedback — S
Haptic (`navigator.vibrate`) and/or a subtle button-press animation on send;
disable page sleep while the tab is open (`screen wake lock API`) so the
"remote" doesn't lock mid-presentation.

## Features

### Open link in default browser — S
Quick-action: paste/typing a URL on the phone opens it on the host.
Sketch: new `open_url` procedure using the `open` crate; **validate the scheme
allow-list (`http`/`https`) before invoking** so a spoofed procedure call can't
launch arbitrary executables. UI: one input row + "Open" button.

### More keys & combos — M
Arrow keys, `Esc`, `Home/End`, `PageUp/Down`, `F1–F12`, and modifier combos
(`Alt+Tab`, `Ctrl+C`, `Win`). Domain change in `src/input.rs`: `KeyName` grows,
and `press_key` becomes `press_key(Combo { modifiers: Vec<Modifier>, key: KeyName })`
with down/down/up/up sequencing through enigo. This unlocks everything below.

### Presentation remote preset — S (after combos)
Arrows + `Enter`/`Esc`/`B`/`F5` + a big "black screen" button. A second UI
tab that turns any phone into a clicker for slides. Probably beam's best
single-purpose use case.

### Media & volume keys — S (after combos)
Play/pause, next/prev, volume up/down/mute via enigo's media key variants
(verify exact `Key` names at implementation time). Four-row tile grid, done.

### Host dashboard — M
A `#[shard]`-powered info panel (or `/` section): hostname, OS, uptime, and
the **current focused window title** (platform-specific; on Windows via
`GetForegroundWindow` + `GetWindowTextW` from the `windows` crate already in
the dependency tree). Optionally a recent-injection log (ring buffer of the
last N events). Also a natural place to surface "input backend: enigo/mock".

### Live typing mode — L
Continuous typing without pressing Send: stream input events (small batches,
~50 ms debounce) over the topcoat `websocket` feature and inject as they
arrive. The textarea stays local-first; this is a separate "live" tab. Watch
for reordering (include a sequence number server-side).

### Snippets — S
Saved quick-send texts stored in `localStorage` (client-side only — no
database, per the design rules). Chip row above the textarea.

### Wake the host screen — S
A "wake" button that nudges the mouse 1px or presses a harmless key via enigo,
so the host display wakes without walking over. Fold it into the quick keys.

## Explicitly out of scope (for now)

- **Mouse/touchpad control** — doubles the scope (pointer UI, latency tuning); revisit only if beam-as-keyboard proves itself.
- **Clipboard sync host→phone** — needs HTTPS for `navigator.clipboard`; messy on LAN http. The textarea paste path already covers the common direction.
- **Databases, accounts, TLS** — see README; the moment one of these feels necessary, re-read the design rules first.
