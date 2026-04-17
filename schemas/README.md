# schemas/

JSON Schema definitions — the **single source of truth** for the shape of authored artifacts.

One file per schema. If it's authored by humans and parsed by code, it has a schema here. Purely internal Rust types skip the schema and use `serde` directly.

## Current schemas

- **`story.schema.json`** — user story YAML shape. Authoritative for `stories/<id>.yml`. Authoring guide: `docs/guides/story-authoring.md`.
- **`pattern.schema.json`** — pattern YAML shape. Authoritative for `patterns/<slug>.yml`. Authoring guide: `docs/guides/pattern-authoring.md`.

## Schemas to be added (Phase 2+)

As the system grows, each new authored artifact gets a schema here. Expected next:

- `epic.schema.json` — when epics get structured beyond a folder name + `epic.md`.
- `manifest.schema.json`, `process.schema.json`, `inputs.schema.json` — when the `agentic-agent-defs` crate formalizes the agent YAML shapes (currently informal convention).
- `verdict.schema.json`, `evidence.schema.json` — when `agentic-verify` ships and the evidence JSONL records need a schema.
- `event.schema.json` — when the future event ledger is built.

## Why JSON Schema (not Rust-only types)

1. **Editor validation.** IDEs with JSON Schema support validate YAML against these files live.
2. **Cross-language.** Python tooling, linters, docs generators can consume the same schemas.
3. **Rust codegen.** The `typify` or `schemars` crate generates Rust types from schemas (or vice versa). Types and docs stay in sync.
4. **External authoring.** A user authoring YAML in their editor gets autocomplete and error squiggles.

## Rule

**If it's authored by humans and parsed by code, it has a schema here.** Speculative schemas (for artifacts not yet authored) are not added in advance — they land when the artifact they describe is first created.
