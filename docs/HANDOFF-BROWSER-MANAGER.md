# Handoff — browser manager & CDP remote (L4)

Status: **design validated live on the host, not started.** Validated 2026-08-31
against Brave 152.0.7977.64 (Chromium 152) on the owner's Windows machine.

## Goal

Make beam a true remote for the host's browser: select tabs, read play state,
seek, click Netflix's "Skip Intro" / "Next Episode" — all focusless — by
driving the browser over the DevTools protocol (CDP), with an onboarding flow
that detects the browser and proposes (never forces) enabling the channel.

## Verified facts (tested on this machine)

- **The flag works on Brave's default profile.** `brave.exe
  --remote-debugging-port=9222` cold-start answered `GET
  http://127.0.0.1:9222/json/version` with the full CDP handshake. The
  Chromium-136 hardening that ignores the flag on default data dirs is
  `GOOGLE_CHROME_BRANDING`-gated; Brave (non-Google branding) did not adopt
  it. **Consequence: no dedicated profile needed for Brave.**
- **No elevation or special rights needed.** Everything happens inside the
  interactive user session: enumerate processes, close them, relaunch them.
- **ProcessSingleton: flags only apply on a cold start.** Launching
  `brave.exe --flag` while Brave runs just opens a window in the existing
  process and silently drops the flag. A restart flow must fully exit the
  browser before relaunching.
- **Background mode keeps the browser alive after window close.** Observed:
  after a graceful close (WM_CLOSE), ~16 brave.exe processes remained —
  still holding the ProcessSingleton *and still serving the debugging port*.
  A restart flow must treat "closed" as "process tree fully exited" and
  force-stop leftovers after the graceful close.
- **The port is localhost-only and unauthenticated.** Anything local could
  drive that browser. Accepted for this home setup (owner decision, same
  trust model as beam itself: LAN, no auth). Never pass a non-loopback bind
  address to the flag.
- **Chrome/Edge proper would refuse the default-dir flag** (136+ hardening).
  Out of scope; if ever wanted, those need `--user-data-dir` pointing
  somewhere non-default (which is effectively the dedicated-profile story).
  Firefox has no CDP (BiDi someday) — keystrokes only.

## Feature 1: browser manager (the onboarding)

A const table of known browsers, brave first:

| browser | CDP | note |
|---|---|---|
| `brave.exe` | yes | primary target |
| `chrome.exe`, `msedge.exe` | degraded | default-dir flag ignored on 136+; would need a custom `--user-data-dir` — defer |
| `firefox.exe` | no | keystrokes only, forever (until BiDi) |

States and phone actions (the button **is** the consent):

| detected state | phone shows |
|---|---|
| not running | **[Start with remote control]** |
| running, CDP off | **[Restart with remote control]** — copy warns tabs restore but unsaved work is lost |
| running, CDP up | "remote control active" → page flips into the deep remote |

**Detection** — process enumeration (`windows` crate, already in the tree)
+ `GET 127.0.0.1:<port>/json/version` probe. Beam uses its own port (e.g.
9223) when *it* launches, but also probes 9222 to recognize a user-launched
CDP browser. Any cached CDP state must carry a short TTL and be re-probed on
use — ports change per launch.

**Restart flow** (the part that bit during validation):

1. graceful close: `taskkill /IM brave.exe` (no `/F`) → WM_CLOSE
2. wait for exit; then force-stop leftovers (`/F`) — background mode will
   keep holding the singleton otherwise
3. cold launch with `--remote-debugging-port=<port> --no-first-run
   --no-default-browser-check`
4. probe `/json/version` until it answers (timeout → honest error, maybe the
   next Brave adopts the hardening — say so, don't hang)

**Nice touch:** after a successful restart, reopen the last media URL via
`Target.createTarget` so the flow lands back on Netflix/YouTube without
relying on the browser's own session-restore setting.

## Feature 2: the CDP remote ("select tab, etc.")

What the protocol actually gives us, once `/json/version` answers:

- **`GET /json/list`** (plain HTTP) — every tab: id, title, URL. This feeds
  the phone's tab list ("Brave > Netflix").
- **Activate a tab** — `Target.activateTarget` over the browser-level WS.
  ⚠️ CDP raises the *tab within its window*; it does **not** raise the OS
  window. Window focus still needs the `SetForegroundWindow` dance
  (synthesized-key workaround, see IDEAS.md L3 research).
- **Open a URL** — `Target.createTarget` (new tab in the existing window).
- **Page intelligence** — attach to a page target and `Runtime.evaluate`:
  play state (`document.querySelector('video').paused`), progress
  (`currentTime` / `duration`), seek (assign `currentTime`), and clicking
  player buttons (Netflix "Skip Intro", "Next Episode", "Back to browse")
  via DOM query + `.click()`.
- **Later, for free:** CDP events over a long-lived WS can push play-state
  changes — the shelved SMTC feature, done better, for browsers.

## Architecture (house patterns)

- `src/browsers.rs`: `BrowserService` trait — detect / start / restart /
  probe — with mock + OS backends, registered as app context like
  `InputService`/`ContextService`. The mock records actions and fakes states
  so the whole UI flow is testable under `--mock`.
- Actions (start/restart/activate/open) are **procedures** (user-initiated
  actions, errors as data). Ambient state (browser/CDP detected state, tab
  list, play state) rides the **context** side: extend the `/focus` GET
  route into a `/context` payload rather than adding more text endpoints.
- Two sources of truth once CDP is alive: the FOCUS strip (focused window,
  keystroke path) and the CDP session. Precedence rule: **an alive CDP
  session wins**; keystroke flows remain the zero-config fallback whenever
  CDP is absent (any window, any browser).

## Open questions

- Does Brave's own "continue where you left off" restore tabs after our
  restart? (Verify once; the `createTarget` reopen makes it moot for media.)
- Exe path discovery: standard install paths first
  (`C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe`),
  per-user install second; registry lookup only if needed.
- Port collision policy (own 9223 vs probing 9222) — simple version above is
  probably enough for a single-user home setup.
- Privacy: `/json/list` pushes real tab titles + URLs to the LAN. This is
  the moment the **pairing token** idea (IDEAS.md) stops being optional-ish.

## Out of scope here

L3 UIA (demoted: Firefox fallback only), SMTC (shelved — read-only added
little; CDP supersedes it for browsers), Chrome/Edge proper, Firefox CDP.

## Suggested build order

1. `BrowserService` detect + probe (read-only) → browser state on the page
2. start / restart procedures + verification probe
3. CDP read: tab list + play state (tab list card on the phone)
4. CDP act: activate tab (with window-raise), open URL
5. deep remote: seek, skip-intro, next-episode (Netflix/YouTube layouts)

## Pointers

`src/context.rs` (service seam pattern), `src/main.rs` (`FocusRoute` — the
route-vs-procedure rule in a doc comment), `src/ui.rs`, `assets/beam.js`
(visibility-gated poller), `docs/IDEAS.md` (roadmap; L4 item cross-links
here).
