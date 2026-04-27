#!/usr/bin/env bash
# =============================================================================
# asset_consumer_minimum.sh -- Verify every asset declares >= 1 consumer.
# =============================================================================
#
# Story 27 (ADR-0007 decision 4 + decision 5 corollary): the asset schema's
# `minItems: 1` invariant on `current_consumers:` is enforced at
# single-file load time by the asset schema itself, but a freshly-
# authored asset that simply OMITS the `current_consumers:` key
# entirely passes any per-file check that does not require the field.
# This verifier runs at corpus boundary: it enumerates every asset
# YAML under `assets/` and refuses with a non-zero exit if any
# asset's `current_consumers:` is absent, empty, or `[]`. Catches the
# orphan-asset shape the asset schema's `description` already names
# as a defect ("An asset with zero current_consumers is an orphan and
# must be deleted or adopted") at the boundary that schema validation
# alone cannot.
#
# Contract
# --------
#   asset_consumer_minimum.sh [--help]
#
#   Inputs:
#     (none)  -- the verifier walks `assets/` from the repo root.
#
#   Exit codes:
#     0  every asset has at least one consumer; the corpus self-test
#        round-trip also passes (synthetic orphan is detected).
#     1  one or more orphan assets detected (corpus check failed),
#        OR the self-test round-trip failed (the orphan-detection
#        function does not actually flag the synthetic orphan),
#        OR a runtime dependency is missing.
#     2  usage error.
#
# Replacement
# -----------
# Phase-1 bootstrap, mirroring `scripts/agentic-search.sh`'s posture.
# Once the Rust workspace ships an `agentic` binary, an
# `agentic verify asset-consumers` (or similar) subcommand subsumes
# this verifier with a schema-aware parser and proper error typing.
# Delete this file at that point and migrate callers.
#
# Dependencies
# ------------
# Requires `python3` with PyYAML (the standard WSL/Linux/macOS
# baseline). The orphan-detection contract requires distinguishing
# "field absent", "field present but empty array", and "field present
# with N>=1 entries" across both flow-style (`current_consumers: []`)
# and block-style (`current_consumers:\n - foo`) YAML shapes -- a
# distinction the awk parser in scripts/agentic-search.sh cannot make
# portably. PyYAML's safe loader handles both shapes uniformly. If
# python3 or PyYAML is missing the verifier exits 1 with a diagnostic.
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

print_usage() {
  cat <<'EOF'
Usage: asset_consumer_minimum.sh [--help]

Walk every asset YAML under assets/ and refuse with a non-zero
exit if any asset's current_consumers: is absent, empty, or [].

Options:
  --help, -h    Show this help.

Exit codes:
  0  every asset has at least one consumer (and the self-test passes)
  1  one or more orphan assets detected, self-test failed, or a
     runtime dependency (yq) is missing
  2  usage error
EOF
}

die_runtime() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

# Walk up from a starting directory until a `.git` entry appears.
# Mirrors `scripts/agentic-search.sh`'s find_repo_root.
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

# Parse `current_consumers:` from an asset YAML and emit one of:
#   ABSENT       -- field is missing entirely
#   EMPTY        -- field present but `[]` or zero entries
#   COUNT:<n>    -- field present with n>=1 entries
#
# Uses `yq` exclusively. The reason this script does not fall back to
# awk: the schema's minItems:1 contract requires distinguishing the
# absent / empty-array / non-empty-array states, and the awk parser
# in scripts/agentic-search.sh is built for scalar extraction only --
# it cannot reliably classify array cardinality across the flow-style
# (`current_consumers: []`) and block-style (`current_consumers:\n -
# foo`) YAML shapes the corpus uses interchangeably.
classify_consumers() {
  local file="$1"
  # PyYAML safe-loads the asset YAML and emits one of three sentinels:
  # ABSENT (field missing), EMPTY (field present but []/null), or
  # COUNT:<n> (n>=1 entries). Handles flow-style and block-style
  # uniformly via PyYAML's safe_load.
  python3 - "$file" <<'PY' || die_runtime "python3/PyYAML failed to parse $1"
import sys, yaml
try:
    with open(sys.argv[1], "r", encoding="utf-8") as fh:
        data = yaml.safe_load(fh) or {}
except Exception as exc:
    sys.stderr.write(f"parse-error: {exc}\n")
    sys.exit(1)
if not isinstance(data, dict):
    print("ABSENT")
    sys.exit(0)
if "current_consumers" not in data:
    print("ABSENT")
    sys.exit(0)
val = data["current_consumers"]
if val is None or (isinstance(val, list) and len(val) == 0):
    print("EMPTY")
    sys.exit(0)
if isinstance(val, list):
    print(f"COUNT:{len(val)}")
    sys.exit(0)
sys.stderr.write(f"unexpected current_consumers shape: {type(val).__name__}\n")
sys.exit(1)
PY
}

# Walk an asset directory and emit diagnostics for orphans.
# Returns 0 on a clean run (no orphans), 1 if any orphan was found.
check_asset_dir() {
  local asset_dir="$1"
  local found_orphan=0

  if [[ ! -d "$asset_dir" ]]; then
    die_runtime "asset directory not found: $asset_dir"
  fi

  shopt -s nullglob globstar
  local files=("$asset_dir"/**/*.yml)
  shopt -u globstar
  shopt -u nullglob

  if [[ ${#files[@]} -eq 0 ]]; then
    die_runtime "no asset YAMLs found under $asset_dir; expected at least one"
  fi

  local f cls
  for f in "${files[@]}"; do
    cls="$(classify_consumers "$f")"
    case "$cls" in
      ABSENT)
        printf 'orphan: %s -- current_consumers field is absent (schema requires minItems: 1)\n' "$f" >&2
        found_orphan=1
        ;;
      EMPTY)
        printf 'orphan: %s -- current_consumers is the empty array (schema requires minItems: 1)\n' "$f" >&2
        found_orphan=1
        ;;
      COUNT:*)
        # Healthy.
        ;;
      *)
        die_runtime "internal: classify_consumers returned unrecognised verdict $cls for $f"
        ;;
    esac
  done

  if [[ "$found_orphan" -eq 1 ]]; then
    return 1
  fi
  return 0
}

# Self-test: synthesise an orphan asset under a tempdir and assert
# `check_asset_dir` flags it. Runs after the live corpus pass so a
# real corpus orphan is reported first; the self-test is a contract
# check on the verifier's own logic, not a substitute for it.
self_test() {
  local tmpdir
  tmpdir="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "rm -rf '$tmpdir'" RETURN

  local orphan="$tmpdir/orphan.yml"
  cat >"$orphan" <<'EOF'
name: scratch-orphan
description: |
  Synthetic orphan asset authored in-process for the
  asset_consumer_minimum.sh self-test. The current_consumers field
  is deliberately empty; the verifier must flag this file.
current_consumers: []
EOF

  # Run the check function on the synthetic dir; expect non-zero.
  if check_asset_dir "$tmpdir" >/dev/null 2>&1; then
    printf 'self-test failed: check_asset_dir returned 0 (success) on a directory containing a synthetic orphan; the orphan-detection logic is broken\n' >&2
    return 1
  fi
  return 0
}

# ---------------------------------------------------------------------------
# Arg parsing
# ---------------------------------------------------------------------------

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

# ---------------------------------------------------------------------------
# Locate repo root + asset dir
# ---------------------------------------------------------------------------

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if ! REPO_ROOT="$(find_repo_root "$PWD")"; then
  if ! REPO_ROOT="$(find_repo_root "$script_dir")"; then
    die_runtime "could not find repo root (.git) from CWD or script dir"
  fi
fi

ASSET_DIR="$REPO_ROOT/assets"

# ---------------------------------------------------------------------------
# Dependency check: python3 with PyYAML must be available.
# ---------------------------------------------------------------------------

if ! command -v python3 >/dev/null 2>&1; then
  die_runtime "python3 is required to parse current_consumers reliably; install python3 and re-run"
fi
if ! python3 -c 'import yaml' >/dev/null 2>&1; then
  die_runtime "PyYAML is required (python3 -c 'import yaml' failed); install python3-yaml and re-run"
fi

# ---------------------------------------------------------------------------
# Main: corpus check, then self-test.
# ---------------------------------------------------------------------------

corpus_status=0
if ! check_asset_dir "$ASSET_DIR"; then
  corpus_status=1
fi

self_test_status=0
if ! self_test; then
  self_test_status=1
fi

if [[ "$corpus_status" -ne 0 || "$self_test_status" -ne 0 ]]; then
  exit 1
fi

printf 'asset_consumer_minimum: ok (every asset under %s declares >= 1 consumer; self-test passed)\n' "$ASSET_DIR"
exit 0
