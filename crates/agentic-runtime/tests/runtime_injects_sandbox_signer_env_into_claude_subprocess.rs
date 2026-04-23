//! Story 18 acceptance test: `ClaudeCodeRuntime` injects
//! `AGENTIC_SIGNER=sandbox:<model>@<run_id>` into the spawned claude
//! subprocess's environment, overriding any outer-shell export.
//!
//! Justification (from stories/18.yml acceptance.tests[9]):
//!   Proves the sandbox convention at the runtime
//!   boundary: when `ClaudeCodeRuntime` (story 19) spawns
//!   a claude subprocess for a run with `run_id =
//!   "a1b2c3"` against a configured model
//!   `"claude-sonnet-4-6"`, the child process's
//!   environment contains `AGENTIC_SIGNER=sandbox:claude-
//!   sonnet-4-6@run-a1b2c3` exactly. The value is
//!   composed by the runtime from its own inputs (model,
//!   run id); the caller does NOT pass it in as a flag.
//!   Inside the sandbox, any `agentic uat` or `agentic
//!   ci record` invocation resolves the signer from this
//!   env var via tier 2 — no special-case code path for
//!   agents. Without this, the compositional story the
//!   outcome promises (one resolver, one chain, one
//!   convention) degrades into two parallel paths (human
//!   path + agent-special path), and the sandbox
//!   convention is a comment rather than a behaviour.
//!
//! Red today: compile-red via the missing `ClaudeCodeRuntime` /
//! `RunConfig` / `SpawnOutcome` symbols in `agentic_runtime` — story
//! 19 has not yet landed the runtime type, and story 18 does not
//! author it; the test fails at the `use` line.

use agentic_runtime::{ClaudeCodeRuntime, RunConfig, SpawnOutcome};

#[test]
fn runtime_injects_sandbox_signer_env_into_claude_subprocess() {
    let cfg = RunConfig {
        model: "claude-sonnet-4-6".to_string(),
        run_id: "a1b2c3".to_string(),
    };

    // Sanity: an outer-shell export must NOT leak into the child's
    // signer — the runtime composes its own value and overrides.
    std::env::set_var("AGENTIC_SIGNER", "outer-shell-person@example.com");

    let runtime = ClaudeCodeRuntime::new();
    // Inspection harness: the runtime must expose a way to observe
    // what env it WOULD inject into the child process without having
    // to actually fork claude. The name `spawn_env_for` is the
    // simplest shape that lets this test assert; the runtime is free
    // to keep the real `spawn` around it.
    let spawn = runtime
        .prepare_spawn(&cfg)
        .expect("prepare_spawn must succeed on a valid config");

    // The child env must contain the composed sandbox signer.
    let child_signer = spawn
        .child_env("AGENTIC_SIGNER")
        .expect("runtime must set AGENTIC_SIGNER on the child process env");

    assert_eq!(
        child_signer, "sandbox:claude-sonnet-4-6@run-a1b2c3",
        "runtime must inject the composed sandbox signer into the child env; \
         outer shell's AGENTIC_SIGNER must NOT leak through"
    );
    assert_ne!(
        child_signer, "outer-shell-person@example.com",
        "outer-shell AGENTIC_SIGNER must be overridden by the runtime-composed value"
    );

    // The SpawnOutcome exists so the type is load-bearing — a future
    // refactor that drops the type would silently pass this assert.
    let _ = std::any::type_name::<SpawnOutcome>();

    // Cleanup.
    std::env::remove_var("AGENTIC_SIGNER");
}
