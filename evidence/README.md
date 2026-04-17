# evidence/

Append-only record of every verify run, organized per story. This is the trust layer — a Verdict is only meaningful if the evidence exists and the evidence is tamper-resistant.

## Layout

```
evidence/
└── runs/
    └── <story-id>/                             # one dir per story that has ever been verified
        └── <ISO-8601-timestamp>-<short-commit>.jsonl
```

One file per verify run. The filename encodes timestamp and commit, so concurrent verifies against the same story never collide — no locking, no coordination required.

## File format

**JSON Lines.** One JSON object per line. Lines are distinct record kinds (envelope / test / verdict) so a reader can stream-parse without loading the whole file.

Minimum fields per record (captured in `stories/1.yml` guidance and enforced by `agentic-verify`):

| Field | Required | Notes |
|-------|----------|-------|
| `run_id` | yes | UUID v4, ties all lines in this file together |
| `story_id` | yes | integer matching `stories/<id>.yml` |
| `commit` | yes | full git hash; repo was clean at this SHA |
| `timestamp` | yes | RFC3339 UTC, matches the filename stem |
| `verdict` | yes (envelope only) | `"Pass"` or `"Fail"` |
| `test_results` | yes | array of `{file, outcome, duration_ms, stderr_tail}` |

Schema shape for the records will be added as `schemas/evidence.schema.json` when `agentic-verify` ships.

## Rules

- **Append-only.** Once a file is written, it is not modified. Writers use `O_APPEND` with one `write()` per record. Readers tolerate a truncated trailing line (kill -9 mid-flush must not break future verifies).
- **Never hand-edit.** The file is the proof. If you need to invalidate a verdict, write a new verdict record under a new timestamp; do not mutate history.
- **Never deleted in normal operation.** A re-verify writes a new file alongside the old; it does not overwrite. Manual cleanup (e.g., during UAT) is acceptable but should be documented in the story's UAT cleanup steps.
- **Not mutated from the story.** The story YAML holds no evidence content — only the convention of where to find it (`evidence/runs/<id>/`). This separation eliminates the concurrency bugs of mutable-status tracking that sank the legacy system.

## Current state

Empty. The `.gitkeep` marker preserves the `evidence/runs/` directory. First real files appear when `agentic-verify` ships (Phase 2) and runs against story 1.
