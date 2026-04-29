#!/usr/bin/env bash
# =============================================================================
# githooks_pre_commit_exists_and_executable.sh -- Story 29, acceptance test 1.
# =============================================================================
#
# Story 29: pre-commit hook refuses commits whose corpus state would render as
# drift. This verifier proves the structural prerequisites of the hook script
# itself: the file is present at the repo root under `.githooks/pre-commit`,
# is tracked in git, has the user-execute mode bit set, declares a recognised
# shebang, and is encoded in LF line endings.
#
# Without this verifier, a clone could ship the hook in a non-executable state
# or with CRLF line endings, and `git config core.hooksPath .githooks` would
# silently no-op -- the gate would be a paper claim and the forged-promotion
# shape this story exists to prevent would still land.
#
# Contract
# --------
#   githooks_pre_commit_exists_and_executable.sh [--help]
#
#   Inputs:
#     (none)  -- the verifier walks the repo root.
#
#   Exit codes:
#     0  the hook file exists, is tracked, is executable, has a recognised
#        shebang, and uses LF line endings.
#     1  one or more of the above invariants failed.
#     2  usage error.
# =============================================================================

set -euo pipefail

print_usage() {
  cat <<'EOF'
Usage: githooks_pre_commit_exists_and_executable.sh [--help]

Assert the tracked .githooks/pre-commit script exists at the repo root,
is executable, declares a recognised shebang, and is LF-encoded.

Options:
  --help, -h    Show this help.

Exit codes:
  0  all invariants hold
  1  one or more invariants failed
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

HOOK="$REPO_ROOT/.githooks/pre-commit"
failures=0

# Invariant 1: file is present and is a regular file.
if [[ ! -e "$HOOK" ]]; then
  printf 'fail: %s does not exist; story 29 requires a tracked pre-commit hook at this path\n' "$HOOK" >&2
  failures=$((failures + 1))
elif [[ ! -f "$HOOK" ]]; then
  printf 'fail: %s exists but is not a regular file\n' "$HOOK" >&2
  failures=$((failures + 1))
fi

# Invariant 2: file is tracked by git.
if [[ -e "$HOOK" ]]; then
  if ! ( cd -- "$REPO_ROOT" && git ls-files --error-unmatch ".githooks/pre-commit" >/dev/null 2>&1 ); then
    printf 'fail: .githooks/pre-commit is not tracked by git (git ls-files --error-unmatch failed); a hook only ships to other clones if it is tracked\n' >&2
    failures=$((failures + 1))
  fi
fi

# Invariant 3: user-execute bit is set.
if [[ -e "$HOOK" && ! -x "$HOOK" ]]; then
  printf 'fail: %s is not executable (test -x); core.hooksPath would silently no-op without the execute bit\n' "$HOOK" >&2
  failures=$((failures + 1))
fi

# Invariant 4: recognised shebang on line 1.
if [[ -f "$HOOK" ]]; then
  first_line="$(head -n 1 -- "$HOOK" | tr -d '\r')"
  case "$first_line" in
    '#!/usr/bin/env bash'|'#!/bin/bash'|'#!/bin/sh'|'#!/usr/bin/env python3')
      ;;
    *)
      printf 'fail: %s does not start with a recognised shebang; got: %q (expected #!/usr/bin/env bash, #!/bin/bash, #!/bin/sh, or #!/usr/bin/env python3)\n' "$HOOK" "$first_line" >&2
      failures=$((failures + 1))
      ;;
  esac
fi

# Invariant 5: LF line endings (no CR characters in the file).
if [[ -f "$HOOK" ]]; then
  if grep -qU $'\r' -- "$HOOK"; then
    printf 'fail: %s contains CR characters (CRLF line endings); git for Windows silently fails to interpret CRLF shebang lines\n' "$HOOK" >&2
    failures=$((failures + 1))
  fi
fi

if [[ "$failures" -ne 0 ]]; then
  printf '\ngithooks_pre_commit_exists_and_executable: FAIL (%d invariant(s) violated)\n' "$failures" >&2
  exit 1
fi

printf 'githooks_pre_commit_exists_and_executable: ok (%s present, tracked, executable, recognised shebang, LF-encoded)\n' "$HOOK"
exit 0
