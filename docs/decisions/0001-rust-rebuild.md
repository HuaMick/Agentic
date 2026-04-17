# ADR-0001: Rust for the rebuild

**Status:** accepted
**Date:** 2026-04-17

## Context

The legacy AgenticEngineering system is Python. It crashed under iteration: concurrent writes to TinyDB corrupted state, the orchestrator bloated to 1100+ lines of tangled logic, and feature accretion outpaced our ability to verify invariants. A rewrite was on the table regardless of language.

The choice of implementation language shapes what's possible and what's likely. Relevant trade-offs for this system:

- Many long-lived subprocess children (Claude agents) → strong process-lifecycle story needed.
- Load-bearing invariants (proof hash, lifecycle gates, resilience boundaries) → compile-time enforcement is a force multiplier.
- Authored content (YAML stories, patterns, agents) + code that operates on it → rich type system helps, but speed of iteration matters too.
- Distribution: single-binary preferred, no interpreter dependency.

## Decision

Rewrite the system in **Rust**, as a cargo workspace with per-component crates.

## Alternatives considered

**Keep Python.** Rejected. Python was the substrate that failed. No compile-enforced module boundaries — any file could import any other, which is exactly how the legacy system tangled. Runtime type checking makes lifecycle invariants impossible to structurally enforce. A "Python done right" pass would be a rewrite in all-but-name anyway.

**Go.** Rejected. Simpler than Rust and good subprocess story, but: no sum types (Status enum with `Tested` / `Healthy` etc. is a natural fit for a Rust enum; Go would use constants or interfaces, weakening the invariant). Interface satisfaction is structural and implicit, which we don't want — we want crates to declare exactly what they expose. Garbage collection + goroutines is fine but not a differentiator here.

**TypeScript (on Deno or Node).** Rejected. Structural typing + runtime lookups defeat the compile-enforced boundary argument. Single-binary distribution requires bundling, which adds complexity for no gain. Claude Code's own ecosystem is TypeScript-adjacent (MCP, SDK), but we're not inside that ecosystem — we're spawning its CLI.

**Elixir / Erlang.** Rejected. Excellent for orchestration and supervision trees, but the authoring-artifact tooling (schemas, serde, JSON Schema codegen) is less mature, and we'd still need Rust-level ergonomics to satisfy the prove-it gate's "concentrated, auditable" requirement.

## Consequences

**Gained:**

- Compile-time enforcement of crate boundaries. `agentic-verify` cannot `use agentic_orchestrator::*` because we don't list it in Cargo.toml — the legacy's tangle-by-import is structurally impossible.
- Sum types for lifecycle (`Status::Tested` etc.) make invalid states unrepresentable.
- Module privacy (`pub(crate)`) lets us restrict verdict construction to the verify crate alone — no other code can write `status = tested`.
- Single static binary, fast startup, no interpreter.
- Strong async runtime (`tokio`) fits the subprocess-per-agent model.

**Given up:**

- Steeper learning curve for authors not already Rust-literate. Mitigation: the per-crate READMEs spell out public API shape and design decisions; detailed Rust knowledge is not needed to redline the design.
- Slower first-prototype cycle than Python. Mitigation: Phase 1 is explicitly "design on paper" — we are not rushing to compile.
- No official Anthropic Rust SDK. Mitigation: use `claude-code-rs` (community, direct port of Python SDK's subprocess pattern) to drive the local `claude` binary. See ADR-0003.

## Related

- ADR-0002 (SurrealDB): document-store choice shaped by Rust's embedded-DB ecosystem.
- ADR-0003 (subscription via subprocess): no official Rust SDK is one of the constraints that made this the right path.
- ADR-0004 (no bootstrap generator): trait-enforced simplicity over tooling.
