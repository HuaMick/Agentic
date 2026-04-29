#!/usr/bin/env bash
# =============================================================================
# githooks_pre_commit_passes_on_clean_corpus.sh -- Story 29, acceptance test 2.
# =============================================================================
#
# Story 29: pre-commit hook refuses commits whose corpus state would render as
# drift. This verifier proves the hook PERMITS commits when the corpus is
# clean -- a hook that false-fails on clean commits would block every
# developer's normal workflow, a silent catastrophic tightening worse than
# the forged promotion the gate exists to prevent.
#
# Strategy
# --------
# Build a synthetic temp repo that contains:
#   - a `.githooks/pre-commit` script copied verbatim from the real repo,
#   - a `stories/` directory with one minimal `proposed` story (no signing
#     row required; proposed is clean by definition for both `agentic
#     stories health` and `agentic stories audit`),
#   - a `README.md` (so we have a non-corpus tracked file to amend in the
#     clean-commit attempt).
# Configure `core.hooksPath=.githooks`, stage a content-only edit to README,
# run `git commit`, and assert exit 0 plus a new HEAD SHA.
#
# Today the real repo has no `.githooks/pre-commit`, so the copy step finds
# no source file and the verifier fails closed. Once story 29 is built, the
# hook copy succeeds, the synthetic clean corpus is exercised, and the
# verifier turns green.
#
# Contract
# --------
#   githooks_pre_commit_passes_on_clean_corpus.sh [--help]
#
#   Exit codes:
#     0  the hook permitted the clean commit.
#     1  the hook refused the clean commit, OR the source hook is missing,
#        OR a runtime dependency is missing.
#     2  usage error.
# =============================================================================

set -euo pipefail

print_usage() {
  cat <<'EOF'
Usage: githooks_pre_commit_passes_on_clean_corpus.sh [--help]

Build a synthetic temp repo, install the real repo's .githooks/pre-commit,
configure core.hooksPath, and assert a clean-corpus commit lands without
refusal.

Options:
  --help, -h    Show this help.

Exit codes:
  0  hook permitted the clean commit
  1  hook refused, source hook missing, or runtime dep missing
  2  usage error
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
  printf 'fail: source hook %s does not exist; story 29 has not yet shipped the tracked pre-commit script -- the clean-corpus walkthrough cannot be exercised\n' "$SOURCE_HOOK" >&2
  exit 1
fi
if [[ ! -x "$SOURCE_HOOK" ]]; then
  printf 'fail: source hook %s exists but is not executable; cannot be invoked by git\n' "$SOURCE_HOOK" >&2
  exit 1
fi

if ! command -v git >/dev/null 2>&1; then
  die_runtime "git is required on PATH"
fi

TMPDIR_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_ROOT"' EXIT

REPO="$TMPDIR_ROOT/clean-corpus"
mkdir -p "$REPO"

cd -- "$REPO"
git init -q -b main >/dev/null
git config user.email "story29-test@local.invalid"
git config user.name "story29-test"
git config commit.gpgsign false

# Seed the synthetic corpus: one minimal `proposed` story plus a README.
# `proposed` stories are clean for both `agentic stories health` (they
# never demand a signing row) and `agentic stories audit` (none of the
# five drift categories trigger on a `proposed` shape).
mkdir -p stories
cat >stories/9001.yml <<'EOF'
id: 9001
title: Synthetic clean-corpus fixture story for story 29 verifier 2
outcome: |
  Holds the clean-corpus shape: a single `proposed` story with no
  acceptance.tests entries, no related_files claims, and no signing
  row. The pre-commit hook MUST permit a content-only commit against
  this corpus.
status: proposed
acceptance:
  tests: []
  uat: |
    Not applicable; this fixture is exercised end-to-end by
    scripts/verify/githooks_pre_commit_passes_on_clean_corpus.sh.
EOF

cat >README.md <<'EOF'
# Clean-corpus fixture

Tracked content-only file used by story 29's clean-commit verifier
to prove the pre-commit hook permits non-drift-inducing commits.
EOF

git add stories/9001.yml README.md
git commit -q -m "seed: synthetic clean corpus"

# Install the hook under .githooks/ and configure core.hooksPath.
mkdir -p .githooks
cp -- "$SOURCE_HOOK" .githooks/pre-commit
chmod +x .githooks/pre-commit
git add .githooks/pre-commit
git commit -q -m "seed: install pre-commit hook"
git config core.hooksPath .githooks

HEAD_BEFORE="$(git rev-parse HEAD)"

# Stage a trivial content-only edit -- amend the tracked README.
printf '\nA second line, content-only, no corpus impact.\n' >>README.md

git add README.md

# Attempt the clean commit. Capture stderr; do NOT use --no-verify (the
# whole point is to exercise the hook).
commit_stderr="$TMPDIR_ROOT/commit-stderr.txt"
set +e
git commit -m "test: clean-corpus content-only edit" 2>"$commit_stderr"
commit_status=$?
set -e

if [[ "$commit_status" -ne 0 ]]; then
  printf 'fail: pre-commit hook refused a clean-corpus commit (git commit exit=%d); a hook that false-fails here breaks every developer workflow.\n' "$commit_status" >&2
  printf '----- captured stderr -----\n' >&2
  cat -- "$commit_stderr" >&2 || true
  printf '----- end stderr -----\n' >&2
  exit 1
fi

HEAD_AFTER="$(git rev-parse HEAD)"
if [[ "$HEAD_BEFORE" == "$HEAD_AFTER" ]]; then
  printf 'fail: git commit reported success (exit 0) but HEAD did not advance; %s == %s\n' "$HEAD_BEFORE" "$HEAD_AFTER" >&2
  exit 1
fi

printf 'githooks_pre_commit_passes_on_clean_corpus: ok (clean-corpus commit landed; HEAD moved %s -> %s)\n' "$HEAD_BEFORE" "$HEAD_AFTER"
exit 0
