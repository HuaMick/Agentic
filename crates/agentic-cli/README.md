# agentic-cli

## What this crate is

The `agentic` binary. `clap`-based subcommand tree that dispatches to library crates. Owns human-vs-machine output formatting (`--json` flag) and the exit-code contract (0 = pass, 1 = real fail, 2 = could-not-verdict).

## Why it's a separate crate

Domain crates (`agentic-uat`, `agentic-dashboard`, ...) stay usable as libraries without pulling in `clap` or the argv-parsing surface. Keeping the CLI separate means the command surface can evolve without rippling into the types.

## Current subcommands (shipped)

```
agentic uat <id> --verdict <pass|fail> [--store <path>]   # story 1
agentic stories health [<id>] [--json] [--store <path>]   # story 3
```

Both subcommands honour `--store` and `AGENTIC_STORE` consistently; see `stories/1.yml` "Store location — default and override" for the authoritative resolution rule.

## Dependencies

- Depends on: `agentic-store`, `agentic-uat`, `agentic-dashboard`, `clap` (derive), `serde_json`, `git2`, `dirs`.
- Depended on by: nothing (the `[[bin]]` target `agentic` is the binary consumed by `cargo install --path crates/agentic-cli`).

The `agentic-entry` crate's latency-fast-path sketch (see its README) is not yet wired; day-one operators install `agentic-cli` directly.

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
