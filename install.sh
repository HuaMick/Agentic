#!/usr/bin/env bash
# Local install for the `agentic` CLI.
#
# Usage:
#   ./install.sh             # install the Rust binary into ~/.cargo/bin
#   ./install.sh --docker    # build the `agentic:local` Docker image
#   ./install.sh --help      # show this help
#
# Idempotent: safe to re-run after `git pull`.

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: ./install.sh [--docker|--help]

Without flags: installs the agentic CLI via `cargo install` into
~/.cargo/bin. Prompts to run rustup-init.sh if cargo is missing.

--docker: builds the local Docker image `agentic:local`. Does not
          auto-run the container — prints the run command instead.
EOF
}

MODE="local"
case "${1:-}" in
    --docker) MODE="docker" ;;
    --help|-h) usage; exit 0 ;;
    "") ;;
    *) echo "error: unknown argument: $1" >&2; usage >&2; exit 2 ;;
esac

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_ROOT"

if [ ! -f "crates/agentic-cli/Cargo.toml" ]; then
    echo "error: must be run from the Agentic repo root (missing crates/agentic-cli/Cargo.toml)" >&2
    exit 1
fi

install_rustup_prompt() {
    echo "cargo not found on PATH." >&2
    echo "This installs the Rust toolchain from https://sh.rustup.rs" >&2
    printf "Install Rust now via rustup? [y/N] " >&2
    local reply=""
    read -r reply || true
    case "$reply" in
        y|Y|yes|YES)
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
            # shellcheck disable=SC1091
            . "$HOME/.cargo/env"
            ;;
        *)
            echo "aborted: cargo required for local install. Re-run with --docker to skip the toolchain." >&2
            exit 1
            ;;
    esac
}

ensure_bashrc_cargo_env() {
    local line='. "$HOME/.cargo/env"'
    local rc="$HOME/.bashrc"
    [ -f "$rc" ] || return 0
    if ! grep -Fqx "$line" "$rc"; then
        printf '\n%s\n' "$line" >> "$rc"
        echo "appended cargo env to ~/.bashrc"
    fi
}

install_local() {
    if ! command -v cargo >/dev/null 2>&1; then
        # shellcheck disable=SC1091
        [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
    fi
    if ! command -v cargo >/dev/null 2>&1; then
        install_rustup_prompt
    fi

    echo "==> cargo install --path crates/agentic-cli --locked --force"
    cargo install --path crates/agentic-cli --locked --force

    ensure_bashrc_cargo_env

    echo "==> verifying agentic --help"
    agentic --help >/dev/null

    cat <<EOF

agentic installed: $(command -v agentic)

Next steps:
  agentic stories health        # story-health dashboard
  agentic uat <id> --verdict <pass|fail>
EOF
}

install_docker() {
    if ! command -v docker >/dev/null 2>&1; then
        echo "error: docker not found on PATH" >&2
        exit 1
    fi

    echo "==> docker build -t agentic:local ."
    docker build -t agentic:local .

    cat <<'EOF'

Image built: agentic:local

Run via the repo wrapper (hides bind-mount + volume flags):
  ./bin/agentic-docker stories health

Or directly:
  docker run --rm -v "$PWD":/workspace \
    -v agentic-store:/root/.local/share/agentic \
    agentic:local stories health
EOF
}

case "$MODE" in
    local) install_local ;;
    docker) install_docker ;;
esac
