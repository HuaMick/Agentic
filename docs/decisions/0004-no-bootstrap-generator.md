# ADR-0004: No bootstrap generator for `.claude/agents/*.md`

**Status:** accepted
**Date:** 2026-04-17

## Context

Claude Code looks for subagent definitions under `.claude/agents/*.md`. Our authoritative agent specifications live under `agents/<category>/<name>/` as three YAML files (`manifest.yml`, `process.yml`, `inputs.yml`). Something has to bridge the two locations — Claude Code reads `.claude/agents/*.md`, humans author YAML.

Two paths to bridge:

1. **Generator.** A tool (probably `cargo xtask gen-bootstraps`) reads the YAML and emits the markdown files. YAML stays authoritative; markdown is regenerated.
2. **Hand-written pointers.** The `.claude/agents/<name>.md` file is ~10 lines, hand-written, and says "read `agents/<cat>/<name>/process.yml` and follow it." No generation.

The legacy AgenticEngineering system maintained both forms by hand and they drifted — the markdown files grew to duplicate parts of the YAML spec, fell out of sync when the YAML changed, and the drift consumed engineering time for no user value.

Our initial proposal assumed the generator path. The user pushed back mid-session: "I just had an instruction in the .claude/agents/story-writer file to read the process.yml file as its only instruction before and that worked fine initially."

Cost/benefit:

- A generator is a piece of code we maintain. It needs tests. Its output format becomes a thing we version. It's a dependency of the agent-authoring loop.
- Hand-written pointers are ~10 lines, trivially reviewable, and impossible to drift meaningfully — there's almost nothing in them to drift against.

## Decision

**No generator.** `.claude/agents/<name>.md` files are hand-written, short (~10 lines each), and delegate to `agents/<category>/<name>/process.yml` as the authoritative specification. When an agent is added, a human author writes the pointer at the same time.

The `agentic-agent-defs` crate includes a `check_pointer_files()` function that sanity-checks the two locations agree on names and tool lists (flags missing or orphan pointers) — but it does **not** regenerate them.

## Alternatives considered

**Full generator** (`cargo xtask gen-bootstraps`). Rejected. Cost (code + tests + format-versioning + build-order complexity) outweighs benefit at this scale. If we had 100 agents and each needed a rich bootstrap, the math would flip — we don't.

**Keep markdown authoritative; drop the YAML.** Rejected. YAML gives us schema validation, programmatic loading by the runtime (to inject process.yml into prompts), and structured inputs definitions. Markdown is poorer for every automated consumer.

**Dual authoritative** (both YAML and markdown maintained by hand, neither generated). Rejected — this is exactly the legacy's failure mode.

**Render YAML → markdown live at agent-spawn time** (no file generation, compute on the fly). Rejected. Claude Code reads `.claude/agents/*.md` as files on disk. We don't control its loader.

## Consequences

**Gained:**

- No tool to build, test, or maintain.
- No build-order dependency (previously: "did you re-run `cargo xtask gen-bootstraps` before committing?" would have been a recurring trap).
- Pointer files stay trivially reviewable. A pull request that changes a pointer shows all ten lines of diff context.
- Drift surface is tiny — a pointer files says "name X, read path Y." Those two things almost never change.

**Given up:**

- The `.claude/agents/*.md` file has to be written when a new agent is authored. Small cost, and the author is already writing YAML in the neighbouring directory.
- If we ever want the pointer to contain richer metadata (tool access rules, model selection, permissions), that content is hand-authored rather than derived. Revisit this ADR if pointer complexity grows past ~20 lines.

## Related

- `agents/README.md`: "Claude Code pointer files" section explains the pattern for authors.
- `crates/agentic-agent-defs/README.md`: describes the `check_pointer_files()` sanity check (no generation, only validation).
- `.claude/agents/story-writer.md`: the first and current example of the pattern.
