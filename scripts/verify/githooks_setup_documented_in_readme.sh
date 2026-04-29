#!/usr/bin/env bash
# =============================================================================
# githooks_setup_documented_in_readme.sh -- Story 29, acceptance test 5.
# =============================================================================
#
# Story 29: pre-commit hook refuses commits whose corpus state would render as
# drift. This verifier proves the one-time setup is discoverable: the
# top-level `README.md` contains the literal command `git config
# core.hooksPath .githooks`, AND it does NOT instruct the developer to
# bypass the hook (no `--no-verify`, no `core.hooksPath ""`, no "skip the
# hook" guidance).
#
# Without this verifier, the hook ships in the tree but no developer is
# told to enable it -- `core.hooksPath` is not on by default, so an
# undocumented hook is effectively not deployed. Conversely, if the README
# tells developers how to bypass the hook, the structural enforcement is
# undone in prose.
#
# Contract
# --------
#   githooks_setup_documented_in_readme.sh [--help]
#
#   Inputs:
#     (none)  -- reads `<repo-root>/README.md`.
#
#   Exit codes:
#     0  README contains the literal setup command and no bypass language.
#     1  README missing the setup command, OR contains bypass language,
#        OR README itself is missing.
#     2  usage error.
# =============================================================================

set -euo pipefail

print_usage() {
  cat <<'EOF'
Usage: githooks_setup_documented_in_readme.sh [--help]

Assert top-level README.md contains the literal `git config
core.hooksPath .githooks` and does NOT instruct the developer to
bypass the hook.

Options:
  --help, -h    Show this help.

Exit codes:
  0  README contains setup command, no bypass language present
  1  README missing the setup command, or contains bypass language
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

README="$REPO_ROOT/README.md"

if [[ ! -f "$README" ]]; then
  printf 'fail: %s does not exist; story 29 requires the setup instruction to land in the top-level README\n' "$README" >&2
  exit 1
fi

failures=0

# Required: the literal setup command. We grep for the exact byte sequence.
if ! grep -qF 'git config core.hooksPath .githooks' -- "$README"; then
  printf 'fail: %s does not contain the literal command `git config core.hooksPath .githooks`; without it, no developer is told to enable the hook and the gate is effectively undeployed.\n' "$README" >&2
  failures=$((failures + 1))
fi

# Forbidden: bypass language. Each pattern is a known way to undo the gate
# in prose. We are deliberately conservative -- patterns are matched
# case-insensitively so a future README author cannot evade the check by
# capitalisation. If a legitimate use of any of these phrases must appear
# (e.g. a quoted ADR excerpt that mentions --no-verify in passing), the
# README author should phrase it without triggering the literal patterns
# below; this verifier's job is to keep escape hatches out of policy
# documentation.
forbidden_patterns=(
  '--no-verify'
  'no-verify'
  'skip the hook'
  'bypass the hook'
  'disable the hook'
  'core.hooksPath ""'
  "core.hooksPath ''"
  'GIT_HOOKS_PATH='
)

for pat in "${forbidden_patterns[@]}"; do
  if grep -qiF -- "$pat" "$README"; then
    printf 'fail: %s contains bypass language %q; documentation must not surface escape hatches that undo the gate.\n' "$README" "$pat" >&2
    failures=$((failures + 1))
  fi
done

if [[ "$failures" -ne 0 ]]; then
  printf '\ngithooks_setup_documented_in_readme: FAIL (%d invariant(s) violated)\n' "$failures" >&2
  exit 1
fi

printf 'githooks_setup_documented_in_readme: ok (%s contains setup command, no bypass language)\n' "$README"
exit 0
