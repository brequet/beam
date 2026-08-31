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

### Esc key — S
`Esc` can ship ahead of the combo rework — a single key, no modifier
machinery (enigo already has `Escape`). The app-specific remotes need it:
in the Netflix player, `Esc` is "back to browse".

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

Successor path: **Media session awareness (SMTC)** below — a real timeline,
seek-to-position, and no host-focus requirement.

### Host dashboard — M
A `#[shard]`-powered info panel (or `/` section): hostname, OS, uptime, and
the **current focused window title** (platform-specific; on Windows via
`GetForegroundWindow` + `GetWindowTextW` from the `windows` crate already in
the dependency tree). Optionally a recent-injection log (ring buffer of the
last N events). Also a natural place to surface "input backend: enigo/mock".
The focused-window data itself comes from **Focused window context (L0)**
below; the dashboard is where it prints, and the remote consumes the same
signal so the hint line can say "f will land in YouTube" (or warn that focus
is elsewhere) instead of staying static.

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

## Host awareness (context-driven remotes)

Research arc from the "beam should know what the host is doing" exploration.
Layers stand alone; each sharpens the remote without requiring the next.
Guiding rule: clean seams — awareness enters as a new service trait beside
`InputService` (OS + mock backends), the web layer reads it through the
trait and reports errors as data, never touching Win32 directly.

### Focused window context (L0) — S/M
Simply print what is focused on the host: **full window title + process
name**, e.g. `"S01E03 — Netflix — Brave"`. Deliberately *no* parsing of
browser titles into app/tab (an earlier "L1" idea, rejected: the raw string
is enough and we don't want a title-parser to maintain per browser).
Windows: `GetForegroundWindow` + `GetWindowTextW` + process name, all in the
`windows` crate already in the tree. Prints on the host dashboard and feeds
the remote's hint line.

- **To research — update model:** page pulls a context procedure (simplest;
  at 1 Hz the Win32 calls cost microseconds) vs a `SetWinEventHook
  (EVENT_SYSTEM_FOREGROUND)` push model (needs a dedicated message-pump
  thread; zero idle cost, instant updates). And how it reaches the page:
  polling while the tab is visible vs topcoat websocket push.
- **To research — privacy:** publishing window titles to the LAN makes the
  pairing-token item above much less optional.

### Media session awareness (SMTC, L2) — M/L
Windows System Media Transport Controls (`windows::Media::Control`, or the
`gsmtc` crate for tokio event streams): every playing app — Brave/Netflix,
Firefox/YouTube, Spotify, VLC — registers a session with title, cover,
playback status, **position + duration**, and commands (play/pause, next/
prev, **seek-to-position**) that work **without host focus** — no keystroke
injection at all. Verified: Chromium (→ Brave) supports SMTC timeline +
seek; Firefox has shipped SMTC since 75. Browser-agnostic, so the Firefox
case needs no special casing.

- Explore **augmenting the media remote when a session is detected** (or a
  separate remote variant): real progress bar + scrubber, seek, an identity
  line ("Netflix — Brave"). Fall back to today's global-media-key remote
  when no session exists.
- Per-app remotes ride on top: **Netflix** and **YouTube** first (Netflix:
  "back to browse" is `Esc` in the player). Spotify / PowerPoint / VLC
  profiles deferred until asked for.
- **To research — updates over time:** SMTC is event-driven
  (`TimelinePropertiesChanged`, `MediaPropertiesChanged`) but position must
  be extrapolated from `LastUpdatedTime` between events; decide how the page
  learns (websocket push vs 1 Hz pull of a session snapshot).

### Browser tab switcher (UIA, L3) — L
A separate feature, not keystrokes: enumerate the open tabs of running
browsers via UI Automation (`uiautomation` crate; Chromium ships native UIA
by default since 138). Chrome/Brave first, Firefox second.

- The phone lists tabs as `"Brave > Netflix"`; tapping one **focuses that
  browser and tab** and it takes precedence, pinned to the top of the
  remote screen. Target flow: "select YouTube tab → press Play" with almost
  no mouse.
- Includes focus-steering research: `EnumWindows` to find the browser
  window + `SetForegroundWindow` (background processes are restricted; the
  standard workaround — synthesizing a harmless key first — fits enigo).
- Watch out: walking UIA trees is heavier than L0/L2 and can be slow with
  many tabs; likely on-demand refresh, not streaming.

### Deep browser integration (CDP, L4) — parked
Keep in mind, not scheduled: launching Chromium with
`--remote-debugging-port` unlocks full automation — list/activate/navigate
tabs, evaluate JS, click DOM buttons like Netflix's "Skip Intro" (which has
no keyboard shortcut). Heaviest layer, and it needs the browser relaunched
with a flag, so it is a deliberate opt-in. Revisit when L3 feels limiting.

## Explicitly out of scope (for now)

- **Mouse/touchpad control** — doubles the scope (pointer UI, latency tuning); revisit only if beam-as-keyboard proves itself.
- **Clipboard sync host→phone** — needs HTTPS for `navigator.clipboard`; messy on LAN http. The textarea paste path already covers the common direction.
- **Databases, accounts, TLS** — see README; the moment one of these feels necessary, re-read the design rules first.
