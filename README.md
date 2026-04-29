# Agentic

A Rust-based agent orchestration system built around story-driven development.

**Status:** Phase 2 / Phase 0 keystone passed. Cargo workspace active (rustc 1.95.0 via rustup in WSL); seven crates compile green. Twenty stories `healthy` (1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 15, 16, 17, 18, 19, 23, 25, 26, 27) after the 2026-04-28 autonomous cascade plus follow-up sessions that landed story 19 (agentic-runtime un-deferred — `Runtime` trait + `ClaudeCodeRuntime` + `MockRuntime` + `green` verdict codified in ADR-0005), then promoted stories 17 (build_config schema), 11 (UAT ancestor-health gate), and 23 (`agentic test-build record` emits the three-verdict shape — `red`/`preserved`/`re-authored` — per ADR-0005 sub-amendment 1, closing the Phase-0 known issue where amendment passes had to self-author the evidence JSONL because the CLI hard-refused on any non-red scaffold). Two remaining `under_construction` (20, 21 — Phase 0 work; story 20's scaffolds are re-redded under ADR-0005 sub-amendment 1 and the workspace E0597 is fixed, but the host-CLI implementation work itself remains forward); one `proposed` (24); three retired (7, 8, 14). The `dag-primary-lens` epic (stories 10-13) remains complete. The `agentic` binary is installable via `./install.sh` (or `cargo install --path crates/agentic-cli` directly; `./install.sh --docker` builds a container image instead) and exposes four top-level subcommands: `agentic uat <id> --verdict <pass|fail>`, `agentic stories health|test|audit`, and `agentic test-build [plan|record]`.

## What this is

A rebuild of [AgenticEngineering](https://github.com/HuaMick/AgenticEngineering) in Rust, starting from first principles. The legacy Python system lives under `legacy/AgenticEngineering/` as a git submodule for reference.

## Core philosophy

1. **Story-driven from the ground up.** Every unit of work is a story with executable acceptance criteria. A story is not `healthy` until a Pass verdict is recorded with evidence. The gate is enforced in code, not policy.
2. **Red-green is a contract, not a convention.** The test-builder agent authors failing scaffolds (using its normal authoring tools); `agentic test-build record` verifies the red state and writes atomic evidence. Build-rust writes implementation source and never edits tests. See ADR-0005.
3. **Slow, stress-tested growth.** The legacy system crashed because it bloated faster than we could verify it. We add code only when a failing test demands it.
4. **Claude is a user of the system, not a component of it.** `agentic-runtime` (the orchestrator crate) spawns `claude` via subprocess to run subagents; product libraries (`agentic-uat`, `agentic-test-builder`, `agentic-store`, etc.) are strictly AI-free and treat claude as an external user — same category as a human developer — who exercises the CLI. Subscription auth via the local `claude` binary, no API billing. See ADR-0003, ADR-0004.
5. **Trait-first, pluggable everywhere it matters.** Runtime, sandbox, store — each is a trait with one impl at a time. See ADR-0002.
6. **Defects amend the owning story, not a new story.** When a defect is found in a healthy story's impl, add a new `acceptance.tests[]` entry to THAT story, auto-revert its status, scaffold the new test red, fix the impl, re-UAT. Stories stay single-owner over their domain; the corpus doesn't fragment.

## Repository layout

```
crates/            Rust workspace (7 active crates + _deferred/ placeholders)
agents/            Authored YAML agent definitions (the product)
assets/            Reusable cross-corpus assets (9 active; consumed by agents and stories; schema in schemas/asset.schema.json)
.claude/agents/    Hand-written pointer .md files that delegate to agents/
.claude/hooks/     Programmatic enforcement of agent contracts (PreToolUse hooks)
patterns/          Reusable design guidance referenced by stories
schemas/           JSON Schemas — authoritative shape of authored artifacts
stories/           User stories — the unit of work
epics/             Epic folders (groups of stories with shared context)
evidence/          Test-builder red-state artefacts only (verdicts live in agentic-store)
projects/          User-space: projects this harness is being used on
docs/              Architecture notes, guides, Architecture Decision Records
legacy/            Old AgenticEngineering codebase (git submodule, reference only)
scripts/           Human-facing convenience scripts (agentic-search.sh)
```

## Active crates

- `agentic-store` — `Store` trait + `MemStore` + `SurrealStore` (backed by `surrealkv` embedded LSM; chosen over full `surrealdb` for compile-memory budget, see workspace `Cargo.toml`). Stories 4 + 5 shipped.
- `agentic-story` — YAML loader with schema validation, DAG check on `depends_on`, and the optional `related_files` field (stories 6 + 9 shipped).
- `agentic-uat` — signed verdict runner with `UatExecutor` trait, dirty-tree refusal, and transitive-ancestor-health gate on `--verdict pass` (stories 1 + 11 shipped).
- `agentic-ci-record` — per-story `test_runs` upserter plus subtree-scoped `CiRunner` with selector grammar and pluggable `TestExecutor` trait (stories 2 + 12 shipped).
- `agentic-dashboard` — DAG-aware four-status story-health view with frontier-default filtering, `--expand`/`--all` flags, selector grammar (`+id`, `id+`, `+id+`), blast-radius columns, subtree drilldown, related-files staleness, and ancestor-inherited unhealthy classification (stories 3 + 9 + 10 + 13 shipped).
- `agentic-cli` — `agentic` binary entrypoint exposing `uat`, `stories health|test`, and `test-build plan|record` (stories 1, 3, 10, 11, 12, 15 shipped).
- `agentic-test-builder` — plan-and-record CLI library backing `agentic test-build`: emits a structured `PlanEntry` per acceptance test, probes user-authored scaffolds via `cargo check` + `cargo test`, and writes atomic red-state JSONL evidence. Strictly AI-free; no claude subprocess, no LLM dependency. User (human or claude-as-agent) authors the scaffolds with their own tools (story 15 shipped, superseding the retired story 7 panic-stub authoring and retired story 14 claude-in-library approach).

## Setup — pre-commit hook (one-time per clone)

The repo ships a tracked pre-commit hook at `.githooks/pre-commit` that
refuses commits whose post-commit corpus state would render as drift
under `agentic stories health --all` or `agentic stories audit`. Per
story 29's enforcement contract, every clone wires it on once with:

```
git config core.hooksPath .githooks
```

The hook fails open if the `agentic` binary is not on PATH (so a fresh
clone before `./install.sh` is not chicken-and-egg blocked), and exits
non-zero with the offending story id named in stderr if any drift is
detected. The hook is the structural gate that keeps the YAML status
field and the `uat_signings` / `manual_signings` tables in sync — once
it is wired, hand-edited promotions cannot be committed.

## Active agents

- `planner/story-writer` — story and pattern curator.
- `build/build-rust` — implements Rust source to drive scaffolded tests green.
- `teacher/guidance-writer` — curator of agent specs and the shared `assets/` layer.
- `test/test-builder` — authors failing test scaffolds and records red-state evidence per ADR-0005.
- `test/test-uat` — executes a story's UAT walkthrough and invokes `agentic uat` with the signed verdict.

## Current state

**Healthy (20):** stories 1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 15, 16,
17, 18, 19, 23, 25, 26, 27. The Phase-0 keystone cascade (4 → 5 → 6
→ 1 → 16 → 18) ran in a single autonomous session on 2026-04-28,
promoting twelve stories (4, 5, 6, 1, 16, 18, 2, 3, 12, 27, 26, 25)
and resolving the defects-amend-the-owning-story ripple from story
18's signer-mandatory contract change. Follow-up sessions same day
landed story 19 (agentic-runtime un-deferred), then promoted story
17 (build_config schema parsed by the loader) and story 11 (UAT
ancestor-health gate refuses Pass on unproven ancestry, including
retired-chain semantics). The 2026-04-29 session promoted story 23
(`agentic test-build record` emits the three-verdict shape per
ADR-0005 sub-amendment 1: `red` for first-authoring, `preserved`
for unchanged scaffolds, `re-authored` for amended scaffolds whose
per-scaffold justification text moved since the most recent
evidence row), closing the Phase-0 known issue where amendment
passes self-authored the evidence JSONL because the CLI hard-
refused on any non-red scaffold.

**Under construction (2):** stories 20, 21. Story 20's scaffolds
are re-redded under ADR-0005 sub-amendment 1's three-gate carve-out
(YAML had drifted past the most recent red evidence), the
workspace-blocking E0597 in its `commit_author_email` test helper is
fixed, and three of its 12 acceptance tests pass on already-shipped
infrastructure (`compose_docker_argv`, `ImageTagResolver`, the
docker-binary precheck). The remaining nine — sandbox green/exhausted/
crashed wiring, host auto-merge, drift refusal, amend-same-story,
CLI subcommand wiring — are the substantive implementation work
deferred to the next session.

**Proposed (1):** story 24 (`agentic test-build record` diagnostic
classifier; tightens probe to E0432/E0599 only).

**Retired (3):** stories 7, 8, 14.

**Programmatic enforcement.** `.claude/hooks/build_rust_guard.py` is a
PreToolUse hook that enforces three of build-rust's contractual
boundaries that already had clear documentation but were violated
anyway: `cargo fmt` is denied outright, Bash invocations must match an
explicit allowlist, and `Write`/`Edit` to `crates/*/tests/**` and `Write`
to `stories/**` are blocked. See `.claude/hooks/README.md` for the
specifics; the hook only fires for the build-rust subagent.

Stories 7, 8, and 14 were retired during the session that shipped
story 15:
- **Story 7** (deterministic panic-stub scaffolder) and **story 14**
  (library wraps `claude` subprocess to author scaffolds) were folded
  into **story 15** — a plan-and-record CLI under the claude-as-user
  model where the library never spawns an LLM. Story 14 was the
  claude-as-component anti-pattern that killed the legacy Python
  system; catching it before it compounded is one of this session's
  main wins.
- **Story 8** (CLI wiring) was consolidated into stories 1 and 3 on
  2026-04-19 after an audit found the split was along library/binary
  crate boundaries rather than user journeys.

Retired story IDs are not reused. See `stories/README.md` for the
full retirement rationale.

See **`CLAUDE.md`** for driving instructions (including the WSL push quirk) and the current story roster. Full list in `stories/README.md`.

## Quick reference

- **Authoring a story:** `docs/guides/story-authoring.md`.
- **Authoring a pattern:** `docs/guides/pattern-authoring.md`.
- **Invoking an agent:** `docs/guides/invoking-agents.md`.
- **Why these choices:** `docs/decisions/` (ADR-0001 through ADR-0005).
