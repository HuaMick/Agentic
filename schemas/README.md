# schemas/

JSON Schema definitions — the **single source of truth** for the shape of authored artifacts.

One file per schema:

- `story.schema.json` — user story YAML shape.
- `epic.schema.json` — epic YAML shape.
- `verdict.schema.json` — verdict record shape.
- `evidence.schema.json` — individual evidence entry shape.
- `manifest.schema.json` — agent manifest shape.
- `process.schema.json` — agent process shape.
- `inputs.schema.json` — agent inputs shape.
- `event.schema.json` — ledger event envelope shape.

## Why JSON Schema (not Rust-only types)

1. **Editor validation.** IDEs with JSON Schema support validate YAML against these files live.
2. **Cross-language.** Python tooling, linters, docs generators can consume the same schemas.
3. **Rust codegen.** The `typify` or `schemars` crate generates Rust types from schemas (or vice versa). Types and docs stay in sync.
4. **External authoring.** A user authoring a story YAML in their editor gets autocomplete + error squiggles.

## Rule

**If it's authored by humans and parsed by code, it has a schema here.** If it's purely internal Rust types, skip the schema — just use `serde`.
