# `.claude/hooks/`

Project-local Claude Code hook scripts. Wired in `.claude/settings.json`.

## `build_rust_guard.py`

A `PreToolUse` hook that programmatically enforces three of build-rust's
contractual boundaries that the agent's spec already documents clearly.
Per the philosophy in CLAUDE.md and the cluster-C audit: if the rule is
already unambiguous in the YAML and the agent violates it anyway, the
right fix is enforcement, not more guidance bullets.

### What it enforces

The hook only fires when the calling subagent is `build-rust` (the
`agent_type` / `agent_id` field on the hook input). Other subagents and
the orchestrating session fall through with `allow`.

| Tool         | Rule                                                                                                                   | Source of truth                                       |
| ------------ | ---------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| `Bash`       | `cargo fmt` is rejected outright. The spec says use `rustfmt <path>`, not `cargo fmt`.                                 | `agents/build/build-rust/inputs.yml` (`cargo-fmt`)    |
| `Bash`       | All other commands must match an explicit allowlist (cargo test/build/check/clippy, rustfmt, read-only/commit-only git, ls/pwd/echo). Anything else (`git push`, `git reset`, `cargo install`, `rm`, …) is rejected. Allowlist in the script's `ALLOWED_PREFIXES`. | Spec contract |
| `Write`      | Writes under `crates/*/tests/**` are rejected unconditionally.                                                          | ADR-0005, contract.yml `does_not_touch`               |
| `Edit`       | Edits under `crates/*/tests/**` are rejected unconditionally.                                                          | ADR-0005                                              |
| `Write`      | Writes anywhere under `stories/**` are rejected (fixture-pollution defence). The single permitted story-file change is the status flip — that's an `Edit` on an existing file. | contract.yml `does_not_touch: stories/**`             |

### What it deliberately does NOT enforce

- The "status flip before crates edit" rule (build-rust forgetting to
  flip `proposed` → `under_construction`). A clean programmatic version
  needs more state than a `PreToolUse` hook has — the right home is a
  future `agentic stories audit` CLI command that detects the drift
  on demand. See the cluster-C audit summary.
- Test-build record's diagnostic classification (story 24). That's a
  code change to `agentic-test-builder`, not a hook.

### How decisions reach Claude

The script writes a `permissionDecision` JSON to stdout (`allow` or
`deny`) per the Claude Code hook contract. On `deny`, the
`permissionDecisionReason` cites the spec section the agent should
re-read so the feedback is actionable.

### Testing

`test_build_rust_guard.sh` exercises 25 cases — every allowlisted
prefix, every documented violation, and the non-build-rust fall-through
paths. Run before changing the script:

```
bash .claude/hooks/test_build_rust_guard.sh
```

Add a case for any new allowlist entry or new denied path.

### Cross-platform

Scripts are invoked as `python3 $CLAUDE_PROJECT_DIR/.claude/hooks/<file>.py`
from `.claude/settings.json`. Python 3 is required on the host running
Claude Code. `shlex` and `fnmatch` are stdlib — no third-party deps.

## `orchestrator_edit_guard.py`

A `PreToolUse` hook that programmatically enforces the
`route-to-the-owner` rule from
`agents/orchestration/session-orchestrator/process.yml`. The
orchestrator's spec says it must NOT edit subagent-owned surfaces
directly — every byte to those paths routes through the owning
agent. Spec-only enforcement proved insufficient (commit 3f4568d:
the orchestrator edited `scripts/verify/asset_consumer_minimum.sh`
directly, with a self-acknowledged "should have been routed
through test-builder" note in the commit message).

### What it enforces

The hook fires only when the calling context has no `agent_type` /
`agent_id` (i.e., the orchestrating Claude session). Any subagent
falls through with `allow` — their own contracts handle their
internal boundaries.

| Tool         | Path glob             | Owning agent                                    |
| ------------ | --------------------- | ----------------------------------------------- |
| `Edit`/`Write` | `scripts/verify/**`   | test-builder (`contract.yml owns: scripts/verify/**`) |
| `Edit`/`Write` | `crates/*/tests/**`   | test-builder (ADR-0005)                         |
| `Edit`/`Write` | `evidence/runs/**`    | test-builder (ADR-0005)                         |

### What it deliberately does NOT enforce

- **`Bash` workarounds** (`sed -i`, `tee`, `>`, `>>`) on the same
  paths. Adding Bash inspection requires parsing arbitrary shell
  strings and is fragile; the better fix when an orchestrator reaches
  for a Bash workaround is a shape change (spawn the owning subagent),
  not a tighter regex. The session-orchestrator spec carries that
  contract.
- `stories/**` writes from the orchestrator. Build-rust's hook
  already permits the single-line status-flip on `stories/*.yml`
  for build-rust; story-writer owns broader writes. If orchestrator
  edits to `stories/**` become a recurring drift, add a third
  glob entry here.

### How decisions reach Claude

Same as `build_rust_guard.py`: stdout JSON with
`permissionDecision` + `permissionDecisionReason` on deny. Exit 0
always.

### Testing

Manual smoke test (no automated test script yet — add
`test_orchestrator_edit_guard.sh` if/when the hook accumulates
denied-glob complexity):

```
echo '{"tool_name": "Edit", "tool_input": {"file_path": "scripts/verify/foo.sh"}}' \
  | python3 .claude/hooks/orchestrator_edit_guard.py
# Expected: deny payload naming test-builder as the owner

echo '{"tool_name": "Edit", "tool_input": {"file_path": "crates/agentic-test-support/src/lib.rs"}}' \
  | python3 .claude/hooks/orchestrator_edit_guard.py
# Expected: allow (src/, not tests/)

echo '{"agent_type": "test-builder", "tool_name": "Write", "tool_input": {"file_path": "scripts/verify/foo.sh"}}' \
  | python3 .claude/hooks/orchestrator_edit_guard.py
# Expected: allow (subagent fall-through)
```
