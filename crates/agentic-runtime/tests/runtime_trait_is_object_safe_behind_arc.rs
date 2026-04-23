//! Story 19 acceptance test: the `Runtime` trait is object-safe and is
//! held by callers as `Arc<dyn Runtime>` — NOT via `impl Runtime`
//! generics. Both `ClaudeCodeRuntime` and `MockRuntime` can be dropped
//! behind the same trait object and a struct field can hold one and
//! invoke `spawn_claude_session` through it.
//!
//! Justification (from stories/19.yml acceptance.tests[0]):
//!   Proves the `Runtime` trait is object-safe and addressable as
//!   `Arc<dyn Runtime>`: a function with the signature
//!   `fn take(_: Arc<dyn Runtime>)` compiles against both
//!   `Arc::new(ClaudeCodeRuntime::new(...)?)` and
//!   `Arc::new(MockRuntime::from_fixture(...))` without any
//!   `where Self: Sized` escape hatches. A second function
//!   holding `Arc<dyn Runtime>` in a struct field calls
//!   `spawn_claude_session` through the trait object and
//!   receives a `RunOutcome`. Without this, callers degrade to
//!   `impl Runtime` generics everywhere — viral generics turn
//!   every caller (the host CLI, the sandbox entrypoint, a
//!   future scheduler) into a monomorphised copy of the
//!   runtime surface, which is exactly the problem the `Store`
//!   trait's `Arc<dyn Store>` discipline solved. Symmetry with
//!   `Store` is deliberate.
//!
//! Red today: compile-red. The `Runtime` trait, `ClaudeCodeRuntime`,
//! `MockRuntime`, `RunConfig`, `RunOutcome`, and `spawn_claude_session`
//! do not yet exist as public symbols of `agentic_runtime`. Every `use`
//! on the next lines fails to resolve, and `cargo check` fails with
//! rustc "unresolved import" errors before any runtime code runs.

use agentic_runtime::{ClaudeCodeRuntime, MockRuntime, RunConfig, RunOutcome, Runtime};
use agentic_store::{MemStore, Store};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

/// The load-bearing signature: if this compiles, the trait is
/// object-safe. The body never runs — the point is the type check on
/// the parameter.
fn take(_runtime: Arc<dyn Runtime>) {}

struct RuntimeHolder {
    runtime: Arc<dyn Runtime>,
}

impl RuntimeHolder {
    async fn drive(&self, cfg: RunConfig) -> RunOutcome {
        self.runtime
            .spawn_claude_session(cfg)
            .await
            .expect("spawn_claude_session should succeed for the mock fixture")
    }
}

fn sample_config(runs_root: PathBuf, story_id: i64) -> RunConfig {
    RunConfig {
        run_id: "11111111-2222-4333-8444-555566667777".to_string(),
        story_id,
        story_yaml_bytes: format!("id: {story_id}\n").into_bytes(),
        signer: "sandbox:claude-sonnet-4-6@run-11111111".to_string(),
        build_config: json!({ "max_inner_loop_iterations": 3 }),
        runs_root,
        repo_path: None,
        branch_name: None,
        prompt: "hello".to_string(),
        event_sink: Box::new(NullSink),
    }
}

struct NullSink;

impl agentic_runtime::EventSink for NullSink {
    fn emit(&mut self, _line: &str) {}
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_trait_is_object_safe_and_held_behind_arc() {
    // Claude-backed runtime behind Arc<dyn Runtime>: the constructor
    // signature is whatever the crate picks, but the result must coerce
    // to the trait object without any `where Self: Sized` escape.
    let claude_rt = ClaudeCodeRuntime::new().expect("ClaudeCodeRuntime::new");
    let as_trait_object_claude: Arc<dyn Runtime> = Arc::new(claude_rt);
    take(as_trait_object_claude);

    // MockRuntime behind Arc<dyn Runtime>: the only difference from the
    // claude path is the event source. Same trait, same dispatch.
    let fixture =
        PathBuf::from("crates/agentic-runtime/tests/fixtures/mock_green_three_pairs.ndjson");
    let mock_rt = MockRuntime::from_fixture(&fixture).expect("MockRuntime::from_fixture");
    let as_trait_object_mock: Arc<dyn Runtime> = Arc::new(mock_rt);

    // The struct-field path — a consumer holding a `Arc<dyn Runtime>`
    // and calling through the trait object — is what proves there is
    // no generic parameter leaking through to the call site.
    let holder = RuntimeHolder {
        runtime: Arc::clone(&as_trait_object_mock),
    };

    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let cfg = sample_config(runs_root_tmp.path().to_path_buf(), 19);
    let outcome: RunOutcome = holder.drive(cfg).await;

    // Just read one field from the RunOutcome so the type is actually
    // load-bearing in the assertion (a future refactor that silently
    // dropped the return type would fail this compile).
    assert!(
        !outcome.run_id.trim().is_empty(),
        "RunOutcome.run_id must be non-empty; got {:?}",
        outcome.run_id
    );

    // Sanity: the store behind the trait object is shareable exactly as
    // `Arc<dyn Store>` is — the symmetry story 19's guidance names.
    let _store: Arc<dyn Store> = Arc::new(MemStore::new());

    take(as_trait_object_mock);
}
