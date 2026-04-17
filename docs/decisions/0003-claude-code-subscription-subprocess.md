# ADR-0003: Claude Code subscription via subprocess

**Status:** accepted
**Date:** 2026-04-17

## Context

The system drives Claude to do agent work — planning, building, testing. Per-token API billing is unacceptable at our expected volume (a single orchestrated epic can easily run thousands of tool-use turns). We want to use the existing Claude Pro / Max subscription.

The legacy Python system used `claude-agent-sdk`, which routes through the local `claude` Code CLI (subprocess, NDJSON over stdio) and inherits whatever auth that CLI has. Subscription use "just works" because `claude -p "<prompt>"` reads OAuth tokens from the local install.

For Rust, the relevant facts (verified by a research subagent in session-of-record):

- No official Anthropic Rust SDK. All crates on crates.io (`anthropic-sdk-rust`, `rusty-anthropic`, etc.) are raw REST wrappers — API-key only.
- `claude-code-rs` (community; decisiongraph) is a direct structural port of the Python SDK's subprocess transport. Same pattern: spawn `claude`, pipe NDJSON both ways, inherit local auth.
- Plain `claude -p "<prompt>" --output-format stream-json --verbose` uses the subscription. The `--bare` flag skips OAuth and demands an API key — we never use `--bare`.
- Subscription auth lives in the local `claude` install: macOS Keychain (`Claude Code-credentials`) or `~/.claude/.credentials.json` on Linux/Windows (mode 0600). A user running our Rust binary inherits their own auth transparently.
- Quota is unified across claude.ai, Desktop, and Claude Code (interactive + headless). A `claude -p` invocation counts against the same 5-hour session window as interactive use.
- The `Task` tool is blocked for subagents (claude-code issue #4182). Subagent fanout has to happen outside Claude — which is exactly our orchestrator model.

## Decision

Drive Claude via subprocess. Wrap the `claude-code-rs` crate (or fork if needs diverge) inside `agentic-runtime` behind a `Runtime` trait. Day-one implementation: `ClaudeCodeRuntime` spawns `claude` with `--output-format stream-json --verbose`, pipes NDJSON over stdio, surfaces streaming events to callers.

Never pass `--bare`. Never import or depend on any raw Anthropic REST client crate.

## Alternatives considered

**Raw Anthropic API via `reqwest`.** Rejected. Per-token billing kills the economics. Loses tool use, streaming, and the Claude Code tool ecosystem (Read/Edit/Bash/MCP/plugins) — we'd be reimplementing all of it.

**HTTP daemon protocol to Claude Code.** Rejected. No such thing exists; Claude Code is a CLI, not a daemon. Inventing it would mean a fork of Claude Code, which we're explicitly not doing.

**Named pipes / custom IPC.** Rejected. `claude-code-rs` already does NDJSON-over-stdio well. Adding a custom transport layer is gratuitous.

**Skip Claude entirely; use a local open model.** Not evaluated here. Out of scope for the rebuild. Could become a second `Runtime` impl later (e.g., `OllamaRuntime`), but day-one focus is on the subscription-using path.

## Consequences

**Gained:**

- Subscription auth "just works" — if the user has run `claude login`, our binary inherits it. No config, no API key handling.
- Full Claude Code tool ecosystem: Read, Edit, Bash, Grep, Glob, WebFetch, MCP servers, skills, plugins, hooks. All available inside our spawned agents.
- Streaming by default (NDJSON events flow as they arrive).
- Rust-side subagent fanout. Since in-Claude `Task` is blocked for subagents, *we* own the spawn tree — Rust enforces depth limits, concurrency caps, and budget (when `_deferred/agentic-budget/` ships).
- `system/api_retry` events give us a native signal for 429 backoff.

**Given up:**

- Quota is shared with interactive Claude Code use. Long orchestration runs can exhaust the 5-hour window. Mitigation: stream-events let us detect 429s and back off; the future `_deferred/agentic-budget/` crate will cap wall-clock / retry counts.
- No way to query remaining quota programmatically (claude-code issue #32796). We react to 429s, we don't forecast.
- Depends on `claude` being installed and authenticated on the host. Our binary is useless without it. Mitigation: document this as a hard prerequisite; add a self-check at CLI startup.
- Anthropic policy area to watch: they announced (March 2026) that subscriptions "no longer cover third-party tools" that proxy subscription auth into SaaS. Our usage — a local binary shelling out to a locally logged-in `claude` — is functionally indistinguishable from any script. Still, monitor.

## Related

- ADR-0001 (Rust rebuild): no official Rust SDK was a constraint factored into language choice.
- `crates/agentic-runtime/README.md`: the `Runtime` trait and the `ClaudeCodeRuntime` impl.
- Story 1's `verify_standalone_resilience.rs` test does NOT spawn agents — that's deliberate. The verify path must work without any runtime at all.
