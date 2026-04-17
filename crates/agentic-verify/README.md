# agentic-verify

## What this crate is

⭐ **The prove-it engine.** Takes a `Story`, runs its acceptance tests, records evidence, issues a `Verdict` (Pass or Fail). This is the gate that every story must clear to reach `tested`.

The first concrete specification of this crate's behaviour lives in `stories/1.yml`. Read that alongside this README; several invariants below are expressed there as acceptance tests.

## Why it's a separate crate

**The prove-it gate must work even when the orchestrator is broken.** If `agentic-orchestrator` has a bug, if the planner crashed, if the runtime misbehaves — you can still run `agentic verify <story-id>` (or drive this crate directly from Claude Code) and get a trustworthy verdict.

Separating verify from everything else means:

1. Verify has the minimum possible dependency footprint.
2. Verify can be invoked standalone as a library, not requiring the full system.
3. When the wheels come off, this is the resilient layer.
4. A human reviewer can trust the verdict because the logic is concentrated and auditable.

## Public API sketch

```rust
pub struct Verifier {
    store: Arc<dyn Store>,
}

impl Verifier {
    pub async fn verify(&self, story_id: &StoryId) -> Result<Verdict, VerifyError>;
}

pub struct Verdict {
    pub story_id: StoryId,
    pub outcome: Outcome,        // Pass | Fail { failed_tests: Vec<PathBuf> }
    pub commit: String,          // full git hash
    pub run_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub test_results: Vec<TestResult>,
    pub trace_ref: PathBuf,
}

pub enum VerifyError {
    DirtyTree,                   // fail-closed; no verdict issued
    UnknownStoryId(StoryId),
    SchemaInvalid(ParseError),
    Io(std::io::Error),
    // ... typed errors, no panics
}
```

## Resilience boundary (load-bearing)

`agentic-verify` may depend **only** on:

- `agentic-story`
- `agentic-store`
- `agentic-events`
- `git2` (for commit hashing and working-tree state)

It **must not** depend on `agentic-orchestrator`, `agentic-runtime`, `agentic-sandbox`, or `agentic-cli`. This is enforced structurally by the workspace Cargo.toml; a test (`verify_standalone_resilience.rs`, per story 1) drives the verifier as a library to catch regressions.

If you find yourself wanting to add a dependency here, add a story that explains why and update this boundary. Don't quietly import.

## Exit-code contract (CLI surface)

When invoked via `agentic verify <id>` (Phase 2+):

- **0** — Pass verdict issued. Evidence file written.
- **1** — Fail verdict issued. Evidence file written.
- **2** — Could not produce a verdict (dirty tree, unknown story, I/O error, schema invalid).

The 1-vs-2 split matters for CI: exit 1 means "fix the code under test;" exit 2 means "investigate or retry." Do not conflate them.

## Key invariants

- **Evidence is append-only and external to the story file.** Each verify run writes one file at `evidence/runs/<story-id>/<ISO-8601-timestamp>-<short-commit>.jsonl`. The story YAML holds no evidence data. Concurrent verifies don't collide because the timestamp+commit pair is the uniqueness key. Partial writes (kill -9 mid-flush) are tolerated by readers — malformed trailing lines are ignored, not fatal.
- **A Verdict is signed by a commit hash.** Before running any test, the verifier reads `git rev-parse HEAD` AND confirms `git status --porcelain` is empty. Dirty tree → `VerifyError::DirtyTree`, no verdict written. This is the load-bearing trust invariant: a Pass without a commit is forgeable.
- **Verify does not mutate story status.** `agentic verify` writes evidence; it does NOT flip the story's `status` field. Promotion (`under_construction` → `tested`) is a separate layer's concern, scheduled via `agentic story audit` or the orchestrator. Keeping verify a pure function over (story, tree, env) eliminates a class of race conditions.
- **Verify is deterministic.** Given the same story + same commit + same environment, the verdict is reproducible. Non-determinism is a bug and surfaces as a stress-test failure.
- **Verify does not spawn agents.** It runs acceptance commands directly (shell, cargo test, custom binaries). No LLM in the loop at verification time.
- **Failure is first-class.** A Fail verdict is as valuable as a Pass. It's recorded, it's searchable, it tells us what didn't work.

## Evidence file schema (minimum fields per record)

```
run_id          UUID v4
story_id        int
commit          full git hash
timestamp       RFC3339 UTC
verdict         "Pass" | "Fail"
test_results    [{file, outcome, duration_ms, stderr_tail}]
```

See `evidence/README.md` for the full on-disk contract.

## Dependencies

- Depends on: `agentic-story`, `agentic-store`, `agentic-events`, `git2`
- Depended on by: `agentic-orchestrator`, `agentic-cli`

## Design decisions (cross-referenced)

- Evidence format and concurrent-write safety: rationalized in `stories/1.yml` and detailed in `evidence/README.md`.
- Why separate from orchestrator: see "Resilience boundary" above and ADR-0001.
- Exit-code split: justified in `stories/1.yml` guidance.

## Open questions

- How do we handle flaky tests? Two options: (a) retry with limit in the test definition, (b) fail and require the story author to address flakiness explicitly. Leaning (b) — flakiness = story quality problem.
- Should verify trigger the promotion transition, or is promote a separate explicit step? **Settled: separate.** Promote is not a verify concern. Captured above; keeping this bullet here as a historical note.

## Stress/verify requirements

- Running verify on the same story 100 times in parallel produces 100 non-colliding evidence files.
- A corrupted evidence file (e.g., partial write from kill -9) does not break future verifies.
- `agentic verify <bad-id>` fails cleanly with a clear error — no panic.
- Verify can be invoked with zero dependencies on any `agentic-*` crate beyond those in the resilience boundary.
- Dirty-tree refusal is hard (no override flag in release builds).
