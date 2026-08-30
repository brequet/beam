# beam

A minimal, single-binary local network input bridge. It opens a web server on
your machine; any device on the same Wi-Fi can open the page and send text or
special keys (`Enter`, `Backspace`, `Tab`, `Space`) into the **focused window
on the host** — as if typed on a physical keyboard.

Built with [Topcoat](https://github.com/tokio-rs/topcoat) (server-rendered UI,
no frontend build step) and [enigo](https://crates.io/crates/enigo) (OS input
injection).

## Layout

```
src/main.rs        CLI (clap), router wiring, LAN IP detection
src/input.rs       InputService abstraction: OsInput (enigo) + MockInput (dev/tests)
src/ui.rs          Topcoat view! page + send_text / press_key / open_url procedures
src/autostart.rs   Logon-task install/uninstall (Windows) + --hidden helpers
```

The web layer only knows the `InputService` trait, so development and tests
never trigger real keystrokes: run with `--mock` and events are just logged.

Follow-up ideas and the raw backlog live in [docs/IDEAS.md](docs/IDEAS.md).

## Build & run

Prerequisites: Rust 1.85+ and the Topcoat CLI (used once to bundle the
embedded UI assets next to the executable):

```sh
cargo install topcoat-cli
```

Build, bundle assets, and run:

```sh
cargo build
topcoat asset bundle        # writes target/debug/assets (or target/release/assets for --release)
cargo run
```

Or use the dev loop (watch, rebuild, rebundle, auto-reload):

```sh
topcoat dev
```

Then scan the printed QR code with a phone on the same Wi-Fi (or open the
printed URL manually), e.g. `http://192.168.1.138:5000`.

### Options

| Flag      | Env         | Default   | Description                                  |
| --------- | ----------- | --------- | -------------------------------------------- |
| `--host`  | `BEAM_HOST` | `0.0.0.0` | Address to bind the web server to            |
| `--port`  | `BEAM_PORT` | `5000`    | Port to bind the web server to               |
| `--mock`  | —           | off       | Log input events instead of injecting them   |
| `--hidden`| —           | off       | Hide the console window (used by autostart)  |

```sh
cargo run -- --port 8080          # custom port
cargo run -- --mock               # safe for development
BEAM_PORT=8080 cargo run          # env fallback
```

### Run at logon (Windows)

```sh
beam install                      # autostart with default options
beam install --port 8080 --mock   # flags passed here are frozen into the task
beam uninstall                    # remove the task (stops a task-started beam)
```

`beam install` registers a Task Scheduler task with an *at log on* trigger
scoped to your user — no admin rights needed (and a Windows Service would not
work anyway: session 0 cannot inject keystrokes). The task runs
`beam --hidden`: no console window, and the current URL is written to
`%LOCALAPPDATA%\beam\beam.log`. The task never gets killed by the scheduler's
72-hour time limit and runs on battery.

The command is idempotent: re-running it refreshes the registration (e.g.
after moving or rebuilding the binary, or changing options). To start it right
away without waiting for the next logon: `schtasks /run /tn beam`.

## Platform notes

- **Windows**: works out of the box; some apps running elevated may ignore
  injected input from a non-elevated process.
- **macOS**: grant *Accessibility* permission to the terminal/binary
  (System Settings → Privacy & Security → Accessibility), or `Enigo::new`
  fails at startup.
- **Linux**: X11 is supported by default; Wayland support requires the
  `enigo` `wayland` feature.

## Security

There is no authentication by design: **anything on the network that can
reach the port can type into your machine.** Keep it on a trusted Wi-Fi, bind
to a specific interface with `--host` if needed, and stop the server when you
are done.

The `open_url` quick-action can additionally make the host open http(s) links
in its default browser; the scheme allow-list (`http`/`https` only) is
validated before the opener runs, so nothing else can be launched.
