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

- **Windows**: done — `beam install` / `beam uninstall` register a Task
  Scheduler logon task scoped to the current user (no admin needed;
  `schtasks /sc onlogon` was avoided because it requires elevation). The task
  runs `beam --hidden`, has no execution-time limit, survives on battery, and
  refuses duplicate instances; status goes to `%LOCALAPPDATA%\beam\beam.log`.
- **macOS**: `LaunchAgent` plist in `~/Library/LaunchAgents` with `RunAtLoad`.
- **Linux**: systemd user unit wanted by `graphical-session.target`.

Remaining for the nice version: a tray icon showing the URL and a "quit"
action.

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

### More keys & combos — M
Arrow keys, `Esc`, `Home/End`, `PageUp/Down`, `F1–F12`, and modifier combos
(`Alt+Tab`, `Ctrl+C`, `Win`). Domain change in `src/input.rs`: `KeyName` grows,
and `press_key` becomes `press_key(Combo { modifiers: Vec<Modifier>, key: KeyName })`
with down/down/up/up sequencing through enigo. This unlocks everything below.

### Presentation remote preset — S (after combos)
Arrows + `Enter`/`Esc`/`B`/`F5` + a big "black screen" button. A second UI
tab that turns any phone into a clicker for slides. Probably beam's best
single-purpose use case.

### Media remote — v1 shipped
"02 — Remote" section: outcome-labeled buttons with key hints. Play/Pause,
Vol−/+, Mute send **global media keys** (`MediaPlayPause`, `VolumeUp/Down/Mute`)
that work regardless of host focus; Fullscreen/−10s/+10s send **real letter
keys** (`f`, `j`, `l` — enigo VK variants, not `.text()` Unicode injection,
which is why app shortcuts never fired). Letter keys need the host browser
focused; a static hint line says so. Lesson: neither needs combos, so
"after combos" was the wrong dependency. Leftovers: next/prev track, arrow
cluster — add only when needed.

### Host dashboard — M
A `#[shard]`-powered info panel (or `/` section): hostname, OS, uptime, and
the **current focused window title** (platform-specific; on Windows via
`GetForegroundWindow` + `GetWindowTextW` from the `windows` crate already in
the dependency tree). Optionally a recent-injection log (ring buffer of the
last N events). Also a natural place to surface "input backend: enigo/mock".
This is also where **live focus awareness** for the remote belongs: show the
focused window title so the UI can confirm "f will land in YouTube" (or warn
that focus is elsewhere) instead of relying on the static hint line.

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
