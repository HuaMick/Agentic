//! Story 19 acceptance test: `ClaudeCodeRuntime::new()` refuses to fall
//! back to any direct-API path when `ANTHROPIC_API_KEY` is set but
//! `claude` is missing from PATH. Returns
//! `RuntimeError::ClaudeSpawn { reason: ClaudeNotFound }`. Additionally
//! asserts the crate's `Cargo.toml` declares no direct-API HTTP
//! dependencies (`reqwest`, `anthropic-sdk-rust`, `rusty-anthropic`).
//!
//! Justification (from stories/19.yml acceptance.tests[2]):
//!   Proves ADR-0003's amendment clause survives a concrete
//!   attack surface: constructing a `ClaudeCodeRuntime` on a
//!   host where `ANTHROPIC_API_KEY` is set (but `claude` is
//!   not on PATH) returns a typed
//!   `RuntimeError::ClaudeSpawn { reason: ClaudeNotFound }`
//!   and NOT a fallback to any direct-API code path. There
//!   is no `reqwest` client in the runtime's dependency
//!   graph (a second assertion in this test reads
//!   `Cargo.toml` and asserts the absence of
//!   `anthropic-sdk-rust`, `rusty-anthropic`, and
//!   `reqwest` as direct dependencies of the runtime
//!   crate). Without this, the "never talk to
//!   api.anthropic.com directly" invariant relies on
//!   nobody adding a crate later and nobody adding an
//!   `if env::var("ANTHROPIC_API_KEY").is_ok()` branch
//!   later — both cheap edits that look reasonable to a
//!   well-meaning contributor.
//!
//! Red today: compile-red. `ClaudeCodeRuntime`, `RuntimeError`, and
//! the `ClaudeSpawn { reason }` variant do not yet exist. The
//! `ClaudeSpawnReason::ClaudeNotFound` sub-enum variant is similarly
//! unresolved.

use agentic_runtime::{ClaudeCodeRuntime, ClaudeSpawnReason, RuntimeError};
use std::fs;
use std::path::PathBuf;

#[test]
fn refuses_api_key_fallback_and_has_no_direct_api_http_deps() {
    // Attack surface (a): API key set, claude missing on PATH.
    // Save and clear PATH so that the constructor cannot find any
    // `claude` binary. Set ANTHROPIC_API_KEY to a value that WOULD
    // make a lazy "fall back to the API" refactor look attractive.
    let saved_path = std::env::var_os("PATH");
    let saved_key = std::env::var_os("ANTHROPIC_API_KEY");
    std::env::set_var("PATH", "");
    std::env::set_var(
        "ANTHROPIC_API_KEY",
        "sk-ant-api03-this-must-not-be-used-for-fallback",
    );

    let result = ClaudeCodeRuntime::new();

    // Restore env before any assertions so a failure leaves no
    // lasting damage to the test process.
    match saved_path {
        Some(v) => std::env::set_var("PATH", v),
        None => std::env::remove_var("PATH"),
    }
    match saved_key {
        Some(v) => std::env::set_var("ANTHROPIC_API_KEY", v),
        None => std::env::remove_var("ANTHROPIC_API_KEY"),
    }

    match result {
        Err(RuntimeError::ClaudeSpawn { reason }) => {
            // The reason must name the binary-lookup failure, not
            // some generic "io error" or "other".
            assert!(
                matches!(reason, ClaudeSpawnReason::ClaudeNotFound),
                "RuntimeError::ClaudeSpawn reason must be ClaudeNotFound on empty-PATH + no-claude host; got {reason:?}"
            );
        }
        Err(other) => panic!(
            "ClaudeCodeRuntime::new on empty-PATH host must return ClaudeSpawn {{ ClaudeNotFound }}; got {other:?}"
        ),
        Ok(_) => panic!(
            "ClaudeCodeRuntime::new on empty-PATH host must NOT succeed (would imply a direct-API fallback path)"
        ),
    }

    // Attack surface (b): static dependency-graph guard — the
    // runtime's own Cargo.toml MUST NOT declare any direct-API HTTP
    // crate. A future refactor that `cargo add reqwest` would make
    // this assertion fail even before the `fallback` branch is
    // written.
    let cargo_toml_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let cargo_toml = fs::read_to_string(&cargo_toml_path)
        .unwrap_or_else(|e| panic!("reading {cargo_toml_path:?}: {e}"));

    for forbidden in &["reqwest", "anthropic-sdk-rust", "rusty-anthropic"] {
        // Require a word-boundary-ish match: the crate name appears as
        // a `<name> =` TOML key on its own. A simple contains-check
        // catches casual additions; we also reject any `"<name>"`
        // shape to catch object-form deps.
        let key_form = format!("{forbidden} =");
        let table_form = format!("{forbidden} = {{");
        let quoted_form = format!("\"{forbidden}\"");
        assert!(
            !cargo_toml.contains(&key_form) && !cargo_toml.contains(&table_form) && !cargo_toml.contains(&quoted_form),
            "agentic-runtime/Cargo.toml must not declare `{forbidden}` as a direct dependency (ADR-0003 forbids); \
             found a reference in Cargo.toml:\n{cargo_toml}"
        );
    }
}
