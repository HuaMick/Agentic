# CLI Installation Guide

The `agentic` CLI is the primary interface for running UAT verdicts and viewing
story health. It's a Rust binary installed via cargo.

## Quick install (recommended)

From the repo root on WSL:

```bash
./install.sh
```

This verifies `cargo` is available (prompts to install `rustup` if not),
runs `cargo install --path crates/agentic-cli --locked --force`, ensures
`~/.cargo/env` is sourced by your `~/.bashrc`, and verifies `agentic --help`
works. Safe to re-run after `git pull`.

For a containerised install (no local Rust toolchain required):

```bash
./install.sh --docker
```

This builds the `agentic:local` Docker image. Run the CLI via the
`bin/agentic-docker` wrapper (bind-mounts the current directory and a named
volume for the store):

```bash
./bin/agentic-docker stories health
./bin/agentic-docker uat 1 --verdict pass
```

Put `bin/agentic-docker` on your PATH to use it like the native binary.

## From source (manual)

If you'd rather drive cargo yourself, install inside WSL where the Rust
toolchain lives:

```bash
cd /home/code/Agentic
source ~/.cargo/env
cargo install --path crates/agentic-cli --locked --force
```

This installs to `~/.cargo/bin/agentic`. Verify:

```bash
command -v agentic          # expect: /home/<user>/.cargo/bin/agentic
file "$(command -v agentic)" # expect: ELF 64-bit executable
agentic stories health       # expect: dashboard with story statuses
```

## Windows Access (via WSL wrapper)

The binary runs inside WSL, but Windows-side tools (including Claude Code
running from Windows) can invoke it through a wrapper script.

### Option 1: Direct `wsl` invocation

From Windows (PowerShell or cmd):

```cmd
wsl -e bash -lc "cd /home/code/Agentic && agentic stories health"
wsl -e bash -lc "cd /home/code/Agentic && agentic uat 1 --verdict pass"
```

### Option 2: Windows batch wrapper (recommended)

Create `agentic.cmd` in a directory on your Windows PATH (e.g.,
`C:\Users\<you>\bin\`):

```cmd
@echo off
wsl -e bash -lc "cd /home/code/Agentic && agentic %*"
```

Then invoke from anywhere on Windows:

```cmd
agentic stories health
agentic uat 1 --verdict pass
```

### Option 3: PowerShell function

Add to your PowerShell profile (`$PROFILE`):

```powershell
function agentic {
    wsl -e bash -lc "cd /home/code/Agentic && agentic $($args -join ' ')"
}
```

## Data location

The store lives at `~/.local/share/agentic/store` (Linux path). This is
accessible from Windows at:

```
\\wsl.localhost\Ubuntu\home\<user>\.local\share\agentic\store
```

## Updating the CLI

After pulling changes to `crates/agentic-cli/`:

```bash
./install.sh
```

Or manually:

```bash
cd /home/code/Agentic
source ~/.cargo/env
cargo install --path crates/agentic-cli --locked --force
```

## Troubleshooting

**"command not found: agentic"**
- Ensure `~/.cargo/bin` is on PATH. Run `source ~/.cargo/env` or add it to
  your shell rc file.

**Permission errors**
- The binary should be owned by your user, not root. If installed as root,
  reinstall as your normal user.

**Windows wrapper returns nothing**
- Ensure WSL is running and the workspace path exists.
- Test with `wsl ls /home/code/Agentic` first.
