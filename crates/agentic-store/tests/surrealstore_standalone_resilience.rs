//! Story 5 acceptance test: SurrealStore is drivable as a standalone library.
//!
//! Justification (from stories/5.yml): a `SurrealStore` can be constructed
//! and driven end-to-end with only `agentic-store` and its declared backend
//! dependencies (the `surrealdb` crate itself, `serde`, `tokio` if required)
//! wired up — no `agentic-orchestrator`, `agentic-runtime`, `agentic-sandbox`,
//! or CLI crate in the test's link graph. Without this, `agentic-store`
//! could grow an orchestrator dependency through a transitive path and we
//! would only learn when the "system is in flames" case (the one this
//! pattern exists for) actually happened.
//!
//! Pattern: standalone-resilient-library. The dependency floor is enforced
//! by what this test imports — only `agentic_store` from the workspace.
//! If any forbidden transitive dependency is added under `agentic-store`,
//! it will appear in this test's link graph and reviewers can spot it.

// Compile-time witness: this test names ONLY `agentic_store` from the
// workspace. Adding `agentic_orchestrator`, `agentic_runtime`,
// `agentic_sandbox`, or `agentic_cli` to this file would be a review-time
// red flag and the standalone-resilience claim would break.
use agentic_store::{Store, StoreError, SurrealStore};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn surrealstore_drives_full_happy_path_with_no_workspace_deps_beyond_agentic_store() {
    let dir = TempDir::new().expect("create temp dir");

    // Construct via the public surface only — no orchestrator, no runtime.
    let store: Box<dyn Store> =
        Box::new(SurrealStore::open(dir.path()).expect("open SurrealStore at fresh temp dir"));

    // Drive one full happy path: upsert + append + get + query, all through
    // the trait, all with serde_json values — exactly the surface a CLI
    // shim would touch.
    store
        .upsert("config", "active", json!({ "version": 1 }))
        .expect("upsert through trait should succeed");
    store
        .append("events", json!({ "kind": "started" }))
        .expect("append through trait should succeed");
    store
        .append("events", json!({ "kind": "finished" }))
        .expect("append through trait should succeed");

    let cfg = store
        .get("config", "active")
        .expect("get should succeed")
        .expect("upserted row should be present");
    assert_eq!(cfg, json!({ "version": 1 }));

    let events = store
        .query("events", &|_| true)
        .expect("query should succeed");
    assert_eq!(
        events.len(),
        2,
        "two appends must yield two rows; got {events:?}"
    );

    // Errors come through the same crate's typed enum — not anyhow, not a
    // runtime / orchestrator error type.
    let _ensure_error_is_local: fn(&StoreError) -> &dyn std::error::Error = |e| e;
}
