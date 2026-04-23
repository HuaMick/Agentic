//! Story 19 acceptance test: `ClaudeCodeRuntime::new()` fail-closes
//! with a typed `RuntimeError::ClaudeSpawn` variant when the `claude`
//! binary is absent from the configured PATH. Not a panic, not a
//! `String` error, not a silent deferred success.
//!
//! Justification (from stories/19.yml acceptance.tests[3]):
//!   Proves `ClaudeCodeRuntime::new(cfg)` fail-closes with a
//!   typed `RuntimeError::ClaudeSpawn` naming the binary
//!   lookup failure when the `claude` binary is absent from
//!   the configured PATH — not a panic, not a `String`
//!   error, not a silent success deferred to first
//!   `spawn_claude_session` call. The test sets `PATH=""`
//!   (empty) for the constructor call and asserts the error
//!   variant. Without this, a missing `claude` produces a
//!   late panic deep in the spawn path with a process-level
//!   "No such file or directory" from `std::process`, and
//!   operators diagnose a corrupt-install problem as a
//!   runtime bug in whatever library called spawn.
//!
//! Red today: compile-red. `ClaudeCodeRuntime` and
//! `RuntimeError::ClaudeSpawn` do not exist.

use agentic_runtime::{ClaudeCodeRuntime, RuntimeError};

#[test]
fn new_returns_typed_claude_spawn_error_on_empty_path() {
    // Save PATH, empty it, call new, restore — all inside the test.
    let saved_path = std::env::var_os("PATH");
    std::env::set_var("PATH", "");

    let result = ClaudeCodeRuntime::new();

    match saved_path {
        Some(v) => std::env::set_var("PATH", v),
        None => std::env::remove_var("PATH"),
    }

    match result {
        Err(RuntimeError::ClaudeSpawn { .. }) => {
            // Typed-variant match is the assertion. The reason
            // sub-enum is covered by a sibling test; this test only
            // pins that the outer variant is `ClaudeSpawn` and the
            // call did not panic or return a stringly-typed error.
        }
        Err(other) => panic!(
            "ClaudeCodeRuntime::new with empty PATH must return RuntimeError::ClaudeSpawn; \
             got {other:?} — the typed error contract is what makes a corrupt-install \
             diagnosable from whatever library called spawn"
        ),
        Ok(_) => panic!(
            "ClaudeCodeRuntime::new with empty PATH must NOT succeed; \
             a silent success deferred to first `spawn_claude_session` is the \
             exact failure mode the typed error was added to prevent"
        ),
    }
}
