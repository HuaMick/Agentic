# 10 — Phase 0 story outlines (revised)

Draft outlines for the Phase 0 stories implied by the revised decisions
in `09-tier1-resolutions.md`. **This note replaces the earlier
cloud-first four-story list** with a Phase-0-Docker-first list
informed by the 2026-04-22 ideation session.

Each outline is directive input for the `story-writer` curator agent —
not an authoritative story YAML. Story-writer will confirm scope,
locate file paths, write justifications in the corpus voice, wire
`depends_on` / `patterns`, validate against
`schemas/story.schema.json`, and commit the proposed story.

IDs below are tentative — next vacant ID is **16** (policy: never
reuse retired IDs 7, 8, 14). Stories are numbered in proposed order.

## Ordering — eventual consistency (revised 2026-04-23)

User direction: *"pay our costs upfront so they don't grow into
weeds; preference for a single UAT; if we need to break things and
they need to stay broken it's fine at this stage, we have no users
so we can lean into eventual consistency."*

**All Phase 0 stories (16–21) are proposed as a batch.** Amendments
to existing stories fire in parallel. Nothing is strictly sequenced
behind anything else. Stories can sit red across the corpus while
the pipeline converges.

```
Phase 0 batch (proposed together):
  16  runs observability       (no existing amendments)
  17  build_config schema      (triggers story 6 amend — bundled with 21)
  18  signer identity          (triggers stories 1, 2 amends)
  19  agentic-runtime unblock  (no existing amendments)
  20  agentic story build      (triggers stories 4, 5 amends for snapshot)
  21  retirement lifecycle     (triggers stories 3, 6, 11 amends
                                plus 9, 10, 13 touch-ups)

Phase 0.5:
  22  cloud-compatible Store   (additive; trait-parity tests from
                                stories 4+5 cover automatically)
```

**Story 21 moved from Phase 0.5 into Phase 0** so the two story-6
schema edits (build_config from 17, retired+superseded_by from 21)
bundle into a single story-6 amendment pass. One auto-revert, one
re-UAT, not two.

## Two cross-cutting prerequisites

Both apply to multiple stories; call out once:

- **Schema edits to `schemas/story.schema.json`.** At least two needed:
  add `build_config` object (story 17), add `retired` to the status
  enum + `superseded_by` field (story 21). Schema changes go through
  whichever curator owns schemas; story-writer flags the dependency,
  doesn't perform the edit unilaterally.
- **`agentic-runtime` un-deferred.** The `_deferred/agentic-runtime/`
  directory activates. ADR-0003's `Runtime` trait + `ClaudeCodeRuntime`
  impl ship. Story 19 covers this. Stories 20 onward depend on it.

---

## Story 16 — Run-trace persistence (observability prerequisite)

### Outcome

> Every invocation of the inner-loop executor (whether human-driven
> `agentic uat` or future `agentic story build`) writes a structured
> **run row** to the `runs` Store table — with run id, story id,
> story YAML snapshot, signer, build config, per-iteration summary,
> final outcome, and a pointer to the raw NDJSON trace blob — so
> failures are inspectable and replayable without re-execution.

### What shifts

1. **New Store table: `runs`**, append-only. Each row is structured
   JSON per the schema below.
2. **New runtime tap:** during any claude subprocess run, the NDJSON
   event stream is tee'd to a trace file (`runs/<id>/trace.ndjson`)
   and a per-iteration summary row is appended to the run.
3. **No claude-as-component shift.** The runtime (story 19's scope)
   owns the tap. Product libraries are unaffected.

### Draft run schema (descriptive, not exhaustive)

```
run_id:                string (uuid or short hash)
story_id:              int
story_yaml_snapshot:   string (SHA256 of the story YAML as it existed at launch)
signer:                string (e.g. "sandbox:claude-sonnet-4-6@run-<id>")
started_at, ended_at:  RFC3339 timestamps
build_config:          { max_inner_loop_iterations, models, ... }
outcome:               "green" | "inner_loop_exhausted" | "crashed"
iterations:            [ { i, started, ended, probes: [...], verdict?, error? } ]
trace_ndjson_path:     string (relative to the runs/ volume)
```

### Acceptance tests (draft)

| File | Justification (terse) |
|------|------------------------|
| `agentic-store/tests/runs_table_accepts_structured_run_row.rs` | A run row with all required fields round-trips through `Store::append` + `Store::query` unchanged. |
| `agentic-runtime/tests/claude_run_tees_ndjson_to_trace_path.rs` | A claude subprocess driven by the runtime emits an NDJSON file at the trace path with one line per event received from stdin. |
| `agentic-runtime/tests/run_row_records_outcome_on_iteration_exhaustion.rs` | Given a budget of N iterations where the agent never declares green, the run row ends with `outcome: inner_loop_exhausted` and exactly N iteration summaries. |
| `agentic-runtime/tests/run_row_records_outcome_on_crash.rs` | If the claude subprocess exits non-zero, the run row ends with `outcome: crashed` and an `error` field on the last iteration. |
| `agentic-runtime/tests/run_row_records_outcome_on_green.rs` | When the agent declares green (both `cargo test` green AND `agentic uat --verdict pass` exits 0), the run row ends with `outcome: green` and a pointer to the resulting signing row. |

### UAT walkthrough sketch

1. Author a minimal fixture story; start a driver that spawns claude
   through the new runtime tap with a 1-iteration budget and a prompt
   that exits immediately.
2. Inspect the `runs` table — a single row appears.
3. Inspect the trace file — non-empty NDJSON.
4. Repeat with a budget of 3 where claude doesn't green; verify the
   `inner_loop_exhausted` path.
5. Repeat with a budget where claude crashes; verify the `crashed`
   path.

### Dependencies

`depends_on: [4, 5]` (Store trait + SurrealStore). Also implicitly
coordinates with story 19 (agentic-runtime) — this story defines the
schema; story 19 does the emitting.

### Open questions for story-writer

- Should this story ship the schema AND the runtime emission, or split
  so the schema lands first and the emission lands with story 19? My
  lean: one story. The schema without an emitter is vaporware.
- Trace blob storage shape. Inside the Store as a big value, or
  filesystem-only with Store pointing at the path? My lean: filesystem
  (under `runs/<id>/trace.ndjson`), with the Store carrying only the
  path. Trace blobs get large fast.

---

## Story 17 — `build_config` field on the story schema

### Outcome

> A story author declaring `build_config: { max_inner_loop_iterations,
> models }` in the story YAML gets their budget and model selection
> respected by the runtime; omitting `build_config` falls back to
> documented defaults.

### What shifts

1. **Schema addition** to `schemas/story.schema.json`: a new optional
   `build_config` object with at least `max_inner_loop_iterations:
   int` and `models: [string]`.
2. **Story loader** (`agentic-story`) parses and validates the field.
3. **Defaults** live in documentation + a single Rust constant; e.g.
   `max_inner_loop_iterations: 5`, `models: []` (runtime picks one).
4. **No shipped behaviour change** — the field is read by story 19/20
   when they land. This story just introduces the field.

### Acceptance tests (draft)

| File | Justification (terse) |
|------|------------------------|
| `agentic-story/tests/build_config_loads_when_present.rs` | A story with a valid build_config round-trips through the loader. |
| `agentic-story/tests/build_config_optional_defaults_apply.rs` | A story omitting build_config loads cleanly; the defaults are documented and accessible via a helper. |
| `agentic-story/tests/build_config_rejects_negative_iterations.rs` | `max_inner_loop_iterations: 0` or negative is rejected at load. |
| `agentic-story/tests/build_config_empty_models_is_valid.rs` | An empty models array is valid; runtime-defaulting is the downstream concern. |

### Dependencies

`depends_on: [6]` (the story loader story).

### Open questions for story-writer

- Mandatory or optional? My lean: optional, with defaults. Mandatory
  forces backfill of stories 1–15.
- Where do defaults live? My lean: a single `DEFAULT_BUILD_CONFIG`
  constant in `agentic-story`, referenced by docs.

---

## Story 18 — Signer identity on runs + signings

### Outcome

> Every `uat_signings` row and every `runs` row carries a `signer:
> String` field, populated deterministically from a three-tier
> resolution chain — CLI flag → env var → git config `user.email` (for
> humans) or runtime-injected sandbox identity (for agents) — and
> stores that attribute so every verdict and every run is attributable.

### What shifts

1. **Store row shape** — every new `uat_signings` and `runs` row
   includes `signer: "<identity>"`.
2. **CLI contracts extend** — `agentic uat --signer` flag.
3. **Resolution chain** — `--signer` → `AGENTIC_SIGNER` → git config
   → typed error. Agent-runs in the sandbox pass `AGENTIC_SIGNER`
   with the convention `sandbox:<model>@<run_id>`.
4. **Dashboard drilldown** shows signer on verdict / run history.

### Convention for agent signers

```
signer: "sandbox:claude-sonnet-4-6@run-a1b2c3"
```

- `sandbox:` prefix marks non-human signer.
- `<model>` is the claude model / subprocess identity.
- `<run_id>` links back to the `runs/` row.

### Acceptance tests (draft)

| File | Justification (terse) |
|------|------------------------|
| `agentic-uat/tests/pass_verdict_writes_signer_from_git_config.rs` | Default resolution path works for humans. |
| `agentic-uat/tests/pass_verdict_signer_flag_overrides_git_config.rs` | Flag has precedence. |
| `agentic-uat/tests/signer_env_var_precedence.rs` | Env var has precedence over git config. |
| `agentic-uat/tests/pass_verdict_fails_when_no_signer_source.rs` | All three absent → `SignerMissing` + exit 2 + no write. |
| `agentic-runtime/tests/sandbox_signer_is_model_at_run_id.rs` | Runs launched via runtime carry `sandbox:<model>@<run_id>` in AGENTIC_SIGNER. |
| `agentic-uat/tests/existing_signings_without_signer_still_read.rs` | Pre-existing rows lacking signer are readable by the dashboard without panic. |

### Dependencies

`depends_on: [1, 16]` (extends story 1; uses run id schema from 16).

### Open questions for story-writer

- Validation on the signer string? My lean: reject empty /
  whitespace-only; accept everything else. Don't pin email format.
- Should `agentic-ci-record` test_runs also carry signer? My lean:
  yes, for symmetry. Story-writer decides bundle-vs-split.

---

## Story 19 — `agentic-runtime` un-deferred: Runtime trait + ClaudeCodeRuntime

### Outcome

> A `Runtime` trait in `agentic-runtime` exposes
> `spawn_claude_session(prompt, tools, budget) -> RunOutcome`, the
> `ClaudeCodeRuntime` impl wraps the local `claude` CLI per ADR-0003,
> and the runtime captures the NDJSON event stream into a tee'd trace
> file and a structured run row — so any downstream consumer (story
> 20's `agentic story build`, future orchestrators) has a single
> typed surface for "spawn an agent and get a run."

### What shifts

1. **Un-defer** `_deferred/agentic-runtime/` into `crates/agentic-runtime/`.
2. **`Runtime` trait** — `spawn_claude_session(...)` with a `RunConfig`
   (models, budget, tools, trace dir, signer).
3. **`ClaudeCodeRuntime` impl** spawns `claude -p --output-format
   stream-json --verbose`, tees NDJSON to trace file, writes run row
   to Store, enforces iteration budget.
4. **Budget enforcement** — reads `max_inner_loop_iterations` from
   the injected `RunConfig`; counts "iterations" as
   claude-tool-use-and-observe turns (story-writer nails down the
   exact counting convention).
5. **Error model** — `RuntimeError::ClaudeSpawn`, `RuntimeError::
   BudgetExhausted`, `RuntimeError::TraceWrite`, `RuntimeError::
   StoreWrite`, all typed, all non-panic.

### Acceptance tests (draft)

| File | Justification (terse) |
|------|------------------------|
| `agentic-runtime/tests/claude_runtime_spawns_and_captures_trace.rs` | A minimal `spawn_claude_session` call produces a non-empty trace file at the configured path. |
| `agentic-runtime/tests/runtime_writes_run_row_on_exit.rs` | After the session ends, a `runs` row exists in the Store with the expected structure. |
| `agentic-runtime/tests/runtime_enforces_iteration_budget.rs` | A session with budget=2 stops claude after 2 iterations and records `outcome: inner_loop_exhausted`. |
| `agentic-runtime/tests/runtime_typed_error_on_missing_claude_binary.rs` | `ClaudeCodeRuntime::new` on a system with no `claude` in PATH returns a typed error, not a panic. |
| `agentic-runtime/tests/runtime_never_uses_bare_flag.rs` | The argv to `claude` never contains `--bare`. |

### Dependencies

`depends_on: [16]` (runs schema). Story 16 and 19 may legitimately
merge if story-writer prefers one unit.

### Open questions for story-writer

- Runtime trait object-safe? My lean: yes, `dyn Runtime` behind
  `Arc<>`, symmetrical with the `Store` trait.
- Where does `RunConfig` live — `agentic-runtime` or a shared types
  crate? My lean: `agentic-runtime` with re-exports. Shared types
  crate is premature.
- How do we test against `claude` in CI without real claude? My lean:
  `MockRuntime` that emits canned NDJSON from a fixture file, plus a
  small set of `#[ignore]` integration tests that run against real
  claude locally.

---

## Story 20 — `agentic story build <id>`: the Phase 0 deliverable

### Outcome

> A developer running `agentic story build <id>` on their laptop
> launches a Docker container — with the `agentic` binary, all agent
> specs, guidance, schemas, and ADRs baked in; the specific story
> mounted at `/work/story.yml`; claude credentials mounted read-only
> — and receives, on exit, a mounted `runs/<id>/` directory containing
> a structured run row and NDJSON trace that together either attest
> the story as green or show where the inner loop stopped.

### What shifts

1. **New CLI subcommand** `agentic story build <id>` in
   `agentic-cli`. Host-side: validates inputs, composes `docker run`
   args, launches.
2. **In-container entrypoint** `agentic story build --in-sandbox <id>`:
   initialises embedded Store, seeds ancestor signings from a mounted
   snapshot, creates the run row via `agentic-runtime`, spawns the
   inner-loop agent.
3. **New primitive: Store snapshot / restore.** Host extracts
   ancestor-closure signings from the user's Store into a small JSON
   bundle; container imports it into its embedded Store before the
   inner loop begins. Preserves story 11's ancestor gate semantics
   inside the sandbox.
4. **Dockerfile** at `infra/sandbox/Dockerfile`. Baked in: `agentic`
   binary, `agents/`, `patterns/`, `schemas/`, `docs/decisions/`,
   `docs/guides/`, `CLAUDE.md`, `README.md`, Rust toolchain, `claude`
   CLI. Mounted in: story YAML, `~/.claude/.credentials.json`,
   ancestor snapshot, `runs/` volume.
5. **Green criterion enforcement** — the inner loop reports green only
   when both `cargo test --workspace` passes AND `agentic uat <id>
   --verdict pass` exits 0.

### Acceptance tests (draft)

| File | Justification (terse) |
|------|------------------------|
| `agentic-cli/tests/story_build_host_composes_expected_docker_run.rs` | The host command composes the expected `docker run` argv (image tag, mounts, env vars) given a fixture story + run config. |
| `agentic-cli/tests/story_build_host_fails_cleanly_when_docker_absent.rs` | Missing `docker` on PATH → typed error, exit 2, no partial state. |
| `agentic-story-build/tests/in_sandbox_seeds_ancestor_signings_into_embedded_store.rs` | Given a mounted ancestor snapshot, the pre-inner-loop step writes the expected rows into the embedded Store. |
| `agentic-story-build/tests/in_sandbox_green_run_writes_signing_and_run_row.rs` | A green fixture-run writes both a `uat_signings` row and a `runs` row with `outcome: green` and signer `"sandbox:<model>@<run_id>"`. |
| `agentic-story-build/tests/in_sandbox_exhausted_run_writes_run_row_no_signing.rs` | An exhausted-inner-loop run writes a `runs` row with `outcome: inner_loop_exhausted` and NO `uat_signings` row. |
| `agentic-store/tests/snapshot_export_contains_expected_ancestor_closure.rs` | `Store::snapshot_for_story(id)` returns the transitive ancestor signings of the given story id; non-ancestors excluded. |

### UAT walkthrough sketch

1. On host, run `agentic story build 16` against a fixture proposed
   story with a 3-iteration budget and a trivial acceptance test.
2. Container launches, inner loop executes, claude iterates until the
   test greens, `agentic uat` signs, container exits.
3. Inspect `runs/<id>/run.json` — shows `outcome: green`, iteration
   trace, signer `"sandbox:<model>@<run_id>"`, pointer to signing row.
4. Re-run against a story that the agent can't complete in 3
   iterations; verify `outcome: inner_loop_exhausted` and no signing.
5. Re-run against a story whose ancestor is unhealthy; verify the
   ancestor gate fires inside the sandbox and `agentic uat` refuses.

### Dependencies

`depends_on: [11, 15, 16, 17, 18, 19]`. This is the crown story of
Phase 0; it integrates everything.

### Open questions for story-writer

- Does this live in `agentic-cli` entirely, or a new crate
  `agentic-story-build`? My lean: new crate, with `agentic-cli` as a
  thin subcommand wrapper. Keeps `agentic-cli` slim and the
  story-build logic testable.
- Snapshot primitive — Store trait extension or a separate
  `agentic-store-snapshot` helper crate? My lean: Store trait
  extension (`snapshot_for_story(id) -> Snapshot` +
  `restore(snapshot)`), because MemStore also benefits in tests.
- Docker image tagging — per-commit SHA or semver? My lean: per-commit
  SHA for the reproducibility receipt, aliased to `latest` for
  convenience.

---

## Story 21 — Retirement lifecycle (Phase 0, bundled with schema edits)

(Unchanged in shape from the earlier note 10 outline for story 16.
Renumbered to 21 here. Not repeating the full content — story-writer
refers to the original description alongside the schema-edit dependency.)

**Key points:**
- `status` enum adds `"retired"`.
- Optional `superseded_by: <id>` field.
- Ancestor gate skips retired ancestors.
- Dashboard default hides retired; `--canopy` shows them.
- Retroactive backfill of hard-deleted 7/8/14.

**Revised to Phase 0** (2026-04-23): while Phase 0's inner loop
doesn't strictly require retirement to function, bundling the
schema edit with story 17's schema edit means story 6 amends once
and re-UATs once. User preference ("single UAT").

---

## Story 22 — Cloud-compatible Store impl (Phase 0.5 prerequisite)

(Unchanged from the earlier note 10 outline for story 19, renumbered
to 22.)

**Key points:**
- New `CloudSurrealStore` impl alongside `SurrealStore`.
- `AGENTIC_STORE_URL` selects between local embedded (default) and
  cloud.
- Trait parity proven via story 4 / story 5 tests re-run against the
  cloud impl.
- Also implements the snapshot/restore primitive story 20 introduces.

---

## What this note is NOT

- **Not a substitute for story-writer's judgement.** Curator may merge,
  split, or decline.
- **Not a schema change plan** — schemas edits go through whichever
  curator owns schemas.
- **Not a test scaffold** — `test-builder` authors the failing tests
  once each story is `proposed`.
- **Not an infra plan** — Terraform / image CI builds / GCP project
  setup live in parallel under `infra/` (see
  `11-sandbox-adr-outline.md`).

## What the next session does with this

1. (Probably in-session with the user) — final confirmation of the
   six-story Phase 0 shape, the ordering, and whether 21/22 slide
   cleanly to Phase 0.5.
2. Invoke `story-writer` with this note as directive input. Propose
   stories 16–20 first.
3. `test-builder` per story in sequence.
4. `build-rust` implements each.
5. `test-uat` walks UAT and promotes.
6. After story 20 lands, Phase 0 is done. Start 21 + 22 for Phase 0.5.

In parallel with story authoring, the sandbox infra track (Dockerfile,
CI image build, docker-compose for Phase 0.5) runs on its own lane —
see `11-sandbox-adr-outline.md`.
