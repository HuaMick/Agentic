#!/usr/bin/env bash
# Exercises orchestrator_edit_guard.py against a battery of inputs.
#
# The hook denies orchestrator-context (no agent_type) Edit/Write to
# scripts/verify/**, crates/*/tests/**, and evidence/runs/**. Subagents
# fall through with allow on any path. Bash is intentionally NOT
# inspected — see the "Bash-gap demonstration" section at the bottom
# for the smoke test that pins the gap in evidence (per CLAUDE.md's
# "Surface ownership" section explaining the deliberate omission).
#
# Each case prints the input one-liner, the decision, and whether it
# matched expectations. Exit non-zero if any case failed.
set -u

HOOK="$(dirname "$0")/orchestrator_edit_guard.py"
PASS=0
FAIL=0

check() {
    local label="$1"; shift
    local expected="$1"; shift
    local payload="$1"; shift
    local actual
    # The hook fails open on malformed JSON: returns exit 0 with no stdout
    # (relying on the harness to interpret the absence of a decision as
    # "no constraint, continue"). Our parser must treat that case as "allow"
    # to match the hook's contract; map empty stdout to "allow".
    local raw_stdout
    raw_stdout="$(printf '%s' "$payload" | python3 "$HOOK" 2>/dev/null)"
    if [ -z "$raw_stdout" ]; then
        actual="allow"
    else
        actual="$(printf '%s' "$raw_stdout" | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d["hookSpecificOutput"]["permissionDecision"])' 2>/dev/null || echo "allow")"
    fi
    if [ "$actual" = "$expected" ]; then
        printf '  PASS  %-72s expected=%s actual=%s\n' "$label" "$expected" "$actual"
        PASS=$((PASS+1))
    else
        printf '  FAIL  %-72s expected=%s actual=%s\n' "$label" "$expected" "$actual"
        FAIL=$((FAIL+1))
    fi
}

# --- Orchestrator (no agent_type) — Edit/Write denials ----------------------
echo "== orchestrator caller, Edit/Write to test-builder territory =="
check "Edit scripts/verify/foo.sh denied" deny \
    '{"tool_name":"Edit","tool_input":{"file_path":"scripts/verify/foo.sh"}}'
check "Write scripts/verify/asset_consumer_minimum.sh denied" deny \
    '{"tool_name":"Write","tool_input":{"file_path":"scripts/verify/asset_consumer_minimum.sh"}}'
check "Edit crates/agentic-ci-record/tests/foo.rs denied" deny \
    '{"tool_name":"Edit","tool_input":{"file_path":"crates/agentic-ci-record/tests/foo.rs"}}'
check "Write crates/agentic-cli/tests/bar.rs denied" deny \
    '{"tool_name":"Write","tool_input":{"file_path":"crates/agentic-cli/tests/bar.rs"}}'
check "Edit evidence/runs/12/2026-04-27.jsonl denied" deny \
    '{"tool_name":"Edit","tool_input":{"file_path":"evidence/runs/12/2026-04-27.jsonl"}}'
check "Write evidence/runs/25/anything.jsonl denied" deny \
    '{"tool_name":"Write","tool_input":{"file_path":"evidence/runs/25/anything.jsonl"}}'

# --- Orchestrator — Edit/Write to non-protected territory should ALLOW ------
echo "== orchestrator caller, Edit/Write to non-protected territory =="
check "Edit crates/agentic-test-support/src/lib.rs allowed" allow \
    '{"tool_name":"Edit","tool_input":{"file_path":"crates/agentic-test-support/src/lib.rs"}}'
check "Edit CLAUDE.md allowed" allow \
    '{"tool_name":"Edit","tool_input":{"file_path":"CLAUDE.md"}}'
check "Write README.md allowed" allow \
    '{"tool_name":"Write","tool_input":{"file_path":"README.md"}}'
check "Edit schemas/story.schema.json allowed" allow \
    '{"tool_name":"Edit","tool_input":{"file_path":"schemas/story.schema.json"}}'
check "Edit .claude/hooks/orchestrator_edit_guard.py allowed" allow \
    '{"tool_name":"Edit","tool_input":{"file_path":".claude/hooks/orchestrator_edit_guard.py"}}'
check "Edit agents/teacher/guidance-writer/contract.yml allowed" allow \
    '{"tool_name":"Edit","tool_input":{"file_path":"agents/teacher/guidance-writer/contract.yml"}}'
check "Write stories/12.yml allowed" allow \
    '{"tool_name":"Write","tool_input":{"file_path":"stories/12.yml"}}'

# --- Subagent fall-through: agent_type set -> allow on any path -------------
echo "== subagent caller — every protected path falls through =="
check "test-builder Edit scripts/verify/foo.sh allowed" allow \
    '{"agent_type":"test-builder","tool_name":"Edit","tool_input":{"file_path":"scripts/verify/foo.sh"}}'
check "test-builder Write crates/agentic-ci-record/tests/foo.rs allowed" allow \
    '{"agent_type":"test-builder","tool_name":"Write","tool_input":{"file_path":"crates/agentic-ci-record/tests/foo.rs"}}'
check "test-builder Write evidence/runs/12/x.jsonl allowed" allow \
    '{"agent_type":"test-builder","tool_name":"Write","tool_input":{"file_path":"evidence/runs/12/x.jsonl"}}'
check "build-rust Edit crates/agentic-cli/tests/foo.rs allowed (subagent FT)" allow \
    '{"agent_type":"build-rust","tool_name":"Edit","tool_input":{"file_path":"crates/agentic-cli/tests/foo.rs"}}'
check "story-writer Edit scripts/verify/foo.sh allowed (subagent FT)" allow \
    '{"agent_type":"story-writer","tool_name":"Edit","tool_input":{"file_path":"scripts/verify/foo.sh"}}'
check "guidance-writer Edit crates/agentic-cli/tests/foo.rs allowed (subagent FT)" allow \
    '{"agent_type":"guidance-writer","tool_name":"Edit","tool_input":{"file_path":"crates/agentic-cli/tests/foo.rs"}}'

# --- Non-Edit/Write tools: orchestrator fall-through to allow ---------------
echo "== orchestrator caller, non-Edit/Write tools =="
check "Read scripts/verify/foo.sh allowed (Read not gated)" allow \
    '{"tool_name":"Read","tool_input":{"file_path":"scripts/verify/foo.sh"}}'
check "Glob crates/*/tests/* allowed (Glob not gated)" allow \
    '{"tool_name":"Glob","tool_input":{"pattern":"crates/*/tests/*"}}'
check "Grep evidence/runs/ allowed (Grep not gated)" allow \
    '{"tool_name":"Grep","tool_input":{"pattern":"foo","path":"evidence/runs/"}}'

# --- Bash-gap demonstration: HOOK ALLOWS Bash on protected paths -------------
# This section pins the deliberate Bash-gap in evidence rather than fixing it.
# Per .claude/hooks/orchestrator_edit_guard.py docstring (and its companion
# README), Bash is NOT inspected. The right corrective when an orchestrator
# reaches for `sed -i scripts/verify/foo.sh` or similar is a SHAPE CHANGE
# (spawn the owning subagent), not a tighter regex inside this hook. Adding
# shell-string parsing here would be fragile (sed/tee/>/>> idiom variants);
# the session-orchestrator spec carries the contract, and CLAUDE.md's
# "Surface ownership and joint authority" section names the gap explicitly.
#
# These cases pass with `allow` to demonstrate the gap exists. If a future
# session decides Bash inspection IS worth it, flip these expected values
# to `deny` and update the hook script accordingly.
echo "== Bash-gap (DELIBERATELY ALLOWED — see hook docstring) =="
check "Bash sed -i scripts/verify/foo.sh ALLOWED (Bash not inspected)" allow \
    '{"tool_name":"Bash","tool_input":{"command":"sed -i s/old/new/ scripts/verify/foo.sh"}}'
check "Bash tee crates/.../tests/x.rs ALLOWED (Bash not inspected)" allow \
    '{"tool_name":"Bash","tool_input":{"command":"echo new content | tee crates/agentic-ci-record/tests/x.rs"}}'
check "Bash >> evidence/runs/12/y.jsonl ALLOWED (Bash not inspected)" allow \
    '{"tool_name":"Bash","tool_input":{"command":"echo \"{...}\" >> evidence/runs/12/y.jsonl"}}'
check "Bash rm -rf crates/.../tests/ ALLOWED (Bash not inspected)" allow \
    '{"tool_name":"Bash","tool_input":{"command":"rm -rf crates/agentic-ci-record/tests/"}}'

# --- Edge cases ---------------------------------------------------------------
echo "== edge cases =="
check "Edit with empty file_path falls through to allow" allow \
    '{"tool_name":"Edit","tool_input":{"file_path":""}}'
check "Edit missing tool_input falls through to allow" allow \
    '{"tool_name":"Edit"}'
check "Malformed JSON: hook fails open (orchestrator process continues)" allow \
    '{not valid json'

# --- Summary ------------------------------------------------------------------
echo ""
echo "=========================================================================="
echo "Summary: $PASS passed, $FAIL failed"
echo "=========================================================================="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
