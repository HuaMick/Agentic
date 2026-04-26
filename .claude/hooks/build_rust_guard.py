#!/usr/bin/env python3
"""PreToolUse hook enforcing build-rust's contract programmatically.

Reads Claude Code's hook JSON on stdin, and:

  - For `Bash`: tokenizes the command and rejects anything outside an
    explicit allowlist (Option B from the hook design).
  - For `Edit`/`Write`: rejects writes to `crates/*/tests/**` and to
    `stories/**` (with one carve-out: a single-line status-field edit
    on `stories/<id>.yml` is the one permitted story-file write).

Only enforces when the calling subagent is `build-rust`. Hook invocations
from the orchestrator session (no `agent_type`) or from other subagents
fall through with `allow`.

Decision is communicated via JSON on stdout with `permissionDecision`
("allow" or "deny") plus `permissionDecisionReason`. Exit 0 always; the
JSON carries the verdict.
"""

from __future__ import annotations

import fnmatch
import json
import shlex
import sys

# ---------------------------------------------------------------------------
# Allowlist for build-rust's Bash tool
# ---------------------------------------------------------------------------
#
# Each entry is a tuple of literal argv tokens that must match the start of a
# command segment. `cargo` requires both tokens (`("cargo", "test")`), so
# `cargo fmt` and `cargo install` are unmatched and rejected. `git` only
# permits read-only and commit-only verbs; `git push`, `git reset`,
# `git checkout`, `git rebase`, `git pull` are intentionally absent — those
# are orchestrator authority, not build-rust's.
ALLOWED_PREFIXES: list[tuple[str, ...]] = [
    # Rust toolchain
    ("cargo", "test"),
    ("cargo", "build"),
    ("cargo", "check"),
    ("cargo", "clippy"),
    ("rustfmt",),
    # Read-only inspection
    ("ls",),
    ("pwd",),
    ("echo",),
    # Git — read-only and commit-only verbs
    ("git", "status"),
    ("git", "diff"),
    ("git", "log"),
    ("git", "show"),
    ("git", "rev-parse"),
    ("git", "branch"),
    ("git", "add"),
    ("git", "commit"),
    ("git", "stash"),
    # WSL wrapper used by the orchestrator's sub-shell pattern; the inner
    # command after `bash -c "<cmd>"` is re-tokenized and re-checked below.
    ("wsl", "bash"),
]

# Tokens that may appear as a prefix to an otherwise-allowed command without
# changing the verdict. `cd <dir> && cargo test` is benign; the cd is just
# directory navigation.
PREFIX_PASSTHROUGH = {"cd"}

# ---------------------------------------------------------------------------
# Path globs for Edit/Write — build-rust may not write here.
# ---------------------------------------------------------------------------
DENIED_WRITE_GLOBS = [
    "crates/*/tests/**",  # ADR-0005: test-builder's authority
    "scripts/verify/**",
]

# `stories/**` is denied EXCEPT for the single-line status edit on an
# existing `stories/<int>.yml`. The Edit tool's diff size is the proxy for
# "single-line edit" — we approve Edit (which presumes the file exists), but
# unconditionally deny Write (which presumes new file or full rewrite).
STORIES_GLOB = "stories/*.yml"


# ---------------------------------------------------------------------------
# Decision helpers
# ---------------------------------------------------------------------------
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


# ---------------------------------------------------------------------------
# Bash command parsing
# ---------------------------------------------------------------------------
def split_into_segments(tokens: list[str]) -> list[list[str]]:
    """Split a tokenised command on shell separators (`&&`, `||`, `;`, `|`).

    Each returned segment is itself a list of tokens. Pipes and chains all
    become independent segments — every one must satisfy the allowlist.
    """
    separators = {"&&", "||", ";", "|"}
    segments: list[list[str]] = []
    current: list[str] = []
    for tok in tokens:
        if tok in separators:
            if current:
                segments.append(current)
                current = []
        else:
            current.append(tok)
    if current:
        segments.append(current)
    return segments


def strip_passthrough_prefix(segment: list[str]) -> list[str]:
    """Strip leading `cd <dir>` style tokens that don't change the verdict."""
    while segment and segment[0] in PREFIX_PASSTHROUGH:
        # Drop the passthrough verb plus its argument (if any).
        segment = segment[1:]
        if segment and not _is_separator(segment[0]):
            segment = segment[1:]
    return segment


def _is_separator(tok: str) -> bool:
    return tok in {"&&", "||", ";", "|"}


def segment_matches_allowlist(segment: list[str]) -> bool:
    if not segment:
        return True  # empty is harmless
    for prefix in ALLOWED_PREFIXES:
        if len(segment) >= len(prefix) and tuple(segment[: len(prefix)]) == prefix:
            return True
    return False


def check_bash(command: str) -> dict:
    """Return an allow/deny decision for a Bash command from build-rust."""
    try:
        tokens = shlex.split(command, posix=True)
    except ValueError as exc:
        return deny(
            f"build-rust hook: could not tokenize the bash command "
            f"({exc}). Refusing rather than guessing. Re-run with a "
            f"simpler quoting."
        )

    segments = split_into_segments(tokens)
    if not segments:
        return allow()

    # Reject `cargo fmt` outright — the spec mandates `rustfmt <path>`
    # invocation, never `cargo fmt`. Caught here before allowlist for a
    # cleaner error message; without this it would simply not match
    # ALLOWED_PREFIXES and produce a generic "not on allowlist" message.
    for seg in segments:
        seg = strip_passthrough_prefix(seg)
        if len(seg) >= 2 and seg[0] == "cargo" and seg[1] == "fmt":
            return deny(
                "build-rust hook: `cargo fmt` is not allowed. The agent's "
                "spec mandates `rustfmt crates/<pkg>/src/<file>.rs` so "
                "formatting cannot reach test-builder's scaffolds under "
                "tests/. See agents/build/build-rust/inputs.yml `cargo-fmt` "
                "command."
            )

    # If the agent is wrapping in `wsl bash -c "<inner>"`, re-tokenize and
    # re-check the inner command. Only the first segment after the `wsl bash`
    # prefix is inspected — the wrapper itself is allowed.
    for i, seg in enumerate(segments):
        seg = strip_passthrough_prefix(seg)
        if len(seg) >= 3 and seg[0] == "wsl" and seg[1] == "bash" and seg[2] == "-c":
            if len(seg) >= 4:
                inner = seg[3]
                inner_decision = check_bash(inner)
                if (
                    inner_decision["hookSpecificOutput"]["permissionDecision"]
                    == "deny"
                ):
                    return inner_decision
            segments[i] = []  # mark wrapper as accepted

    for seg in segments:
        seg = strip_passthrough_prefix(seg)
        if not seg:
            continue
        if not segment_matches_allowlist(seg):
            head = " ".join(seg[:3])
            return deny(
                f"build-rust hook: command `{head}...` is not on build-rust's "
                f"Bash allowlist. Allowed prefixes: cargo "
                f"{{test,build,check,clippy}}, rustfmt, git "
                f"{{status,diff,log,show,rev-parse,branch,add,commit,stash}}, "
                f"ls/pwd/echo. If you need a different command, escalate to "
                f"the user — do not work around the allowlist."
            )

    return allow()


# ---------------------------------------------------------------------------
# Write/Edit path checks
# ---------------------------------------------------------------------------
def check_write_or_edit(tool_name: str, file_path: str) -> dict:
    """Return an allow/deny decision for a file write from build-rust."""
    # Path is sometimes absolute (Windows UNC or WSL path); reduce to a
    # repo-relative form by taking the segment after `Agentic/` if present.
    rel = file_path.replace("\\", "/")
    marker = "Agentic/"
    idx = rel.rfind(marker)
    if idx >= 0:
        rel = rel[idx + len(marker) :]
    # Also strip a leading slash in case the path was already repo-rooted.
    rel = rel.lstrip("/")

    for glob in DENIED_WRITE_GLOBS:
        if fnmatch.fnmatch(rel, glob):
            return deny(
                f"build-rust hook: writes to `{glob}` are forbidden. "
                f"Path `{rel}` matches. Test authoring is test-builder's "
                f"sole authority per ADR-0005; if a test needs to change, "
                f"escalate. See agents/build/build-rust/contract.yml "
                f"does_not_touch."
            )

    if fnmatch.fnmatch(rel, STORIES_GLOB):
        # The single permitted story-file edit is the status flip on an
        # existing stories/<id>.yml. `Write` (full file create or rewrite)
        # is never that — reject Write unconditionally on stories/**.
        if tool_name == "Write":
            return deny(
                "build-rust hook: `Write` to stories/** is forbidden. "
                "Build-rust's only permitted story-file write is the "
                "single-line status flip on an existing stories/<id>.yml — "
                "use `Edit` for that, not `Write`. Creating new files in "
                "stories/ (e.g. fixture YAMLs) is a contract violation; "
                "test fixtures live under crates/<crate>/tests/fixtures/ "
                "or in the test source itself."
            )
        # `Edit` is permitted; the Edit tool's behaviour requires the file
        # to already exist, and the agent's spec confines this edit to
        # the `status:` field.
        return allow()

    return allow()


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------
def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except json.JSONDecodeError as exc:
        # Fail open — if we can't parse the input, let the harness handle it.
        # The hook should not be a tighter gate than the harness intends.
        sys.stderr.write(f"build-rust-guard: malformed hook input ({exc}); "
                         f"falling through.\n")
        return 0

    agent_type = payload.get("agent_type")
    agent_id = payload.get("agent_id")
    # Only enforce when the caller is build-rust. Both fields are checked
    # because the docs refer to both; we accept either signal.
    if agent_type != "build-rust" and agent_id != "build-rust":
        print(json.dumps(allow()))
        return 0

    tool_name = payload.get("tool_name", "")
    tool_input = payload.get("tool_input", {}) or {}

    if tool_name == "Bash":
        command = tool_input.get("command", "")
        decision = check_bash(command)
    elif tool_name in ("Write", "Edit"):
        file_path = tool_input.get("file_path", "")
        decision = check_write_or_edit(tool_name, file_path)
    else:
        decision = allow()

    print(json.dumps(decision))
    return 0


if __name__ == "__main__":
    sys.exit(main())
