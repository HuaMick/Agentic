#!/usr/bin/env bash
# =============================================================================
# githooks_pre_commit_finds_agentic_when_installed_via_cargo.sh -- Story 29, test 6.
# =============================================================================
#
# Story 29 (2026-04-30 amendment): pre-commit hook MUST self-discover the
# `agentic` binary at the standard cargo install location (`~/.cargo/bin/agentic`)
# even when git invokes the hook with PATH stripped to its typical default
# (`/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin`). Without
# self-discovery, the existing `command -v agentic` check at .githooks/pre-commit
# returns non-zero on developed clones, the hook prints the "skipping" notice,
# and the gate silently fail-opens -- a forged-promotion or unhealthy-induction
# commit lands unchallenged. This is the regression discovered against HEAD
# `919745b` and pinned by this verifier.
#
# Strategy
# --------
# 1. Confirm `agentic` is installed at `${HOME}/.cargo/bin/agentic`. If absent,
#    SKIP with a clear message (this isn't a fresh-clone test; the verifier's
#    contract assumes a developed clone).
# 2. Build a synthetic temp repo whose corpus is clean (one `proposed` story,
#    no signing rows demanded) so that IF the hook self-discovers the binary
#    and proceeds into the audit + health checks, both CLIs return exit 0
#    against the synthetic corpus and we get a deterministic non-skipping run.
# 3. Install the real repo's `.githooks/pre-commit` into the temp repo.
# 4. Stage a trivial content-only edit, then invoke `.githooks/pre-commit`
#    DIRECTLY (not via `git commit`) with PATH explicitly set to git's typical
#    hook environment. Capture stderr.
# 5. Assert stderr does NOT contain the literal "skipping pre-commit hook"
#    string. The hook self-discovered the binary if and only if that notice
#    is absent.
#
# The verifier is RED today because `.githooks/pre-commit` performs
# `command -v agentic` BEFORE prepending `~/.cargo/bin` to PATH. Build-rust
# turns it green by editing the hook to PATH-prepend (or equivalent
# direct-lookup) before the existence check.
#
# Contract
# --------
#   githooks_pre_commit_finds_agentic_when_installed_via_cargo.sh [--help]
#
#   Exit codes:
#     0  hook self-discovered agentic; the "skipping" notice was NOT emitted.
#     1  hook printed the "skipping" notice (the regression), OR source hook
#        missing, OR runtime dependency missing.
#     2  usage error.
#    77  agentic is not installed at ~/.cargo/bin/agentic; the verifier's
#        precondition does not hold. Skipped, not failed.
# =============================================================================

set -euo pipefail

print_usage() {
  cat <<'EOF'
Usage: githooks_pre_commit_finds_agentic_when_installed_via_cargo.sh [--help]

Build a synthetic temp repo, install the real repo's .githooks/pre-commit,
invoke the hook directly with PATH stripped to git's typical hook
environment, and assert the hook self-discovers the cargo-installed
`agentic` binary (no "skipping pre-commit hook" notice on stderr).

Options:
  --help, -h    Show this help.

Exit codes:
  0   hook self-discovered the binary; no "skipping" notice
  1   hook printed the "skipping" notice, or any sub-invariant failed
  2   usage error
  77  agentic not installed at ~/.cargo/bin/agentic; precondition unmet
EOF
}

die_runtime() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

find_repo_root() {
  local dir
  dir="$1"
  while [[ "$dir" != "/" && -n "$dir" ]]; do
    if [[ -e "$dir/.git" ]]; then
      printf '%s\n' "$dir"
      return 0
    fi
    dir="$(dirname -- "$dir")"
  done
  return 1
}

case "${1:-}" in
  --help|-h)
    print_usage
    exit 0
    ;;
  '')
    ;;
  *)
    printf 'error: unexpected argument: %s\n\n' "$1" >&2
    print_usage >&2
    exit 2
    ;;
esac

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if ! REPO_ROOT="$(find_repo_root "$PWD")"; then
  if ! REPO_ROOT="$(find_repo_root "$script_dir")"; then
    die_runtime "could not find repo root (.git) from CWD or script dir"
  fi
fi

SOURCE_HOOK="$REPO_ROOT/.githooks/pre-commit"

if [[ ! -f "$SOURCE_HOOK" ]]; then
  printf 'fail: source hook %s does not exist; story 29 has not yet shipped the tracked pre-commit script -- the cargo-discovery walkthrough cannot be exercised\n' "$SOURCE_HOOK" >&2
  exit 1
fi
if [[ ! -x "$SOURCE_HOOK" ]]; then
  printf 'fail: source hook %s exists but is not executable; cannot be invoked by git\n' "$SOURCE_HOOK" >&2
  exit 1
fi

if ! command -v git >/dev/null 2>&1; then
  die_runtime "git is required on PATH"
fi

# Precondition: agentic must be installed at the standard cargo location.
# This verifier's contract is the developed-clone case (after `cargo install
# --path crates/agentic-cli --locked`), not the fresh-clone bootstrap case.
# If the binary is genuinely absent, SKIP with exit 77 -- not a failure.
CARGO_AGENTIC="${HOME:-/root}/.cargo/bin/agentic"
if [[ ! -x "$CARGO_AGENTIC" ]]; then
  printf 'skip: %s not present or not executable; this verifier requires a developed clone with `cargo install --path crates/agentic-cli --locked` already run. Install the binary and re-run.\n' "$CARGO_AGENTIC" >&2
  exit 77
fi

TMPDIR_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_ROOT"' EXIT

REPO="$TMPDIR_ROOT/cargo-discovery-corpus"
mkdir -p "$REPO"

cd -- "$REPO"
git init -q -b main >/dev/null
git config user.email "story29-test@local.invalid"
git config user.name "story29-test"
git config commit.gpgsign false

# Seed: one minimal `proposed` story. `proposed` is clean for both
# `agentic stories health --all` (no signing-row obligation) and
# `agentic stories audit` (none of the five drift categories trip on
# proposed shape). If the hook self-discovers the binary, it proceeds
# into both CLIs against this corpus and they both return 0 -- so the
# hook returns 0 silently. If the hook does NOT self-discover, it
# prints the "skipping" notice and exits 0 -- same exit code, but the
# stderr signature is what we assert on.
mkdir -p stories
cat >stories/9006.yml <<'EOF'
id: 9006
title: Synthetic clean fixture for story 29 cargo-discovery verifier
outcome: |
  Holds the clean-corpus shape for the cargo-discovery walkthrough. A
  single `proposed` story with no acceptance.tests, no related_files,
  and no signing-row obligation. The hook MUST proceed past the
  binary-discovery branch and run both CLIs cleanly against this
  corpus.
status: proposed
acceptance:
  tests: []
  uat: |
    Not applicable; this fixture is exercised end-to-end by
    scripts/verify/githooks_pre_commit_finds_agentic_when_installed_via_cargo.sh.
EOF

cat >README.md <<'EOF'
# Cargo-discovery fixture

Tracked content-only file used by story 29's cargo-discovery verifier
to provide a stage-able edit while exercising the pre-commit hook.
EOF

git add stories/9006.yml README.md
git commit -q -m "seed: clean fixture for cargo-discovery verifier"

mkdir -p .githooks
cp -- "$SOURCE_HOOK" .githooks/pre-commit
chmod +x .githooks/pre-commit
git add .githooks/pre-commit
git commit -q -m "seed: install pre-commit hook"

# Stage a trivial content-only edit so the hook has a non-empty staged
# tree to inspect (mirrors a real commit's invocation context).
printf '\nA second line, content-only, no corpus impact.\n' >>README.md
git add README.md

# The core probe: invoke `.githooks/pre-commit` directly, with PATH set
# to git's typical pre-commit-hook default (NO `~/.cargo/bin`). Capture
# stderr; preserve HOME so the hook can resolve `${HOME}/.cargo/bin`
# itself if it does PATH-prepend (the fix shape).
GIT_HOOK_PATH='/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin'
hook_stderr="$TMPDIR_ROOT/hook-stderr.txt"
hook_stdout="$TMPDIR_ROOT/hook-stdout.txt"

set +e
env -i HOME="${HOME:-/root}" PATH="$GIT_HOOK_PATH" .githooks/pre-commit \
  >"$hook_stdout" 2>"$hook_stderr"
hook_status=$?
set -e

# The assertion: stderr must NOT contain the "skipping pre-commit hook"
# fail-open notice. That notice is the literal signature of the
# regression: the hook ran command -v agentic against the stripped PATH,
# did not find the cargo-installed binary, printed the notice, and
# exited 0 -- silently bypassing the gate on developed clones.
if grep -qF 'skipping pre-commit hook' -- "$hook_stderr"; then
  printf 'fail: hook printed the fail-open notice "skipping pre-commit hook" against a stripped PATH; the cargo-installed binary at %s was NOT self-discovered. This is the 2026-04-30 regression -- the gate fail-opens silently on developed clones whose git invokes the hook with PATH stripped of ~/.cargo/bin.\n' "$CARGO_AGENTIC" >&2
  printf 'hook exit code: %d\n' "$hook_status" >&2
  printf 'captured stderr (begin):\n' >&2
  cat -- "$hook_stderr" >&2 || true
  printf 'captured stderr (end)\n' >&2
  exit 1
fi

printf 'githooks_pre_commit_finds_agentic_when_installed_via_cargo: ok (hook self-discovered %s under stripped PATH; no skipping notice; hook exit=%d)\n' "$CARGO_AGENTIC" "$hook_status"
exit 0
