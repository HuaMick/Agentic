#!/usr/bin/env bash
# =============================================================================
# agentic-search.sh — Search story YAML files by term(s) across selected fields.
# =============================================================================
#
# Purpose
# -------
# Phase-1 bootstrap tool. Returns matching story IDs + titles for agents and
# humans so work can begin before the full Rust CLI exists. The stories/
# directory holds user stories as YAML files validated against
# schemas/story.schema.json; this script searches the fields that matter for
# routing work: title, outcome, guidance, and acceptance.uat.
#
# Contract
# --------
#   agentic-search.sh <terms...> [--field outcome|title|guidance|uat|all]
#                                [--json] [--help]
#
#   Inputs:
#     <terms...>  Positional words (quote multi-word phrases). Case-insensitive.
#                 At least one term is required.
#     --field F   Restrict search to a single field. Default: all (searches
#                 title + outcome + guidance + acceptance.uat).
#     --json      Machine-readable JSON array output.
#                 Default is human format: "<id>  <title>  [matched_fields]".
#
#   Outputs:
#     stdout:  Hits, ranked by (# terms matched DESC, id ASC).
#     stderr:  Warnings (e.g. fallback parser in use) and errors.
#
#   Exit codes:
#     0  Success (even when zero hits).
#     1  Unrecoverable runtime error (e.g. cannot find repo root).
#     2  Usage error (missing terms, bad flag, unknown field).
#
# Replacement
# -----------
# This script is a temporary bootstrap. Once the Rust workspace ships an
# `agentic` binary, `agentic search` will subsume this functionality with a
# proper schema-aware parser, dependency-graph awareness, and structured
# output. At that point this file should be deleted and callers migrated.
#
# Dependencies
# ------------
# Prefers `yq` (v4, mikefarah/yq). Falls back to grep/awk/sed if yq is not
# installed; fallback is awareness-limited about YAML (it handles `|` block
# scalars via indentation tracking) and prints a one-shot warning on stderr.
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

print_usage() {
  cat <<'EOF'
Usage: agentic-search.sh <terms...> [--field outcome|title|guidance|uat|all] [--json]

Searches stories/*.yml in the current repo. Matches are case-insensitive.

Options:
  --field F     Restrict search to one field. Default: all.
                Valid: outcome, title, guidance, uat, all.
  --json        Emit JSON array: [{"id": N, "title": "...", "matched_fields": [...]}, ...]
  --help, -h    Show this help.

Exit codes:
  0  success (zero hits is still success)
  1  runtime error
  2  usage error
EOF
}

die_usage() {
  printf 'error: %s\n\n' "$1" >&2
  print_usage >&2
  exit 2
}

die_runtime() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

# Walk up from a starting directory until we find a .git entry (file or dir).
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

# JSON-escape a string for use inside a JSON string literal.
json_escape() {
  local s=$1
  s=${s//\\/\\\\}
  s=${s//\"/\\\"}
  s=${s//$'\n'/\\n}
  s=${s//$'\r'/\\r}
  s=${s//$'\t'/\\t}
  printf '%s' "$s"
}

# Lower-case a string using awk (portable, avoids bash 4 ${var,,}).
to_lower() {
  awk 'BEGIN{ s=ARGV[1]; print tolower(s); exit }' "$1"
}

# ---------------------------------------------------------------------------
# YAML field extraction (fallback parser — no yq available)
#
# Handles:
#   scalar:        key: value
#   quoted scalar: key: "value" or key: 'value'
#   block scalar:  key: | / key: |- / key: |+ / key: > with indented body
#
# Does NOT handle: flow scalars spanning multiple lines, anchors, aliases,
# merge keys. That's acceptable for Phase-1 stories (kept simple by schema).
# `uat` lives under `acceptance:` as a nested key, so we parse with awareness
# of a shallow nesting level.
# ---------------------------------------------------------------------------

# extract_block_scalar FILE KEY_REGEX MIN_INDENT
#   Emits the value of the matched key, whether it's a same-line scalar
#   (plain, single-quoted, or double-quoted) or a `|`/`>` block body whose
#   indentation is inferred from the first body line.
extract_block_scalar() {
  # Args: file, key_regex, min_indent_required (0 for root, >0 for nested)
  local file="$1" key_regex="$2" min_indent="$3"
  awk -v key_re="$key_regex" -v min_indent="$min_indent" '
    function leading_spaces(s,   i, n) {
      n = 0
      for (i = 1; i <= length(s); i++) {
        if (substr(s, i, 1) == " ") n++; else break
      }
      return n
    }
    BEGIN { state = 0 }   # 0 = seeking, 1 = capturing
    {
      line = $0
      sub(/\r$/, "", line)
      indent = leading_spaces(line)
      stripped = line; sub(/^ +/, "", stripped)

      if (state == 0) {
        if (indent != min_indent) next
        if (stripped !~ key_re) next
        rest = stripped; sub(key_re, "", rest); sub(/^[[:space:]]+/, "", rest)
        if (rest ~ /^[|>][-+]?[[:space:]]*($|#)/) {
          state = 1
          key_indent = indent
          body_indent = -1
          next
        }
        # scalar on same line — quoted or plain
        if (rest ~ /^".*"$/) {
          v = substr(rest, 2, length(rest) - 2)
          gsub(/\\"/, "\"", v); gsub(/\\\\/, "\\", v)
          print v
        } else if (rest ~ /^'\''.*'\''$/) {
          v = substr(rest, 2, length(rest) - 2)
          print v
        } else {
          sub(/[[:space:]]+#.*$/, "", rest)
          print rest
        }
        exit
      }

      # state == 1: capturing block body
      if (stripped == "") { print ""; next }
      if (body_indent < 0) {
        if (indent <= key_indent) { exit }
        body_indent = indent
      }
      if (indent < body_indent) { exit }
      # Emit the line with body_indent stripped.
      print substr(line, body_indent + 1)
    }
  ' "$file"
}

# Unified extractor using the dedicated block-scalar function.
get_field() {
  local file="$1" field="$2"
  case "$field" in
    title|outcome|guidance|id|status)
      extract_block_scalar "$file" "^${field}[[:space:]]*:" 0
      ;;
    uat)
      # uat is nested under acceptance. We need to find the acceptance block
      # and then look for "uat:" inside it. Use a two-step approach: emit
      # the acceptance sub-document, then feed it back through with min_indent=2.
      # Simpler: grep the whole file with awareness of nesting.
      awk '
        function leading_spaces(s,   i, n) {
          n = 0
          for (i = 1; i <= length(s); i++) {
            if (substr(s, i, 1) == " ") n++; else break
          }
          return n
        }
        BEGIN { in_acc = 0; acc_indent = -1; state = 0 }
        {
          line = $0; sub(/\r$/, "", line)
          indent = leading_spaces(line)
          stripped = line; sub(/^ +/, "", stripped)
          if (state == 1) {
            # Capturing uat block body.
            if (stripped == "") { print ""; next }
            if (body_indent < 0) {
              if (indent <= uat_key_indent) { exit }
              body_indent = indent
            }
            if (indent < body_indent) { exit }
            print substr(line, body_indent + 1)
            next
          }
          if (!in_acc) {
            if (indent == 0 && stripped ~ /^acceptance[[:space:]]*:[[:space:]]*$/) {
              in_acc = 1; acc_indent = 0
            }
            next
          }
          # Inside acceptance.
          if (stripped == "" || stripped ~ /^#/) next
          if (indent <= acc_indent) {
            # left acceptance
            in_acc = 0; next
          }
          # Looking for uat key at any depth > acc_indent (typically 2).
          if (stripped !~ /^uat[[:space:]]*:/) next
          rest = stripped; sub(/^uat[[:space:]]*:/, "", rest); sub(/^[[:space:]]+/, "", rest)
          if (rest ~ /^[|>][-+]?[[:space:]]*($|#)/) {
            state = 1
            uat_key_indent = indent
            body_indent = -1
            next
          }
          if (rest ~ /^".*"$/) {
            v = substr(rest, 2, length(rest) - 2)
            gsub(/\\"/, "\"", v); gsub(/\\\\/, "\\", v)
            print v; exit
          } else if (rest ~ /^'\''.*'\''$/) {
            v = substr(rest, 2, length(rest) - 2)
            print v; exit
          } else {
            sub(/[[:space:]]+#.*$/, "", rest)
            print rest; exit
          }
        }
      ' "$file"
      ;;
    *)
      return 0
      ;;
  esac
}

# yq-based extractor (v4 syntax).
get_field_yq() {
  local file="$1" field="$2"
  case "$field" in
    title|outcome|guidance|id|status)
      yq -r ".${field} // \"\"" "$file" 2>/dev/null
      ;;
    uat)
      yq -r '.acceptance.uat // ""' "$file" 2>/dev/null
      ;;
  esac
}

# Dispatch based on whether yq is present.
get_field_any() {
  if $USE_YQ; then
    get_field_yq "$1" "$2"
  else
    get_field "$1" "$2"
  fi
}

# ---------------------------------------------------------------------------
# Arg parsing
# ---------------------------------------------------------------------------

FIELD="all"
JSON=false
TERMS=()

while (($#)); do
  case "$1" in
    --help|-h)
      print_usage
      exit 0
      ;;
    --json)
      JSON=true
      shift
      ;;
    --field)
      [[ $# -ge 2 ]] || die_usage "--field requires a value"
      FIELD="$2"
      shift 2
      ;;
    --field=*)
      FIELD="${1#--field=}"
      shift
      ;;
    --)
      shift
      while (($#)); do TERMS+=("$1"); shift; done
      ;;
    -*)
      die_usage "unknown flag: $1"
      ;;
    *)
      TERMS+=("$1")
      shift
      ;;
  esac
done

case "$FIELD" in
  all|title|outcome|guidance|uat) ;;
  *) die_usage "invalid --field: $FIELD (expected: all|title|outcome|guidance|uat)" ;;
esac

if [[ ${#TERMS[@]} -eq 0 ]]; then
  die_usage "at least one search term is required"
fi

# ---------------------------------------------------------------------------
# Locate repo + stories dir
# ---------------------------------------------------------------------------

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# Prefer repo root from CWD (matches `git` behaviour — search the repo the
# user is in, not the one the script happens to live in). Fall back to the
# script's own directory so installs outside a repo still work when invoked
# from anywhere inside the Agentic checkout.
if ! REPO_ROOT="$(find_repo_root "$PWD")"; then
  if ! REPO_ROOT="$(find_repo_root "$script_dir")"; then
    die_runtime "could not find repo root (.git) from CWD or script dir"
  fi
fi

STORIES_DIR="$REPO_ROOT/stories"
if [[ ! -d "$STORIES_DIR" ]]; then
  die_runtime "stories directory not found: $STORIES_DIR"
fi

# ---------------------------------------------------------------------------
# yq detection
# ---------------------------------------------------------------------------

if command -v yq >/dev/null 2>&1; then
  USE_YQ=true
else
  USE_YQ=false
  printf 'warning: yq not installed; falling back to grep/awk parser (handles `|` block scalars only)\n' >&2
fi

# ---------------------------------------------------------------------------
# Fields to search given --field
# ---------------------------------------------------------------------------

if [[ "$FIELD" == "all" ]]; then
  SEARCH_FIELDS=(title outcome guidance uat)
else
  SEARCH_FIELDS=("$FIELD")
fi

# ---------------------------------------------------------------------------
# Main scan
# ---------------------------------------------------------------------------

# Lower-case terms up front.
LC_TERMS=()
for t in "${TERMS[@]}"; do
  LC_TERMS+=("$(to_lower "$t")")
done

# Collect results as TSV: score\tid\ttitle\tmatched_fields_comma
results_tmp="$(mktemp)"
# shellcheck disable=SC2064
trap "rm -f '$results_tmp'" EXIT

shopt -s nullglob
yml_files=("$STORIES_DIR"/*.yml)
shopt -u nullglob

for file in "${yml_files[@]}"; do
  # Extract id + title (always needed for output).
  id="$(get_field_any "$file" id | head -n 1)"
  # Strip whitespace.
  id="${id#"${id%%[![:space:]]*}"}"
  id="${id%"${id##*[![:space:]]}"}"
  # Fallback: derive from filename if missing.
  if [[ -z "$id" ]]; then
    base="$(basename -- "$file" .yml)"
    # Accept plain "<N>.yml" or "story-NNNN-*.yml"
    if [[ "$base" =~ ^([0-9]+)$ ]]; then
      id="${BASH_REMATCH[1]}"
    elif [[ "$base" =~ ^story-0*([0-9]+)- ]]; then
      id="${BASH_REMATCH[1]}"
    else
      id="$base"
    fi
  fi
  # Strip leading zeros for numeric sort, but keep "0" itself.
  if [[ "$id" =~ ^[0-9]+$ ]]; then
    id=$((10#$id))
  fi

  title="$(get_field_any "$file" title | head -n 1)"

  # For each field to search, pull the text and check each term.
  matched_fields=()
  score=0
  for f in "${SEARCH_FIELDS[@]}"; do
    content="$(get_field_any "$file" "$f" || true)"
    lc_content="$(printf '%s' "$content" | awk '{print tolower($0)}')"
    field_matched=false
    for lct in "${LC_TERMS[@]}"; do
      if [[ "$lc_content" == *"$lct"* ]]; then
        field_matched=true
        score=$((score + 1))
      fi
    done
    if $field_matched; then
      matched_fields+=("$f")
    fi
  done

  if [[ $score -gt 0 ]]; then
    # Join matched_fields with commas.
    mf_joined=""
    for mf in "${matched_fields[@]}"; do
      mf_joined+="${mf},"
    done
    mf_joined="${mf_joined%,}"
    # TSV row: score \t id \t title \t matched_fields
    printf '%s\t%s\t%s\t%s\n' "$score" "$id" "$title" "$mf_joined" >>"$results_tmp"
  fi
done

# ---------------------------------------------------------------------------
# Sort and render
# ---------------------------------------------------------------------------

# Sort by score DESC (col 1), then id ASC (col 2, numeric when possible).
# We use -k1,1nr -k2,2n. Non-numeric ids will sort lexicographically under -n
# which is acceptable for our use-case (schema enforces integer id).
sorted="$(sort -t $'\t' -k1,1nr -k2,2n "$results_tmp" || true)"

if [[ -z "$sorted" ]]; then
  if $JSON; then
    printf '[]\n'
  fi
  exit 0
fi

if $JSON; then
  printf '['
  first=true
  while IFS=$'\t' read -r _score id title matched_fields; do
    if $first; then first=false; else printf ','; fi
    # Build matched_fields JSON array.
    mf_json="["
    mf_first=true
    IFS=',' read -r -a mf_arr <<<"$matched_fields"
    for mf in "${mf_arr[@]}"; do
      if $mf_first; then mf_first=false; else mf_json+=','; fi
      mf_json+="\"$(json_escape "$mf")\""
    done
    mf_json+="]"
    # Encode id: numeric if integer, else string.
    if [[ "$id" =~ ^-?[0-9]+$ ]]; then
      id_json="$id"
    else
      id_json="\"$(json_escape "$id")\""
    fi
    printf '{"id":%s,"title":"%s","matched_fields":%s}' \
      "$id_json" "$(json_escape "$title")" "$mf_json"
  done <<<"$sorted"
  printf ']\n'
else
  while IFS=$'\t' read -r _score id title matched_fields; do
    printf '%s  %s  [%s]\n' "$id" "$title" "$matched_fields"
  done <<<"$sorted"
fi

exit 0
