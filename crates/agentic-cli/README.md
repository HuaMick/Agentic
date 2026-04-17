# agentic-cli

## What this crate is

The command-line interface. `clap`-based subcommand tree that dispatches to library crates. Owns human-vs-machine output formatting (`--json` flag).

## Why it's a separate crate

Keeping the CLI separate from the binary (`agentic-entry`) means tests and other tools can drive commands programmatically. Keeping the CLI separate from the domain crates means the command surface can evolve without rippling into the types.

## Public API sketch

```rust
pub fn run(argv: Vec<String>) -> ExitCode;

// Subcommand modules mirror the command tree:
mod commands {
    mod story;    // agentic story {new, list, verify, show}
    mod epic;     // agentic epic {new, list, archive}
    mod agent;    // agentic agent {list, validate, check-pointers}
    mod verify;   // agentic verify <story-id>
    mod config;
}
```

## Dependencies

- Depends on: `agentic-story`, `agentic-verify`, `agentic-agent-defs`, `agentic-store`, `agentic-orchestrator`
- Depended on by: `agentic-entry`

## Design decisions

- **`clap` with derive macros.** Declarative, produces good --help, integrates with rustdoc.
- **One module per subcommand group.** Mirror the command tree in the source tree. If `agentic story verify` exists, it lives in `commands/story/verify.rs`.
- **Output formatting is centralized.** `output::human()` vs `output::json()` helpers; no ad-hoc `println!` scattered.
- **No domain logic.** Command handlers translate argv → library call → formatted output. If logic is creeping in here, it belongs in a domain crate.

## Open questions

- `clap` vs `bpaf` vs hand-rolled? Going with `clap` unless there's a reason.
- Do we need a TUI mode? Deferred — `_deferred/agentic-stream/` is where that would go first.

## Stress/verify requirements

- All subcommands produce valid JSON under `--json`.
- Exit codes are documented and stable (0 = success, 2 = usage error, other non-zero = domain-specific).
- `--help` output is comprehensive and doesn't regress between versions.
