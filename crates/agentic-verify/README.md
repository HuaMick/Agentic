# agentic-verify

## What this crate is

⭐ **The prove-it engine.** Takes a `Story`, runs its acceptance criteria, records evidence, issues a `Verdict` (Pass or Fail). This is the gate that every story must clear before it can be marked proven.

## Why it's a separate crate

This is the most important design decision in the whole system.

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
    pub outcome: Outcome,   // Pass | Fail { failed_criteria: Vec<CriterionId> }
    pub commit: String,
    pub run_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub criterion_results: Vec<CriterionResult>,
    pub trace_ref: PathBuf,
}
```

## Dependencies

- Depends on: `agentic-story`, `agentic-store`, `agentic-events`, `git2` (for commit hashing)
- Depended on by: `agentic-orchestrator`, `agentic-cli`

## Design decisions

- **Evidence is append-only and external to the story file.** Each verify run writes one file under `evidence/runs/<story-id>/<timestamp>-<commit>.jsonl`. The story YAML holds a pointer, not the data. Concurrent verifies don't collide.
- **A Verdict is signed by a commit hash.** You cannot fabricate a Pass — it requires a clean git state and a recorded commit.
- **Verify is deterministic.** Given the same story + same commit + same environment, the verdict is reproducible. Non-determinism is a bug and surfaces as a stress-test failure.
- **Verify does not spawn agents.** It runs acceptance commands directly (shell, cargo test, custom binaries). No LLM in the loop at verification time.
- **Failure is first-class.** A Fail verdict is as valuable as a Pass. It's recorded, it's searchable, it tells us what didn't work.

## Open questions

- How do we handle flaky tests? Two options: (a) retry with limit in the criterion definition, (b) fail and require the story author to address flakiness explicitly. Leaning (b) — flakiness = story quality problem.
- Should verify also trigger the promote transition, or is promote a separate explicit step? Leaning: verify issues the verdict, a higher layer (orchestrator or CLI) decides to promote.

## Stress/verify requirements

- Running verify on the same story 100 times in parallel produces 100 non-colliding evidence files.
- A corrupted evidence file (e.g., a partial write from a kill -9) does not break future verifies.
- `agentic verify <bad-id>` fails cleanly with a clear error, not a panic.
- Verify can be invoked with zero dependencies on any other agentic-* crate beyond `agentic-story`, `agentic-store`, `agentic-events`.
