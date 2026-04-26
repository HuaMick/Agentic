#!/usr/bin/env bash
# Exercises build_rust_guard.py against a battery of inputs.
# Each case prints the input one-liner, the decision, and whether it matched expectations.
set -u

HOOK="$(dirname "$0")/build_rust_guard.py"
PASS=0
FAIL=0

check() {
    local label="$1"; shift
    local expected="$1"; shift
    local payload="$1"; shift
    local actual
    actual="$(printf '%s' "$payload" | python3 "$HOOK" | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d["hookSpecificOutput"]["permissionDecision"])')"
    if [ "$actual" = "$expected" ]; then
        printf '  PASS  %-60s expected=%s actual=%s\n' "$label" "$expected" "$actual"
        PASS=$((PASS+1))
    else
        printf '  FAIL  %-60s expected=%s actual=%s\n' "$label" "$expected" "$actual"
        FAIL=$((FAIL+1))
    fi
}

# --- Build-rust caller cases --------------------------------------------------
echo "== build-rust caller =="
check "cargo test allowed" allow \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"cargo test --workspace --no-fail-fast"}}'
check "cargo build allowed" allow \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"cargo build --workspace"}}'
check "cargo clippy allowed" allow \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"cargo clippy --workspace --all-targets -- -D warnings"}}'
check "rustfmt allowed" allow \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"rustfmt crates/agentic-store/src/lib.rs"}}'
check "git status allowed" allow \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"git status"}}'
check "git commit allowed" allow \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"git commit -m message"}}'
check "cd && cargo test allowed" allow \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"cd /tmp && cargo test"}}'
check "wsl bash wrapper around cargo test allowed" allow \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"wsl bash -c \"cargo test\""}}'

# --- Forbidden Bash commands --------------------------------------------------
check "cargo fmt --all denied" deny \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"cargo fmt --all"}}'
check "cargo fmt -p denied" deny \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"cargo fmt -p agentic-store"}}'
check "cargo fmt no-args denied" deny \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"cargo fmt"}}'
check "cd && cargo fmt --all denied" deny \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"cd /tmp && cargo fmt --all"}}'
check "wsl bash -c cargo fmt denied" deny \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"wsl bash -c \"cargo fmt --all\""}}'
check "git push denied" deny \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"git push origin main"}}'
check "git reset --hard denied" deny \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"git reset --hard HEAD"}}'
check "rm -rf denied (not on allowlist)" deny \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"rm -rf target"}}'
check "cargo install denied" deny \
    '{"agent_type":"build-rust","tool_name":"Bash","tool_input":{"command":"cargo install ripgrep"}}'

# --- File path checks --------------------------------------------------------
check "Write to crates/agentic-uat/tests/foo.rs denied" deny \
    '{"agent_type":"build-rust","tool_name":"Write","tool_input":{"file_path":"/home/code/Agentic/crates/agentic-uat/tests/foo.rs"}}'
check "Edit to crates/agentic-uat/tests/foo.rs denied" deny \
    '{"agent_type":"build-rust","tool_name":"Edit","tool_input":{"file_path":"/home/code/Agentic/crates/agentic-uat/tests/foo.rs"}}'
check "Edit to crates/agentic-uat/src/lib.rs allowed" allow \
    '{"agent_type":"build-rust","tool_name":"Edit","tool_input":{"file_path":"/home/code/Agentic/crates/agentic-uat/src/lib.rs"}}'
check "Write to stories/4001.yml denied (fixture pollution)" deny \
    '{"agent_type":"build-rust","tool_name":"Write","tool_input":{"file_path":"/home/code/Agentic/stories/4001.yml"}}'
check "Edit to stories/16.yml allowed (status flip)" allow \
    '{"agent_type":"build-rust","tool_name":"Edit","tool_input":{"file_path":"/home/code/Agentic/stories/16.yml"}}'

# --- Non-build-rust callers fall through -------------------------------------
echo "== non-build-rust caller =="
check "test-builder cargo fmt --all falls through" allow \
    '{"agent_type":"test-builder","tool_name":"Bash","tool_input":{"command":"cargo fmt --all"}}'
check "orchestrator (no agent_type) falls through" allow \
    '{"tool_name":"Bash","tool_input":{"command":"cargo fmt --all"}}'
check "test-builder Write to crates tests falls through" allow \
    '{"agent_type":"test-builder","tool_name":"Write","tool_input":{"file_path":"/home/code/Agentic/crates/agentic-uat/tests/foo.rs"}}'

echo ""
echo "Result: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
