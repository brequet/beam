# beam — development tasks (`just` lists them)
[windows]
set shell := ["pwsh", "-NoLogo", "-NoProfile", "-Command"]

default:
    @just --list

# Build the debug binary.
build:
    cargo build

# Bundle UI assets next to the exe (required before run; per-profile).
bundle:
    topcoat asset bundle

# Build, bundle, run the real server in the foreground (blocks; agents: dev-mock).
run *args:
    topcoat asset bundle
    cargo run -- {{args}}

# Start a DETACHED mock server (rebuilds + re-bundles first; no real keystrokes).
dev-mock port="5001":
    pwsh -NoLogo -NoProfile -File scripts/beam-dev.ps1 start -Port {{port}} -Mock

# Start a DETACHED server with REAL injection (rebuilds + re-bundles first).
dev port="5000":
    pwsh -NoLogo -NoProfile -File scripts/beam-dev.ps1 start -Port {{port}}

# Stop the detached dev server listening on a port.
dev-stop port="5001":
    pwsh -NoLogo -NoProfile -File scripts/beam-dev.ps1 stop -Port {{port}}

# Print the dev server log (follow=1 streams it).
dev-log port="5001" follow="":
    pwsh -NoLogo -NoProfile -File scripts/beam-dev.ps1 log -Port {{port}} {{ if follow != "" { "-Follow" } else { "" } }}

# Run unit tests.
test:
    cargo test

# Lint with clippy.
lint:
    cargo clippy --all-targets

# Format all Rust code.
fmt:
    cargo fmt

# Lint + tests (CI gate).
check: lint test
