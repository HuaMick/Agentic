//! Story 5 acceptance test (RED scaffold): standalone-resilient-library
//! pattern, applied to `agentic-store` with the SurrealDB backend.
//!
//! Justification (from stories/5.yml):
//!   Proves the standalone-resilient-library claim: a `SurrealStore` can
//!   be constructed and driven end-to-end with only `agentic-store` and
//!   its declared backend dependencies (the `surrealdb` crate itself,
//!   `serde`, `tokio` if required) wired up — no `agentic-orchestrator`,
//!   `agentic-runtime`, `agentic-sandbox`, or CLI crate in the test's
//!   link graph.
//!
//! Dependency floor is pinned by imports. This file MUST NOT use any of:
//!   - agentic_orchestrator
//!   - agentic_runtime
//!   - agentic_sandbox
//!   - agentic_cli
//!   - agentic_events
//! If an implementer adds any of those as an `agentic-store` (dev-)dep
//! purely to satisfy this test, the review catches it; if they add it
//! silently through a transitive path, the dependency floor in this file
//! still stays honest.
//!
//! Red state expected at scaffold-write time:
//!   - `agentic_store::SurrealStore` does not exist.
//!   - `tempfile` is not a dev-dependency yet.

use agentic_store::{Store, SurrealStore};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn surrealstore_drives_happy_path_with_only_backend_deps_in_scope() {
    // Setup: one TempDir for the SurrealDB on-disk root.
    let root = TempDir::new().expect("tempdir should be creatable");

    // Act: full happy path, library-only — construct, upsert, append,
    // get, query, drop. No CLI, no orchestrator, no sandbox.
    let store =
        SurrealStore::open(root.path()).expect("SurrealStore::open should succeed on empty temp dir");

    store
        .upsert("evidence", "run-1", json!({ "verdict": "pass" }))
        .expect("upsert should work on a bare SurrealStore");
    store
        .append("log", json!({ "msg": "standalone ok" }))
        .expect("append should work on a bare SurrealStore");

    let got = store
        .get("evidence", "run-1")
        .expect("get should succeed")
        .expect("upserted row should be found");
    assert_eq!(got, json!({ "verdict": "pass" }));

    let log = store
        .query("log", &|_| true)
        .expect("query should succeed");
    assert_eq!(log.len(), 1);

    panic!(
        "red: Proves the standalone-resilient-library claim: a `SurrealStore` can be constructed and driven end-to-end with only `agentic-store` and its declared backend dependencies (the `surrealdb` crate itself, `serde`, `tokio` if required) wired up — no `agentic-orchestrator`, `agentic-runtime`, `agentic-sandbox`, or CLI crate in the test's link graph."
    );
}
