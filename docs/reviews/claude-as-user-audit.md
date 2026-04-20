# Claude-as-user audit of the story + pattern corpus

**Date:** 2026-04-20
**Scope:** every file under `stories/*.yml` and `patterns/*.yml`, plus
flagged cross-references into `docs/decisions/` (ADRs), `stories/README.md`,
and `CLAUDE.md`.
**Rule under audit:** Claude is a USER of the system, not a COMPONENT of
it. The library / CLI / schemas / store / evidence format must be
deterministic and fully testable without AI. Claude is one of several
possible users — same category as a human developer — who exercises the
CLI and makes judgement calls. Contracts read "given inputs A, B, C,
running command X produces observable Y." They do not read "the library
authors tests via claude" or "the CLI generates a diff."

This is an audit report only. No story, pattern, ADR, agent spec, or
durable instruction file was edited.

---

## stories/14.yml: test-builder authors real acceptance tests via the local claude subprocess

**Status:** healthy (signed at cb163e2).

**What's wrong.** The entire story contract is claude-as-component.
Every axis of the rubric fires.

- **(a) Contract framing.** `outcome`:
  > "A developer running `agentic test-build <id>` against a proposed
  > story receives scaffolds whose bodies are real acceptance tests
  > derived from each justification, so build-rust can drive the story
  > from red to green without editing the tests."

  The library is the author of the scaffolds; the human does not write
  them. Fields `guidance.Prompt contract (auditable)` (lines ~320-356)
  and `guidance.Determinism / cache contract` (lines ~368-398) embed
  the `claude -p` subprocess, prompt-assembly function, and a content-
  hashed scaffold cache as first-class parts of `agentic-test-builder`.

- **(b) Acceptance test shape.** Five of seven library-level tests
  PATH-stub `claude` and pin system behaviour around its output:
  - `claude_unavailable_is_fail_closed.rs` (line 88)
  - `claude_timeout_is_fail_closed.rs` (line 102)
  - `scaffold_body_is_parseable_rust.rs` (line 58) — tests what happens
    "if `claude`'s output is not parseable Rust."
  - `scaffold_body_is_cached_by_content_hash.rs` (line 73) — pins that
    a second run does NOT spawn `claude` and serves the cached body.
  - The binary-level `test_build_invokes_claude_and_writes_real_scaffold.rs`
    (line 116) explicitly names "a stubbed `claude` binary on `PATH`
    (via a tempdir that prepends an executable shim)."

- **(c) UAT walkthrough.** Setup step 2 demands
  `command -v claude` resolves locally; steps 5-14 walk through claude
  invocations, cache hits, and failure modes that only exist because
  claude is in the system.

- **(d) Guidance.** The `What is NEW in this story` block (lines ~259-272)
  names the three new typed errors — `ClaudeUnavailable`,
  `ClaudeTimeout`, `ScaffoldParseError` — as runtime primitives of the
  `agentic-test-builder` library. The `Standalone-resilience posture`
  block (lines ~424-439) goes further, explicitly embedding the claude
  subprocess into the standalone-resilient-library's dependency floor:
  > "`agentic-test-builder` remains standalone-resilient-library-
  > shaped (see the referenced pattern). The `claude` subprocess is
  > spawned via a small in-crate helper (NOT a dependency on
  > `agentic-runtime`, which depends on the world and would break the
  > resilience posture)."
  >
  > "`test-build` is on the critical verify path (red-state evidence
  > MUST be producible when the rest of the system is in flames) and
  > therefore owns its own narrow subprocess shim."

  This is the clearest form of the leakage: the story argues that
  because the test-builder is on the resilience path, it MUST own its
  own claude wrapper. The correct conclusion is the opposite — if
  red-state evidence must be producible when the rest of the system is
  in flames, then claude is not allowed on that path at all, because
  claude is outside the system. A human user (or a Claude Code
  orchestrator running the CLI) writes the scaffolds with Edit/Write.

**Proposed correction.** Retire this story. Per the user directive this
session, a replacement is already being drafted by a parallel
story-writer. That draft should frame the problem as "given a story
with substantive justifications, `agentic test-build <id>` records that
the story's declared test files are MISSING (red-by-absence), writes a
red-state evidence row, and the user (human OR Claude Code) authors the
test files with their own editor" — the CLI verifies and records; the
user creates. No further work on story 14 itself is needed from this
audit; noted here only so the user has a complete picture.

---

## stories/7.yml: Record each story's red state before implementation begins

**Status:** under_construction (README.md carries a footnote that it
shipped healthy at e5f4997 but was mutated by story 14's in-flight
work).

**What's wrong.** Nothing in the story 7 YAML itself is claude-as-
component. The story's guidance is explicit in the opposite direction:

  > "**Scaffold shape is deterministic from justification.** Given the
  > same justification text, test-builder produces a byte-identical
  > Rust scaffold. The test's function name is a snake_case reduction
  > of the first sentence; the `panic!(..)` message is the
  > justification's first line; there is no timestamp, UUID, or other
  > non-deterministic content in the scaffold body." (lines ~254-257)

That is a mechanical-template story — no AI in the library's critical
path. Evidence format, fail-closed gates, preservation semantics, dirty-
tree refusal, thin-justification refusal, `[dev-dependencies]`
authority: all deterministic.

However, the story's recent history has been contaminated by story 14's
claude-as-component direction. The README.md footnote

  > "Story 7 shipped `healthy` at commit `e5f4997` but its
  > implementation was mutated by story 14's in-flight work; 5 of its
  > 9 tests currently fail. The YAML still reads `status: healthy`
  > pending a re-UAT pass that will correct it."

indicates that the `agentic-test-builder` crate currently has claude
code baked into it from story 14's implementation work, and that story
7's own test suite is red against that mutation.

**Why it violates.** (a) and (d) — the YAML is clean, but the on-disk
implementation that `agentic-test-build` now embodies no longer matches
the story 7 contract. This is a symptom of the story 14 leakage rather
than a story 7 authoring defect.

**Proposed correction.** Re-verify story 7 against its own YAML once
story 14 is retired/replaced. The story's contract (deterministic
scaffolds, fail-closed refusals, preservation rule, evidence row shape)
is correct and claude-as-user-compatible as written. The replacement
story that supersedes story 14 should either (i) leave story 7's
deterministic-scaffold contract intact and expand it with additional
red-state evidence options, or (ii) if the replacement removes scaffold
authoring from the library altogether, the story 7 guidance block
"Scaffold shape is deterministic from justification" needs a small
edit — but only AFTER the story 14 replacement is decided.

No direct edit of stories/7.yml is proposed from this audit; the needed
edits (if any) are downstream of the story 14 replacement decision.

---

## stories/1.yml: Sign a UAT verdict and promote the story to healthy (library + CLI)

**Status:** healthy.

**What's wrong.** Mostly clean — story 1's core framing is explicitly
claude-as-user. The guidance block "Orchestrator-driven UAT at the
binary boundary" (lines ~217-229) says:

  > "`agentic uat <id> --verdict <pass|fail>` is the orchestrator-
  > supplies-the-verdict path. The human (or a Claude Code session
  > acting as orchestrator) has already walked through the story's
  > prose UAT and decided the verdict; the CLI exists to sign that
  > decision."

That is textbook claude-as-user. Human or Claude Code wear the same
"user" hat.

There is ONE speculative footnote that hedges in the wrong direction
(lines ~226-229):

  > "A future story may add an agent-driven executor behind a different
  > flag (e.g. `--executor claude`); keep the `--verdict` flag
  > mandatory for now rather than defaulting it, so the operator's
  > decision is always explicit in the command line."

**Why it violates.** (d) — the speculation leaves the door open for a
future story to embed claude inside the `agentic-uat` library as an
executor, which would be the same category of mistake story 14 made
for `agentic-test-builder`. The `UatExecutor` trait block immediately
above it (lines ~204-215) already talks about "The real impl — an
agent-driven executor that drives a human or sub-agent through the
prose journey — is a later story."

**Proposed correction.** Prose edit. Two options:

1. Remove the `--executor claude` speculation entirely. The `--verdict`
   flag is the right long-term shape: the user (human or Claude Code)
   decides the verdict and the CLI signs it. There is no legitimate
   claude-as-executor story downstream.

2. Reword the hedge to make the claude-as-user stance explicit, e.g.:

   > "A future story may add a guided-UAT mode (e.g. `--guided`) that
   > walks a human (or Claude Code acting as orchestrator) through the
   > prose UAT more interactively — but the CLI continues to sign a
   > user-supplied verdict, not a claude-generated one. `--verdict` is
   > mandatory for now so the decision is always explicit."

Option (1) is cleaner; option (2) preserves the doc trail for anyone
grepping for executor shapes.

Suggested edit is to `guidance.UatExecutor is a trait, not a concrete
agent.` block AND `guidance.Orchestrator-driven UAT at the binary
boundary.` block — the two blocks together carry the full trait-plus-
speculation framing. Story 1's status is `healthy`, so any edit auto-
reverts it to `under_construction` and will require a re-UAT. That cost
is small; the speculation is the only place in the corpus outside
story 14 where the text invites claude-as-component.

---

## patterns/standalone-resilient-library.yml

**Status:** active; referenced by stories 1, 2, 5, 7, 10, 11, 14.

**What's wrong.** Nothing in the pattern itself is claude-as-component.
The pattern is clean. However, the pattern is being MISUSED by
story 14's guidance (quoted above) to justify embedding claude inside
`agentic-test-builder`. The pattern's `when_to_use` clause says:

  > "Apply when ALL of the following hold:
  > - The crate is on the critical 'prove the system still works'
  >   path (verify, uat, store-write, evidence-record). When the rest
  >   of the system is in flames, this crate must still produce a
  >   correct result.
  > - The functionality is meaningful as a library call (a single
  >   function or trait method that returns a typed result), not only
  >   as a pipeline.
  > - We can credibly assert 'no orchestrator dependency' without
  >   losing real capability — i.e. the library does not need the
  >   runtime to do its job."

Story 14 reads this as "because `agentic-test-builder` is on the
critical path, it must own its own claude subprocess shim rather than
depend on `agentic-runtime`." The correct reading is "because
`agentic-test-builder` is on the critical path, it must not depend on
claude at all — claude is outside the system." The pattern itself does
not say that, but it does not say the opposite either.

**Why it violates.** (d), marginally. The pattern's text is correct;
the gap is an unstated implication.

**Proposed correction.** Minor prose addition to
`standalone-resilient-library.yml`'s `how_we_do_it` or `trade_offs`
block, making the claude-as-user stance explicit:

  > "A standalone-resilient library does not spawn `claude` (or any
  > other LLM subprocess) in its own code. Claude is a user of the
  > system — the library's correctness when the rest of the system is
  > in flames depends on it being AI-free. LLM-driven work belongs in
  > agents (`agents/<category>/<name>/`) that invoke the CLI as users,
  > not in libraries the CLI wraps."

This addition ripples into every story that references the pattern (1,
2, 5, 7, 10, 11) — each will auto-revert to `under_construction` when
the pattern is edited (per `patterns/README.md`: "Editing a pattern
invalidates proof for every story that references it"). That cost is
acceptable per the user directive and closes the interpretive gap
story 14 exploited.

---

## patterns/fail-closed-on-dirty-tree.yml

**Status:** active; referenced by stories 1, 7, 11, 14.

**What's wrong.** Clean. The pattern describes an evidence-recording
gate (dirty-tree → typed error → exit 2) and has no claude surface.

**Proposed correction.** None.

---

## Adjacent artefacts flagged for user review (outside my jurisdiction)

These are not stories or patterns but contain claude-as-component
language that needs the user's attention once story 14 is retired.

### docs/decisions/0003-claude-code-subscription-subprocess.md

**What's wrong.** The ADR's Decision block reads:

  > "Drive Claude via subprocess. Wrap the `claude-code-rs` crate (or
  > fork if needs diverge) inside `agentic-runtime` behind a `Runtime`
  > trait. Day-one implementation: `ClaudeCodeRuntime` spawns `claude`
  > with `--output-format stream-json --verbose`..."

The scoping "inside `agentic-runtime`" is correct — claude belongs to
the ORCHESTRATOR layer, which is explicitly the thing that drives
subagents. The ADR is not itself claude-as-component leakage.

BUT: the ADR is silent on whether other product libraries (like
`agentic-test-builder`) are allowed to wrap claude too. Story 14 reads
the ADR as licensing any "runtime code" to spawn claude. The user
directive this session should be codified in ADR-0003 with an explicit
"Scope" paragraph:

  > "This ADR sanctions a single `ClaudeCodeRuntime` impl of the
  > `Runtime` trait inside `agentic-runtime` (the orchestrator layer).
  > It does NOT sanction wrapping `claude` inside product libraries
  > (`agentic-uat`, `agentic-ci-record`, `agentic-dashboard`,
  > `agentic-store`, `agentic-story`, `agentic-test-builder`, etc.).
  > Those libraries treat claude as an external user — the same
  > category as a human developer — who exercises the CLI. Embedding
  > an LLM subprocess inside a non-orchestrator product library turns
  > an unavailable/quota-exhausted claude into a system-wide failure
  > mode, which is the architectural mistake the legacy Python system
  > documented."

Also line 58-59 currently reads:

  > "Story 1's `verify_standalone_resilience.rs` test does NOT spawn
  > agents — that's deliberate. The verify path must work without any
  > runtime at all."

This is the correct stance applied to story 1, but it is not
generalized. Generalize the "verify path works without any runtime at
all" sentence to all product libraries, not just story 1.

**Severity.** Needs amendment. ADR is the user's jurisdiction; flagging
only.

### CLAUDE.md (line 58-60, "Core principles")

**What's wrong.** Core principles bullet reads:

  > "**Subscription auth.** Runtime code uses the local `claude`
  > binary (subscription auth) via subprocess. Never use raw
  > Anthropic API clients. See ADR-0003."

"Runtime code" is ambiguous. The intent (per ADR-0003) is
"`agentic-runtime` specifically, not other crates." Story 14 reads the
CLAUDE.md bullet as licensing any library to shell out to claude.

**Severity.** Durable instruction file; amendment is the user's call.
Suggested minimally-invasive edit:

  > "**Subscription auth.** `agentic-runtime` (the orchestrator) uses
  > the local `claude` binary (subscription auth) via subprocess to
  > spawn subagents. Never use raw Anthropic API clients. Product
  > libraries (`agentic-uat`, `agentic-test-builder`, etc.) do NOT
  > wrap `claude` themselves — claude is a user of the CLI, not a
  > component of the libraries. See ADR-0003."

### stories/README.md

**What's wrong.** Two concrete pieces need attention once story 14 is
retired:

- Line 65, the current-corpus table row for story 14:
  > "| 14 | test-builder authors real acceptance tests via the local claude subprocess | under_construction |"

  (Table currently says `under_construction`; the file itself says
  `healthy` — a separate data-integrity note, but both readings carry
  claude-as-component framing.)

- Lines 79-86, the narrative paragraph explaining why story 14 was
  needed:
  > "Story 14 is a hard prerequisite for the `dag-primary-lens` epic
  > (stories 10-13) picked up during story 10's implementation attempt:
  > the `agentic test-build` binary shipped by story 7 writes panic-
  > stub scaffolds that build-rust cannot drive to green, so every
  > proposed story in the epic needs story 14's real-acceptance-test
  > scaffolding to cross the red-green line. See `stories/14.yml` for
  > the full scope and the splitting analysis against story 7."

  This paragraph reads a legitimate ergonomics gap (panic-stubs are not
  acceptance tests) but reaches the wrong conclusion (embed claude in
  the library). The correct conclusion is the user (human or Claude
  Code acting as user) authors the test files using Edit/Write after
  `agentic test-build` records the red-by-absence evidence row.

**Severity.** `stories/README.md` is the user's curation; the
story-writer agent edits stories but the README narrative is
arguably outside. Flag for user review and update once story 14 is
replaced.

### epics/live/dag-primary-lens/epic.yml

Not audited in detail (epics are outside the stated stories+patterns
scope). But story 14's `depends_on` chain and the story-14 narrative
paragraph in `stories/README.md` imply the epic references story 14 as
a prerequisite. Worth grepping for "14" in the epic and updating once
story 14 is replaced.

---

## Clean-verdict section

Every file in `stories/*.yml` and `patterns/*.yml` was read in full for
this audit. The following are CLEAN under the claude-as-user rule (no
claude-as-component leakage):

- `stories/2.yml` — Record per-story test results to test_runs on every
  CI run. Clean; describes a library-level recorder with typed
  `Recorder::record` and no claude surface.
- `stories/3.yml` — Render the four-status story-health dashboard.
  Clean; purely a read-path story over `agentic-store`.
- `stories/4.yml` — Provide a Store trait with an in-memory
  implementation. Clean; pure type-level contract.
- `stories/5.yml` — Back the Store trait with an embedded SurrealDB
  implementation. Clean; durability + trait-parity.
- `stories/6.yml` — Load and validate stories/*.yml into typed Story
  values. Clean; parse/validate with typed errors. No claude.
- `stories/9.yml` — Scope dashboard staleness to each story's declared
  file dependencies. Clean; glob-match logic in `agentic-dashboard`.
- `stories/10.yml` — Render the story corpus as a DAG with
  frontier-of-work view and blast-radius drilldown. Clean; pure read-
  path DAG traversal + render.
- `stories/11.yml` — UAT refuses to sign Pass for a story standing on
  an unproven ancestor. Clean; the refusal rule is store-driven and
  claude-free.
- `stories/12.yml` — Scope `agentic stories test <selector>` runs to a
  DAG subtree. Clean; selector grammar + test-executor trait with a
  real cargo shim as the impl. No claude in the contract.
- `stories/13.yml` — Classify a story as unhealthy when any transitive
  ancestor is not healthy. Clean; classifier-rule change in
  `agentic-dashboard`.
- `patterns/standalone-resilient-library.yml` — itself clean. Flagged
  above for a stance-clarifying prose addition, not for a violation.
- `patterns/fail-closed-on-dirty-tree.yml` — clean.

---

## Summary

**Files checked:** 13 stories (1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13,
14) + 2 patterns (standalone-resilient-library, fail-closed-on-dirty-
tree) = 15 story/pattern files in scope. All were read in full.

**Severity breakdown:**

- **Retire (whole contract is claude-as-component):** 1
  - `stories/14.yml` (replacement draft already in flight per session
    directive — not re-proposed in this audit).

- **Re-verify once upstream fix lands:** 1
  - `stories/7.yml` (YAML clean; implementation contaminated by
    story 14's mutation; re-UAT after story 14 is replaced).

- **Prose edit:** 2
  - `stories/1.yml` — trim or reword the `--executor claude`
    speculation so claude-as-user stays explicit end-to-end.
  - `patterns/standalone-resilient-library.yml` — add an explicit
    "pattern does not sanction claude-wrapping inside the library"
    clause; ripples auto-revert to the 6 stories that reference it
    (expected and acceptable per the user directive).

- **Adjacent-file flags (outside my jurisdiction; user review):** 3
  - `docs/decisions/0003-claude-code-subscription-subprocess.md` —
    needs a Scope paragraph clarifying the ADR sanctions
    `agentic-runtime` only, not product libraries.
  - `CLAUDE.md` core-principles "Subscription auth" bullet — needs
    scope-narrowing so "runtime code" is unambiguous.
  - `stories/README.md` narrative paragraph on story 14 + the
    current-corpus table row for story 14 — update once story 14 is
    replaced.

- **Clean:** 11 files (stories 2, 3, 4, 5, 6, 9, 10, 11, 12, 13 +
  `patterns/fail-closed-on-dirty-tree.yml`).

**Headline.** The corpus is mostly clean. The leakage is concentrated
in story 14 (by design — the whole story is the mistake), with two
second-order issues (story 1's speculative future-flag footnote; the
standalone-resilient-library pattern's unstated implication that
story 14 exploited). The adjacent-file leakage in ADR-0003, CLAUDE.md,
and stories/README.md is milder but worth fixing to close the
interpretive loophole that let story 14 happen.

Breaking stories and redoing UATs is the expected cost per the user
directive. The prose edits proposed here will re-open stories 1, 2, 5,
7, 10, 11 (via the standalone-resilient-library pattern edit) and
story 1 directly — that is the right trade for closing the
architectural hole.
