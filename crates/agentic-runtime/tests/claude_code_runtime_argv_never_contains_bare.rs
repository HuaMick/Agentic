//! Story 19 acceptance test: `ClaudeCodeRuntime::compose_argv(&config)`
//! returns an argv vector that NEVER contains `--bare` or any
//! `--bare=...` flag, whose first non-program token is `-p`, and which
//! contains `--output-format stream-json --verbose` as adjacent tokens.
//!
//! Justification (from stories/19.yml acceptance.tests[1]):
//!   Proves the ADR-0003 prohibition is enforced in code, not
//!   just in prose: `ClaudeCodeRuntime::compose_argv(&config)`
//!   (a pure function the spawn path routes through) returns
//!   an argv vector whose elements never contain the literal
//!   `--bare`, never contain a key-value-style
//!   `"--bare=..."`, and whose first non-program argument is
//!   `-p`. The same vector contains `--output-format`
//!   `stream-json` and `--verbose` as adjacent tokens. The
//!   test asserts the positive and negative shapes over a
//!   matrix of `RunConfig` inputs (with and without models
//!   declared, with and without budget). Without this, the
//!   prohibition lives only in the ADR and the first
//!   well-meaning refactor that "adds an API-key fallback
//!   for CI" removes it silently; this test makes that
//!   refactor compile-green then test-red.
//!
//! Red today: compile-red. `ClaudeCodeRuntime::compose_argv` and the
//! `RunConfig` struct do not yet exist.

use agentic_runtime::{ClaudeCodeRuntime, EventSink, RunConfig};
use serde_json::json;
use std::path::PathBuf;

struct NullSink;
impl EventSink for NullSink {
    fn emit(&mut self, _line: &str) {}
}

fn matrix_configs() -> Vec<RunConfig> {
    let base_prompt = "drive the agent".to_string();
    let mk = |build_config: serde_json::Value, signer_tail: &str| RunConfig {
        run_id: format!("aaaaaaaa-bbbb-4ccc-8ddd-{signer_tail:>012}"),
        story_id: 19,
        story_yaml_bytes: b"id: 19\n".to_vec(),
        signer: format!("sandbox:claude-sonnet-4-6@run-{signer_tail}"),
        build_config,
        runs_root: PathBuf::from("/tmp/ignored-for-argv"),
        repo_path: None,
        branch_name: None,
        prompt: base_prompt.clone(),
        event_sink: Box::new(NullSink),
    };
    vec![
        // no models declared, no budget
        mk(json!({}), "000000000001"),
        // models declared, no budget
        mk(json!({ "models": ["claude-sonnet-4-6"] }), "000000000002"),
        // no models, budget set
        mk(json!({ "max_inner_loop_iterations": 5 }), "000000000003"),
        // both present
        mk(
            json!({
                "models": ["claude-sonnet-4-6"],
                "max_inner_loop_iterations": 2
            }),
            "000000000004",
        ),
    ]
}

#[test]
fn compose_argv_never_contains_bare_and_has_required_tokens_adjacent() {
    for cfg in matrix_configs() {
        let argv: Vec<String> = ClaudeCodeRuntime::compose_argv(&cfg);

        // Negative shape: NEVER `--bare`, NEVER `--bare=...`.
        for (i, token) in argv.iter().enumerate() {
            assert!(
                token != "--bare",
                "argv[{i}] == \"--bare\" violates ADR-0003; full argv: {argv:?}"
            );
            assert!(
                !token.starts_with("--bare="),
                "argv[{i}] = {token:?} begins with `--bare=`; ADR-0003 forbids. full argv: {argv:?}"
            );
            assert!(
                !token.contains("--api-key"),
                "argv[{i}] = {token:?} contains `--api-key`; ADR-0003 forbids direct-API flags. full argv: {argv:?}"
            );
        }

        // Positive shape: first non-program argument is `-p`. argv[0]
        // is the program name (e.g. "claude"); argv[1] must be `-p`.
        assert!(
            argv.len() >= 2,
            "argv must have at least program + `-p`; got {argv:?}"
        );
        assert_eq!(
            argv[1], "-p",
            "first non-program argv token must be `-p`; got argv[1] = {:?}; full argv: {argv:?}",
            argv[1]
        );

        // Adjacent tokens: `--output-format` immediately followed by
        // `stream-json`.
        let fmt_idx = argv
            .iter()
            .position(|t| t == "--output-format")
            .unwrap_or_else(|| panic!("argv must contain `--output-format`; got {argv:?}"));
        assert!(
            fmt_idx + 1 < argv.len(),
            "`--output-format` must be followed by a value; argv: {argv:?}"
        );
        assert_eq!(
            argv[fmt_idx + 1],
            "stream-json",
            "argv[{}] must be `stream-json` (adjacent to --output-format); full argv: {argv:?}",
            fmt_idx + 1
        );

        // `--verbose` must appear somewhere in the argv.
        assert!(
            argv.iter().any(|t| t == "--verbose"),
            "argv must contain `--verbose`; got {argv:?}"
        );
    }
}
