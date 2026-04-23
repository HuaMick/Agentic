# story_tree research

Open research folder capturing the ideation that led to the
**Phase 0 / 0.5 / 1 ladder** for building Agentic into a
sandboxed story-hardening loop.

Named by the user ("story tree"): sandboxes are ephemeral branches
that compete, die, and coexist; the durable structure is the story
corpus.

This folder exists because the ideation spanned multiple sessions.
Every note is self-contained enough that a fresh Claude Code session
can pick up the thread without prior conversation loaded.

## If you are a fresh agent reading this cold

**Start at `NEXT-SESSION.md` and this README.** Both are current and
load-bearing. Then pick from the notes below based on what you need:

- If you need to know **what's decided and what to do next** — notes
  09, 10, 11, 13, plus `NEXT-SESSION.md`.
- If you need **context on how we got here** — notes 01, 02, 03, 05.
- If you need **external perspective** — note 12.
- If you need **historical reasoning** (superseded but still useful) —
  notes 04, 06, 07, 08 (each carries a status banner at the top
  pointing to the current state).

Authority sits above this folder. Read in this order if you need
ground truth:

1. `stories/*.yml` — current contracts
2. `CLAUDE.md` — current driving rules
3. `docs/decisions/*.md` — current ADRs
4. `README.md` (top of repo) — current project state
5. `schemas/*.json` — current shapes

The research folder is a sketchpad. If it disagrees with the
authoritative artefacts, the authoritative artefacts win.

## Notes, by role

### Current (load-bearing for next session)

- **`09-tier1-resolutions.md`** — **Phase 0 / 0.5 / 1 ladder**
  resolution. Docker-local first; Cloud Workstations rejected;
  sandbox signer identity; ancestor gate option (a); green
  criterion. User-ratified 2026-04-22.
- **`10-phase1-story-outlines.md`** — directive input for
  `story-writer`: outlines for Phase 0 stories 16–20 + Phase 0.5
  stories 21–22.
- **`11-sandbox-adr-outline.md`** — outline for **ADR-0006:
  Sandboxed story-hardening loop with reproducibility attestation**
  + ADR-0003 amendment. Names the story tree + reproducibility
  attestation as the research bet.
- **`12-external-patterns.md`** — skeptical April 2026 survey.
  Sandboxing is commoditising; story-tree + cold-rebuild attestation
  is the genuine research bet.
- **`13-existing-stories-impact.md`** — which of the 12 live
  healthy stories will need **amendments** (6), **touch-ups** (4),
  or **no change** (1) when the Phase 0 pipeline lands.
- **`NEXT-SESSION.md`** — the ordered action list for the next
  session. Start here.

### Context / framing (still valid, useful for fresh agents)

- **`01-goal.md`** — what we're trying to build, bounded. Innovation
  claim: the story tree, not the sandbox primitive.
- **`02-current-state.md`** — snapshot of the system today:
  healthy stories, crates, architecture, workflow pain points that
  motivated cloud / sandbox direction.
- **`03-tree-metaphor.md`** — the branches-die / compete / coexist
  thinking that motivates sandbox isolation + retirement.
- **`05-reproducibility.md`** — why reproducibility-from-spec-alone
  is the healthy-gate. Load-bearing. Read before touching Phase 0
  stories that will expect this principle to hold.

### Historical (superseded — status banners inside each)

- **`04-sandbox-model.md`** — original four-decision decomposition
  of "cloud sandbox." Still-useful framing; Cloud Workstations
  recommendation rejected. **See note 09 for current shape.**
- **`06-stack-bet.md`** — original GCP services mapping. Cost
  envelope and SurrealDB-on-GCE recommendation still valid; Cloud
  Workstations rejected. **See note 09 for current shape.**
- **`07-prerequisites.md`** — original P1–P6 prerequisite
  taxonomy. Superseded by the concrete Phase 0 story list in
  note 10. **See note 10 for current story list.**
- **`08-open-questions.md`** — original tier-1 / tier-2 / tier-3
  questions. Tier 1 resolved in note 09; tier 2 largely resolved
  by the Phase 0 shape; tier 3 still open for future sessions.

## Scope boundary

This folder is RESEARCH, not contracts. Nothing here is authoritative.
Stories, patterns, ADRs, schemas, and agent specs remain the only
authoritative artefacts. When this research crystallises, it
graduates into:

- ADR(s) under `docs/decisions/` (ADR-0006 is next)
- Stories under `stories/` (16–22 are next)
- Schema edits under `schemas/` (triggered by stories 17, 21)
- Epic(s) under `epics/live/` when the Phase 0 epic is declared

Everything here is the sketchpad that precedes those.

## Conventions

- Notes are markdown. No YAML. No schemas. No tests.
- File paths referenced inline use repo-relative paths.
- Cross-references use relative links within this folder.
- User voice is quoted with `"user:"` attribution; agent synthesis is
  plain prose.
- Open questions surface explicitly with `OPEN:` markers so future
  sessions can grep.
- Superseded notes carry a `⚠️ Status` banner at the top pointing to
  the current state. Body preserved for historical reasoning.

## State at time of writing

Research folder last revised 2026-04-22 after a multi-round ideation
session with the user. All live stories still `healthy`. Phase 0
shape ratified in-conversation, pending crystallisation into stories
and ADR. Note 13 (existing-stories impact) added 2026-04-23 during
cleanup pass.

Commit that created this folder: run `git log --oneline -1
docs/research/story_tree/README.md` to find it.
