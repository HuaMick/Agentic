# story_tree research

Open research folder capturing the ideation that led to the next major
architectural direction: a **cloud-backed story tree** where one human
orchestrates N agents who build inside isolated cloud sandboxes.

This folder exists because the ideation is bigger than a single prompt.
Every note below is self-contained enough that a fresh Claude Code
session can pick up the thread without having the prior conversation
loaded.

## Why "story tree"

Named by the user. The phrase captures:

1. The existing story DAG (stories live, link via `depends_on`, form a
   graph).
2. The tree metaphor we're extending toward: branches can die, compete,
   and coexist (see `03-tree-metaphor.md`).
3. Each branch of the tree = a sandbox where an agent (or human) can
   experiment without touching the main trunk.

## Reading order for the next session

Read in order. Each note points at files in the repo the reader should
open alongside.

1. **`01-goal.md`** — what we're trying to build, bounded clearly.
2. **`02-current-state.md`** — snapshot of the system today (healthy
   stories, architecture, what's ready).
3. **`03-tree-metaphor.md`** — the branches-as-biological-branches
   thinking that motivates sandbox isolation.
4. **`04-sandbox-model.md`** — what "cloud sandbox" means concretely,
   decomposed into four underlying decisions.
5. **`05-reproducibility.md`** — the deeper principle the user is
   chasing: an agent should be able to build a story from the story
   alone. Why this is load-bearing, and why bigger-budget teams don't
   seem to be doing it.
6. **`06-stack-bet.md`** — GCP + Gemma + cost-controlled shape.
7. **`07-prerequisites.md`** — what needs to land locally before cloud.
8. **`08-open-questions.md`** — decisions the next session must make.
9. **`NEXT-SESSION.md`** — the ordered action list.

## Scope boundary

This folder is RESEARCH, not contracts. Nothing here is authoritative.
Stories, patterns, ADRs, and agent specs remain the only authoritative
artefacts. When this research crystallises, it graduates into:

- ADR(s) under `docs/decisions/`
- Stories under `stories/`
- Epic(s) under `epics/live/`

Everything here is the sketchpad that precedes those.

## Conventions

- Notes are markdown. No YAML. No schemas. No tests against them.
- File paths referenced inline use repo-relative paths.
- Cross-references use relative links within this folder.
- The user's voice is quoted with attribution ("user:"); the agent's
  synthesis is plain prose.
- Open questions are surfaced explicitly with `OPEN:` markers so next
  session can grep.

## State at time of writing

Commit at time this folder was created: run `git log --oneline -1
docs/research/story_tree/README.md` to find it. All 13 stories healthy.
`dag-primary-lens` epic complete. System is in the cleanest state it
has been.
