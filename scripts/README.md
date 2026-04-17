# scripts/

Human-facing convenience scripts. Bootstrap tooling that exists before the full Rust CLI, with the explicit intent that each one be replaced by a proper `agentic <subcommand>` once the CLI ships.

## Current scripts

### `agentic-search.sh`

Search story YAML files by term(s) across title / outcome / guidance / acceptance.uat. Phase-1 bootstrap — returns matching story IDs + titles for agents and humans so work can begin before the full Rust CLI exists.

**Contract:**

```
agentic-search.sh <terms...> [--field outcome|title|guidance|uat|all]
                             [--json] [--quiet] [--help]
```

- `<terms...>` — positional words (quote multi-word phrases). Case-insensitive. At least one required.
- `--field F` — restrict to one field. Default: `all` (title + outcome + guidance + uat).
- `--json` — machine-readable output.
- `--quiet` — suppress non-error stderr (e.g. the yq-fallback warning). **Use this when agents run the script and log its output verbatim.**
- Exit codes: `0` success (even with zero hits), `1` runtime error, `2` usage error.

**Dependencies:** Prefers `yq` (mikefarah v4). Falls back to a grep/awk parser that handles common YAML scalar shapes and `|`/`>` block scalars. Fallback prints a stderr warning unless `--quiet` is set.

**Replacement plan:** once the Rust workspace ships an `agentic` binary, `agentic search` will subsume this functionality with a proper schema-aware parser, dependency-graph awareness, and structured output. At that point delete this file and migrate callers.

## Rule for new scripts here

- Must have a header comment block describing purpose, contract, exit codes, dependencies, and replacement plan.
- Must be self-contained (no hidden repo-relative assumptions beyond walking up to `.git`).
- Must exit non-zero on error with a clear message.
- Must have a `--help` flag.
- Should accept `--quiet` when noise in agent summaries is a real concern.

Anything that requires coordinated state across scripts belongs in a crate, not here.
