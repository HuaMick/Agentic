# agentic-entry

## What this crate is

The `agentic` binary. A thin launcher that peeks at `argv`, handles a fast-path for agent-name queries (e.g., `agentic build-rust --help`) before loading the rest of the CLI, and otherwise delegates to `agentic-cli`.

## Why it's a separate crate

Keeps startup cheap. In the legacy Python system, `agentic <agent-name>` was a hot path that needed to skip the 200ms CLI import cost. In Rust this matters less, but the split keeps `agentic-cli` as a library consumable from tests and other binaries, not tied to a particular binary's startup concerns.

## Public API sketch

```rust
// main.rs only — no library surface
fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    if let Some(cmd) = fast_path::try_handle(&argv) { return cmd; }
    agentic_cli::run(argv)
}
```

## Dependencies

- Depends on: `agentic-cli`
- Depended on by: nothing (it's a binary)

## Design decisions

- **No logic lives here.** Fast-path detection is the one exception, and only for latency-critical agent-help lookups.
- **Binary crate, not lib.** This is the only binary in the day-one set (besides `agentic-stress`).

## Open questions

- Is the fast path worth it in Rust? Benchmark before building.

## Stress/verify requirements

- Startup latency under 50ms for `agentic --help`.
- Unknown-command exit code matches CLI conventions.
