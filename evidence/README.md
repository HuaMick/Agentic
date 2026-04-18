# evidence/

**The file-based verdict model described in earlier drafts has been superseded.** Verdict records — the trust layer — now live in `agentic-store` (SurrealDB embedded via `surrealkv`). This directory is used **only** for test-builder red-state artefacts, per ADR-0005.

## Where verdicts actually live

In the DB (`agentic-store`), two tables carry the proof:

- **`test_runs`** — upserted per story+commit. One row per `(story_id, commit)`; a re-run at the same commit overwrites. Fields include `run_id` (UUID v4), `story_id`, `commit`, `timestamp`, `verdict` (`Pass` / `Fail`), and `test_results: [{file, outcome, duration_ms, stderr_tail}]`.
- **`uat_signings`** — append-only. One row per `agentic uat` signature promoting a story to `healthy`. Never updated, never deleted. References the `test_runs` row it attests.

A story transitions to `healthy` only when a `uat_signings` row exists pointing at a `Pass` `test_runs` row at a clean commit. `agentic stories health` (story 3) reads these tables; nothing else is authoritative.

See `crates/agentic-store/README.md` for the Store trait, and `crates/agentic-uat/README.md` for the signing flow (story 1, unstarted).

## What this directory is for

Test-builder red-state artefacts. Per ADR-0005, the red-green contract requires test-builder to commit proof that new tests fail before `build-rust` implements against them. That proof lands here:

```
evidence/
└── runs/
    └── <story-id>/
        └── <ISO-8601-timestamp>-<short-commit>.jsonl
```

One JSON-lines file per red-state capture. Each line is a distinct record (envelope / test / outcome). These files are **not** verdicts — they are pre-implementation artefacts proving the test suite is honest.

## Rules for the red-state files

- **Append-only.** Writers use `O_APPEND` with one `write()` per record. Readers tolerate a truncated trailing line.
- **Never hand-edit.** The file is the proof of an honest red state.
- **Never mutated from the story.** The story YAML holds no evidence content — only the convention of where red-state artefacts land.

## Why two homes

File-based artefacts are committable atomics that travel with the commit that introduced the failing test. Query-able verdict history (dashboards, health roll-ups, "which stories regressed since Tuesday") wants a DB. Keeping the two concerns in their natural homes eliminates the mutable-status concurrency bugs that sank the legacy system.

## Current state

The `.gitkeep` marker preserves `evidence/runs/`. First red-state file will land when `agentic-ci-record` (story 2) leaves its scaffold state and test-builder writes a real failing test through it.
