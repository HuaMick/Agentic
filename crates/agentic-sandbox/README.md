# agentic-sandbox

## What this crate is

Per-session isolation. Defines the `Sandbox` trait and provides day-one `ProcessSandbox` impl: per-session tmp workspace, scrubbed environment variables, ulimit restrictions, and restricted working directory.

## Why it's a separate crate

Inspired by the broader industry pattern (Firecracker microVMs, gVisor, container-per-agent) — but we start light. Having a trait from day one means upgrading from process-level to container or microVM isolation is additive, not a rewrite. No caller code changes.

The legacy system had no isolation; an agent misbehaving could corrupt shared state. This crate is the structural answer.

## Public API sketch

```rust
#[async_trait]
pub trait Sandbox {
    async fn prepare(&self, req: SandboxRequest) -> Result<SandboxSession>;
    async fn exec(&self, session: &SandboxSession, cmd: Command) -> Result<ExitStatus>;
    async fn teardown(&self, session: SandboxSession) -> Result<()>;
}

pub struct SandboxSession {
    pub id: Uuid,
    pub workspace: PathBuf,
    pub env: HashMap<String, String>,
    // ... impl-specific isolation handle
}

pub struct ProcessSandbox { /* ... */ }
impl Sandbox for ProcessSandbox { /* per-session tmp dir + scrubbed env + ulimits */ }
```

## Dependencies

- Depends on: `tokio`, `cap-std` (for capability-based filesystem access), `rlimit`
- Depended on by: `agentic-runtime`

## Design decisions

- **Start with process-level isolation.** Each session gets a fresh temp workspace, an environment with sensitive vars scrubbed (no GITHUB_TOKEN, no home-dir secrets unless explicitly allowed), and resource limits (CPU, memory, open files). This is maybe 2× better than the legacy's "just spawn a subprocess."
- **Container/microVM impls are future.** `_deferred/` isn't the right home — they'll live here as additional impls (`ContainerSandbox`, `MicroVMSandbox`) when built. The trait stays the same.
- **Capability-based FS via `cap-std`.** Subprocess sees only its workspace dir; attempts to read/write outside fail at the syscall level. Rust-first, no OS-specific tricks.
- **No network isolation at this layer — yet.** Claude needs to talk to Anthropic; we don't block that. If we need network allowlists later, add it via a separate impl or a future `NetworkPolicy` trait.

## Open questions

- Do we enforce read-only access to the repo root (so agents can't accidentally edit checked-in files outside their workspace)? Leaning: yes, agents work in a worktree-like copy.
- How do containerized impls handle `claude` subprocess spawning? The `claude` binary needs to be available inside the container + have access to the user's credentials. May push us toward a credentials-proxy design.
- What exact ulimits? Start conservative: 2GB RAM, 50% CPU share, 1024 open files.

## Stress/verify requirements

- An agent cannot write outside its sandbox workspace (attempt produces clean error, not host-FS damage).
- A runaway agent (infinite loop, fork bomb) is killed by ulimits within 10 seconds.
- Concurrent sandboxes don't share state (tmp dirs, env, locks).
- Teardown always runs, even on caller panic (RAII guard).
