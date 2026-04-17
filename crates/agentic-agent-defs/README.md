# agentic-agent-defs

## What this crate is

Loads and validates YAML agent definitions under `agents/`. Handles:

- Parsing `manifest.yml`, `process.yml`, `inputs.yml` for each agent.
- Resolving transitive input layers (shared inputs like `planner-shared.yml`).
- Validating against JSON Schema (strict — legacy's manifest drift was its #1 friction).
- Confirming the existence of the hand-written `.claude/agents/<agent>.md` pointer file that delegates to this agent's YAML.

Note: we intentionally do NOT generate `.claude/agents/*.md` files. Those pointers are hand-written ten-liners; a round-trip generator adds maintenance surface without preventing meaningful drift. See `agents/README.md` and `CLAUDE.md` for the rationale.

## Why it's a separate crate

Agent definitions are authored content — the product of the system. The loading and validation logic is substantial (transitive layers, fragment references, schema enforcement) and is reused by multiple callers: the runtime (to inject process.yml into prompts), the CLI (`agentic agent list/validate`), and the pointer-file sanity check.

Separating "what an agent IS" (definition, this crate) from "how we run one" (`agentic-runtime`) keeps each concern testable in isolation.

## Public API sketch

```rust
pub struct AgentDef {
    pub manifest: Manifest,
    pub process: Process,
    pub inputs: ResolvedInputs,   // transitive layers flattened
}

pub struct Registry {
    agents: HashMap<AgentName, AgentDef>,
}

impl Registry {
    pub fn load_from_dir(path: &Path) -> Result<Self, LoadError>;
    pub fn get(&self, name: &AgentName) -> Option<&AgentDef>;
    pub fn validate_all(&self) -> Vec<ValidationError>;
}

pub fn check_pointer_files(registry: &Registry, claude_agents_dir: &Path) -> Vec<PointerIssue>;
```

## Dependencies

- Depends on: `serde_yaml`, `jsonschema`, `agentic-events`
- Depended on by: `agentic-runtime`, `agentic-orchestrator`, `agentic-cli`, xtask

## Design decisions

- **Schema validation is strict.** Unknown fields are errors, not warnings. Legacy drifted because manifests grew ad-hoc fields; we reject that by default.
- **Transitive layers are flattened at load time.** Consumers see the resolved view, not the raw YAML chain.
- **Fragment references (`file.yml#path.to.key`) validated at load time.** Broken references fail fast, not at runtime.
- **No generator.** YAML is authoritative. `.claude/agents/*.md` is hand-written and short (~10 lines per file). Drift is kept in check by human review, not a generator; the payoff of a generator is too small at this scale to justify its own maintenance.
- **Pointer files are sanity-checked, not generated.** `check_pointer_files()` reports missing, stale, or orphan `.claude/agents/*.md` entries so authors catch drift in CI — without a generator taking over hand-authored text.

## Open questions

- Do we support YAML includes (`!include`) or stick with explicit layer references?
- Schema evolution: how do we version the manifest schema without breaking old agents?
- What exactly counts as a "stale" pointer file (description drift is fine; name mismatch is not — where's the line)?

## Stress/verify requirements

- Every checked-in agent YAML loads and validates.
- Pointer-file check flags every missing or orphaned `.claude/agents/*.md` entry.
- A malformed YAML produces an error identifying the exact file and line.
