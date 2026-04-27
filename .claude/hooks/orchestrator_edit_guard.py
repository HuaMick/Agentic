#!/usr/bin/env python3
"""PreToolUse hook denying orchestrator edits to subagent-owned surfaces.

Fires only when the calling context has no `agent_type` / `agent_id`
(i.e., the orchestrating Claude session). Subagents — test-builder,
build-rust, story-writer, guidance-writer, etc. — fall through with
`allow`; their own spec contracts handle their internal boundaries.

Denies `Edit` and `Write` from the orchestrator on:

  - scripts/verify/**       (test-builder owns; contract.yml `owns`)
  - crates/*/tests/**       (test-builder owns; ADR-0005)
  - evidence/runs/**        (test-builder owns; ADR-0005)

Does NOT inspect `Bash` commands. `sed -i` / `tee` / `>` workarounds
on these paths remain spec-only enforced via
`agents/orchestration/session-orchestrator/process.yml`'s
`route-to-the-owner` rule. Adding Bash inspection here would require
parsing arbitrary shell strings and is fragile; the better corrective
when an orchestrator reaches for a Bash workaround is a shape change
(spawn the owning subagent), not a tighter regex.

Background: triggered by commit 3f4568d, where the orchestrator
edited `scripts/verify/asset_consumer_minimum.sh` directly with a
self-acknowledged "should have been routed through test-builder"
note. Spec-only enforcement proved insufficient.

Decision is communicated via JSON on stdout per the Claude Code
hook contract: `permissionDecision` (`allow` or `deny`) plus
`permissionDecisionReason` on deny. Exit 0 always.
"""

from __future__ import annotations

import fnmatch
import json
import sys

# (glob, owning agent) — orchestrator may not Edit/Write these paths.
DENIED_GLOBS_FOR_ORCHESTRATOR: list[tuple[str, str]] = [
    (
        "scripts/verify/**",
        "test-builder (contract.yml `owns`: scripts/verify/**)",
    ),
    (
        "crates/*/tests/**",
        "test-builder (ADR-0005)",
    ),
    (
        "evidence/runs/**",
        "test-builder (ADR-0005)",
    ),
]


def allow() -> dict:
    return {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
        }
    }


def deny(reason: str) -> dict:
    return {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }


def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except json.JSONDecodeError as exc:
        # Fail open — if we can't parse, defer to harness.
        sys.stderr.write(
            f"orchestrator-edit-guard: malformed hook input ({exc}); "
            f"falling through.\n"
        )
        return 0

    # Subagents carry an agent_type / agent_id; the orchestrator does not.
    # Any non-empty subagent indicator means "not orchestrator" — allow.
    agent_type = payload.get("agent_type") or ""
    agent_id = payload.get("agent_id") or ""
    if agent_type or agent_id:
        print(json.dumps(allow()))
        return 0

    tool_name = payload.get("tool_name", "")
    if tool_name not in ("Edit", "Write"):
        print(json.dumps(allow()))
        return 0

    file_path = (payload.get("tool_input") or {}).get("file_path", "")
    if not file_path:
        print(json.dumps(allow()))
        return 0

    # Reduce path to repo-relative form (mirrors build_rust_guard.py's logic).
    rel = file_path.replace("\\", "/")
    marker = "Agentic/"
    idx = rel.rfind(marker)
    if idx >= 0:
        rel = rel[idx + len(marker):]
    rel = rel.lstrip("/")

    for glob, owner in DENIED_GLOBS_FOR_ORCHESTRATOR:
        if fnmatch.fnmatch(rel, glob):
            print(json.dumps(deny(
                f"orchestrator-edit-guard: orchestrator session may not "
                f"{tool_name} `{rel}` — this surface is owned by {owner}. "
                f"Route the change through that subagent (e.g. spawn "
                f"test-builder with the brief; do not edit the file "
                f"directly). See agents/orchestration/session-orchestrator/"
                f"process.yml `route-to-the-owner` rule. Triggered by the "
                f"lesson of commit 3f4568d, where this exact pattern "
                f"caused a self-acknowledged contract violation."
            )))
            return 0

    print(json.dumps(allow()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
