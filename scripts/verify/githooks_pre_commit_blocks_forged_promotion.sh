#!/usr/bin/env bash
# =============================================================================
# githooks_pre_commit_blocks_forged_promotion.sh -- Story 29, acceptance test 3.
# =============================================================================
#
# Story 29: pre-commit hook refuses commits whose corpus state would render as
# drift. This verifier proves the hook REFUSES a commit whose post-commit
# corpus state would be the forged-promotion shape: a story whose YAML says
# `status: healthy` with no `uat_signings.verdict=pass` row and no
# `manual_signings` row -- i.e. story 25's fifth audit category
# (`yaml-healthy-without-signing-row`) and story 3's `error: status-evidence
# mismatch` row.
#
# Strategy
# --------
# Build a synthetic temp repo with:
#   - the real repo's `.githooks/pre-commit` script,
#   - a `stories/` directory with one `under_construction` story,
#   - no embedded store / no signing row.
# Configure `core.hooksPath=.githooks`, hand-edit the story's `status:` line
# to `healthy`, stage it, and try to `git commit`. Assert:
#   - `git commit` exits non-zero,
#   - HEAD did NOT advance,
#   - stderr names the offending story id (the literal id we forged).
#
# Today the real repo has no `.githooks/pre-commit`, so the verifier fails
# closed at the source-hook check. Once story 29 ships the hook + the
# audit/health gate-mode exit codes (stories 3 and 25), the synthetic
# forge is exercised and the verifier turns green.
#
# Contract
# --------
#   githooks_pre_commit_blocks_forged_promotion.sh [--help]
#
#   Exit codes:
#     0  hook refused the forged commit, HEAD unchanged, stderr names the id.
#     1  hook permitted the forged commit, OR HEAD advanced, OR stderr did
#        not name the offending story id, OR source hook missing, OR a
#        runtime dependency is missing.
#     2  usage error.
# =============================================================================

set -euo pipefail

print_usage() {
  cat <<'EOF'
Usage: githooks_pre_commit_blocks_forged_promotion.sh [--help]

Build a synthetic temp repo, install the real repo's .githooks/pre-commit,
hand-forge a story's status to healthy without a signing row, and assert
the hook refuses the resulting commit.

Options:
  --help, -h    Show this help.

Exit codes:
  0  hook refused the forged commit, HEAD unchanged, stderr names the id
  1  hook permitted the forged commit, or any sub-invariant failed
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
  printf 'fail: source hook %s does not exist; story 29 has not yet shipped the tracked pre-commit script -- the forged-promotion walkthrough cannot be exercised\n' "$SOURCE_HOOK" >&2
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

REPO="$TMPDIR_ROOT/forge-corpus"
mkdir -p "$REPO"

cd -- "$REPO"
git init -q -b main >/dev/null
git config user.email "story29-test@local.invalid"
git config user.name "story29-test"
git config commit.gpgsign false

# Seed: one `under_construction` story with no signing row. Picking a
# synthetic high id (9002) so it never collides with a real story.
mkdir -p stories
cat >stories/9002.yml <<'EOF'
id: 9002
title: Synthetic forge-target fixture story for story 29 verifier 3
outcome: |
  Currently `under_construction`. The hook MUST refuse a commit that
  flips this story's status to `healthy` without a corresponding
  uat_signings or manual_signings row -- the forged-promotion shape
  story 25's fifth audit category was added to detect.
status: under_construction
acceptance:
  tests: []
  uat: |
    Not applicable; this fixture is exercised end-to-end by
    scripts/verify/githooks_pre_commit_blocks_forged_promotion.sh.
EOF

git add stories/9002.yml
git commit -q -m "seed: under_construction fixture"

mkdir -p .githooks
cp -- "$SOURCE_HOOK" .githooks/pre-commit
chmod +x .githooks/pre-commit
git add .githooks/pre-commit
git commit -q -m "seed: install pre-commit hook"
git config core.hooksPath .githooks

HEAD_BEFORE="$(git rev-parse HEAD)"

# Forge: hand-edit the `status:` line from `under_construction` to `healthy`.
sed -i 's/^status: under_construction$/status: healthy/' stories/9002.yml
if ! grep -q '^status: healthy$' stories/9002.yml; then
  die_runtime "internal: forge sed did not produce 'status: healthy' line; stories/9002.yml is malformed for the test"
fi

git add stories/9002.yml

commit_stderr="$TMPDIR_ROOT/commit-stderr.txt"
set +e
git commit -m "forge: 9002" 2>"$commit_stderr"
commit_status=$?
set -e

if [[ "$commit_status" -eq 0 ]]; then
  printf 'fail: git commit returned 0 on a forged-promotion attempt; the hook MUST refuse this case (story 25 fifth audit category + story 3 error row).\n' >&2
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

# Assert stderr names the offending story id. The hook is required to
# emit a message that lets the developer know which story tripped the
# gate without re-running the audit by hand. We accept either the bare
# integer (9002) or a path-shaped reference (stories/9002.yml).
if ! grep -qE '(\b9002\b|stories/9002\.yml)' -- "$commit_stderr"; then
  printf 'fail: hook refused the commit but stderr did not name the offending story id (9002 / stories/9002.yml); a developer cannot triage without that.\n' >&2
  printf '----- captured stderr -----\n' >&2
  cat -- "$commit_stderr" >&2 || true
  printf '----- end stderr -----\n' >&2
  exit 1
fi

printf 'githooks_pre_commit_blocks_forged_promotion: ok (commit refused exit=%d, HEAD unchanged at %s, stderr named 9002)\n' "$commit_status" "$HEAD_AFTER"
exit 0
