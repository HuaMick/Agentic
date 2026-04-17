# modules/

External code referenced by this repo as git submodules. Not first-party — read-only reference material.

## Layout

- `legacy/AgenticEngineering/` — the Python predecessor system. Reference only. Do not edit, do not port code directly. Lessons extracted from it live in `docs/decisions/` (ADRs) and per-crate READMEs; the submodule is kept so we can cross-reference when a design question comes up.

## Adding a new submodule here

Only add if the code genuinely belongs external (another repo's canonical home). Internal infrastructure goes under `crates/` or `scripts/`, not here.

Command form for reference:

```
git submodule add <url> modules/<name>
```
