# AGENTS.md

Guidance for coding agents working in this repository.

## Project

**beam** — single-binary Rust web server (Topcoat + tokio) that serves a page
to devices on the local Wi-Fi and injects what they send as keyboard input
into the host's focused window (enigo). No auth, no database, by design.

- `src/main.rs` — clap CLI (`--host`, `--port`, `--mock`), router wiring, serving
- `src/keys.rs` — the key catalogue: every `Key` a device can send, with its
  wire name, label, and kind; the single source of truth for `press_key` and
  the page's key buttons
- `src/input.rs` — `InputService` trait; `OsInput` (enigo) and `MockInput` backends
- `src/ui.rs` — Topcoat `view!` page + `send_text` / `press_key` procedures

Domain rules: the web layer only talks to the `InputService` trait; the key
catalogue (`keys.rs`) is the single source of truth for wire names and labels;
the enigo injection mapping lives in `input.rs`; errors surfaced to the
browser are data (`Result<String, String>`), never exceptions.

## Commands

Use the justfile (pwsh shell on Windows):

- `just build` — debug build
- `just bundle` — build + bundle UI assets (required before `cargo run`; the
  bundle is per-profile, so re-run after `--release` builds)
- `just run` — bundle then run in the foreground (blocks; interactive use only)
- `just dev-mock` — rebuild + re-bundle, then start a detached mock server on
  :5001 (no real keystrokes); also kills any stale server on the port first
- `just dev` — same, on :5000 with REAL injection
- `just dev-stop` / `just dev-log` — manage the detached server
- `just check` — clippy + tests; `just fmt` — format

## Environment rules (Windows / pwsh agent shells)

- **Never start background servers with `Start-Process`** (or a blocking
  `cargo run`): the spawned process stays inside the shell's process tree and
  the harness waits on it — tool calls appear to hang for minutes.
- The only supported pattern is `scripts/beam-dev.ps1`: it spawns via WMI
  (`Win32_Process.Create`) through a `.cmd` wrapper that owns the log handles,
  so the server is fully detached and the calling shell returns immediately.
- Kill servers by port (`just dev-stop`), never leave `beam.exe` running when
  a session ends. Stale logs/wrappers live in `target/` (gitignored).
- Before committing, check `git status` for browser-test artifacts —
  `.playwright-mcp/` is gitignored but new tool dirs can appear.

## Verification

1. `just check` (lint + unit tests) before any commit.
2. For UI/procedure changes: `just dev-mock`, exercise the page in the browser,
   confirm events in `just dev-log`, then `just dev-stop`.

## References

- Topcoat API quirks and gotchas: the user-level **topcoat** skill.
- Roadmap / follow-up ideas: `docs/IDEAS.md`.
