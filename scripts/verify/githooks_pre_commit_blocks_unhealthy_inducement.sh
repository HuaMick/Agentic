#!/usr/bin/env bash
# =============================================================================
# githooks_pre_commit_blocks_unhealthy_inducement.sh -- Story 29, test 4.
# =============================================================================
#
# Story 29: pre-commit hook refuses commits whose corpus state would render as
# drift. This verifier proves the hook REFUSES a commit whose post-commit
# corpus state would induce a fell-from-grace classification: a previously-
# healthy story whose `related_files` (or staged source tree) now point at a
# file the dashboard cannot resolve, or whose acceptance tests would fail --
# the `unhealthy` shape the dashboard surfaces and story 3's revised exit-code
# contract maps to exit 2.
#
# Strategy
# --------
# Build a synthetic temp repo with:
#   - the real repo's `.githooks/pre-commit`,
#   - a `stories/` directory containing a healthy story whose
#     `related_files:` points at a file we will then DELETE in the staged
#     commit. A staged change that removes a related-files target reproduces
#     the "fell-from-grace" inducement shape: post-commit, the story still
#     claims `status: healthy` but its related_files claim a path that no
#     longer exists, which the dashboard classifies as `unhealthy` (or at
#     minimum, an error row -- both trip the hook's gate).
# Configure `core.hooksPath=.githooks`, stage the deletion, attempt
# `git commit`. Assert:
#   - exit non-zero,
#   - HEAD unchanged,
#   - stderr names the offending story id.
#
# Today the real repo has no `.githooks/pre-commit`, so the verifier fails
# closed at the source-hook check. Once story 29 ships the hook + the
# audit/health gate-mode exit codes (stories 3 and 25), the synthetic
# inducement is exercised and the verifier turns green.
#
# Contract
# --------
#   githooks_pre_commit_blocks_unhealthy_inducement.sh [--help]
#
#   Exit codes:
#     0  hook refused the inducement commit, HEAD unchanged, stderr names id.
#     1  hook permitted the commit, OR HEAD advanced, OR stderr did not
#        name the story id, OR source hook missing, OR runtime dep missing.
#     2  usage error.
# =============================================================================

set -euo pipefail

print_usage() {
  cat <<'EOF'
Usage: githooks_pre_commit_blocks_unhealthy_inducement.sh [--help]

Build a synthetic temp repo, install the real repo's .githooks/pre-commit,
delete a healthy story's related-files target, and assert the hook refuses
the resulting commit.

Options:
  --help, -h    Show this help.

Exit codes:
  0  hook refused the inducement commit, HEAD unchanged, stderr names id
  1  hook permitted, or any sub-invariant failed
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
  printf 'fail: source hook %s does not exist; story 29 has not yet shipped the tracked pre-commit script -- the unhealthy-inducement walkthrough cannot be exercised\n' "$SOURCE_HOOK" >&2
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

REPO="$TMPDIR_ROOT/induce-corpus"
mkdir -p "$REPO"

cd -- "$REPO"
git init -q -b main >/dev/null
git config user.email "story29-test@local.invalid"
git config user.name "story29-test"
git config commit.gpgsign false

# Seed: a `healthy` story that claims a related_files target. Synthetic
# id 9003 to avoid collision with real corpus.
mkdir -p stories src
cat >src/widget.rs <<'EOF'
// Synthetic source file that story 9003 claims as a related_files target.
// Deleting this file in a staged commit reproduces the fell-from-grace
// inducement shape that the pre-commit hook MUST refuse.
pub fn widget() {}
EOF

cat >stories/9003.yml <<'EOF'
id: 9003
title: Synthetic inducement-target fixture story for story 29 verifier 4
outcome: |
  Currently `healthy`; claims `src/widget.rs` as a related_files
  target. The hook MUST refuse a commit that deletes that target,
  because the post-commit corpus state would have a healthy story
  whose related_files claim a non-existent file -- the fell-from-
  grace shape story 3's dashboard surfaces as `unhealthy`.
status: healthy
acceptance:
  tests: []
  uat: |
    Not applicable; this fixture is exercised end-to-end by
    scripts/verify/githooks_pre_commit_blocks_unhealthy_inducement.sh.
related_files:
  - src/widget.rs
EOF

git add stories/9003.yml src/widget.rs
git commit -q -m "seed: healthy fixture with related_files target"

mkdir -p .githooks
cp -- "$SOURCE_HOOK" .githooks/pre-commit
chmod +x .githooks/pre-commit
git add .githooks/pre-commit
git commit -q -m "seed: install pre-commit hook"
git config core.hooksPath .githooks

HEAD_BEFORE="$(git rev-parse HEAD)"

# Induce: delete the related-files target. The staged tree now has a
# healthy story whose related_files claim a path that no longer exists
# -- the dashboard's fell-from-grace shape.
git rm -q src/widget.rs

commit_stderr="$TMPDIR_ROOT/commit-stderr.txt"
set +e
git commit -m "induce: delete 9003 related_files target" 2>"$commit_stderr"
commit_status=$?
set -e

if [[ "$commit_status" -eq 0 ]]; then
  printf 'fail: git commit returned 0 on an unhealthy-inducement attempt; the hook MUST refuse this case (story 3 dashboard fell-from-grace + exit 2 gate).\n' >&2
  printf '----- captured stderr -----\n' >&2
  cat -- "$commit_stderr" >&2 || true
  printf '----- end stderr -----\n' >&2
  exit 1
fi

HEAD_AFTER="$(git rev-parse HEAD)"
if [[ "$HEAD_BEFORE" != "$HEAD_AFTER" ]]; then
  printf 'fail: HEAD advanced (%s -> %s) despite git commit reporting non-zero (%d); the corpus moved when the hook claimed to refuse it.\n' "$HEAD_BEFORE" "$HEAD_AFTER" "$commit_status" >&2
  exit 1
fi

if ! grep -qE '(\b9003\b|stories/9003\.yml)' -- "$commit_stderr"; then
  printf 'fail: hook refused the commit but stderr did not name the offending story id (9003 / stories/9003.yml); a developer cannot triage without that.\n' >&2
  printf '----- captured stderr -----\n' >&2
  cat -- "$commit_stderr" >&2 || true
  printf '----- end stderr -----\n' >&2
  exit 1
fi

printf 'githooks_pre_commit_blocks_unhealthy_inducement: ok (commit refused exit=%d, HEAD unchanged at %s, stderr named 9003)\n' "$commit_status" "$HEAD_AFTER"
exit 0
