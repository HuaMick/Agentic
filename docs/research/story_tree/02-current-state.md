# 02 — Current state snapshot

## Read these files first (canonical references)

Before doing any further work, read in this order:

1. `README.md` — project status, installed state.
2. `CLAUDE.md` — driving rules (agents, workflow, env quirks, core
   principles).
3. `stories/README.md` — story corpus index + conventions.
4. `agents/README.md` — agent roster + authority boundaries.
5. `docs/decisions/0001` through `0005` — the five ADRs that shape
   everything else.

Then skim:

6. `crates/README.md` (if it exists) or browse `crates/` — seven active
   crates, each with its own README.
7. `schemas/story.schema.json` — authoritative shape of a story.
8. `patterns/` — two active patterns (standalone-resilient-library,
   fail-closed-on-dirty-tree).

## Corpus at time of writing

**13 stories, all `healthy`:**

| id | title | crate |
|----|-------|-------|
| 1 | UAT signs verdict + promotes story | agentic-uat |
| 2 | CI test-run recorder | agentic-ci-record |
| 3 | Four-status dashboard (narrowed post-realignment) | agentic-dashboard |
| 4 | Store trait + MemStore | agentic-store |
| 5 | SurrealStore (embedded surrealkv) | agentic-store |
| 6 | Story YAML loader + schema + DAG check | agentic-story |
| 9 | Dashboard staleness scoped to `related_files` | agentic-dashboard |
| 10 | DAG-primary frontier dashboard | agentic-dashboard |
| 11 | UAT ancestor-health gate | agentic-uat |
| 12 | Subtree-scoped test selector | agentic-ci-record |
| 13 | Unhealthy-ancestor classifier | agentic-dashboard |
| 15 | `agentic test-build plan/record` (claude-as-user) | agentic-test-builder |

**Retired (hard-deleted YAMLs + tests):**

- Story 7 — deterministic panic-stub scaffolder. Folded into 15.
- Story 8 — CLI-wiring story. Folded into 1+3.
- Story 14 — `agentic test-build` authored scaffolds via claude
  subprocess (claude-as-component). Superseded by 15.

The retirements are currently destructive. Future work is to introduce
a `retired` lifecycle status so retirements become metadata rather than
deletion (see `03-tree-metaphor.md`).

## Seven active crates

- `agentic-store` — `Store` trait + `MemStore` + `SurrealStore`
  (embedded `surrealkv`). **Cloud swap here is where Phase 1 starts.**
- `agentic-story` — YAML loader, schema validation, DAG + cycle check.
- `agentic-uat` — signed verdict runner; ancestor-health gate;
  dirty-tree refusal.
- `agentic-ci-record` — per-story test result recorder; subtree-scoped
  test selector.
- `agentic-dashboard` — four-status + DAG-primary view with frontier
  filter, blast radius, selectors, `--all`, `--expand`, drilldown.
- `agentic-test-builder` — `plan` + `record` CLI backing the
  red-state-as-committable-atomic contract. **Zero AI surface in the
  library.** (Story 15.)
- `agentic-cli` — the `agentic` binary entrypoint.

## Architectural posture

**Claude is a user of the system, not a component of it.**

Post session 2026-04-20 realignment:

- No product library spawns `claude` or any LLM subprocess.
- Only `agentic-runtime` (still `_deferred/`) is sanctioned to spawn
  claude, per ADR-0003's amended scope.
- Test-builder, test-uat, build-rust — all AGENTS — drive the CLI as
  users (Edit/Write/cargo/agentic commands).
- This is non-negotiable going into cloud. Replicating the claude-as-
  component mistake in a cloud setting would be catastrophic.

Read `docs/decisions/0003-claude-code-subscription-subprocess.md`
`## Scope` section for the canonical statement.

## The plan/record workflow (story 15)

The core authoring loop looks like:

```
agentic test-build plan <id> --json    # read-only; structured plan
<user/agent writes scaffolds via Edit/Write>
agentic test-build record <id>         # probes + atomic evidence
```

This is how red-state evidence gets produced. Story 11's ancestor gate
then gates UAT signing. Story 13's classifier surfaces inherited
unhealthy state.

## Current workflow pain points (pre-cloud)

These are the rough edges that make cloud worth pursuing now:

- **Subagent git coordination.** Parallel subagent commits can
  inadvertently bundle work if one runs `git rm` / `git add` while
  another is about to commit. Mitigated with `.claude/worktrees/`
  isolation (committed in `e3c3ef3`) but still fragile.
- **Local environment drift.** Cargo.lock dirty from random builds;
  pre-existing untracked files; every UAT agent has to account for
  dirt that isn't their concern.
- **Binary currency.** After impl edits we must reinstall `agentic` via
  `cargo install --path crates/agentic-cli --force` before UAT. Easy
  to forget. In a sandbox model, each sandbox has its own built
  binary; currency is automatic.
- **Store cross-contamination.** UATs use `--store <temp>` to avoid
  polluting the real store, but this leaks periodically. Cloud-backed
  store with per-sandbox write scoping would fix this structurally.
- **Sequential bottleneck.** The orchestrator session serialises most
  work. Real parallel agent work requires sandbox isolation.

## Known outstanding issues

These are tracked but not blocking:

- Orphan `uat_signings` rows for retired stories 7, 8, 14 in the store
  (data cruft, not blocking).
- `agentic-runtime` crate still `_deferred/` (hasn't been needed yet;
  will be needed for cloud agent spawning per ADR-0003).

## Useful commands for orienting a fresh session

```bash
# All currently healthy stories
wsl bash -c "cd /home/code/Agentic && grep -l '^status: healthy' stories/*.yml | sort"

# Current git state (commits ahead of origin, working tree)
wsl bash -c "cd /home/code/Agentic && git status && git log --oneline origin/main..HEAD"

# Workspace test health
wsl bash -c "source ~/.cargo/env && cd /home/code/Agentic && cargo test --workspace --no-fail-fast 2>&1 | grep -E '^test result' | tail -20"

# Dashboard in frontier view
/home/mick/.cargo/bin/agentic stories health

# Dashboard in forest view (shows all stories)
/home/mick/.cargo/bin/agentic stories health --all
```
