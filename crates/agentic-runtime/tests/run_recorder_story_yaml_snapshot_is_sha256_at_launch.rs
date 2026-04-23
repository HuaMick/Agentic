//! Story 16 acceptance test: `story_yaml_snapshot` is a SHA-256 of
//! the story YAML bytes AT LAUNCH — not a path, not a mutable
//! reference.
//!
//! Justification (from stories/16.yml acceptance.tests[6]):
//!   Proves the snapshot field is a content hash, not a path or a
//!   mutable reference: given a recorder started with
//!   `story_yaml_bytes: &[u8]` equal to the exact bytes of the
//!   story YAML at launch, the resulting `runs` row's
//!   `story_yaml_snapshot` equals `sha256(bytes)` as 64 lowercase
//!   hex characters. Mutating the story YAML on disk AFTER recorder
//!   start does NOT change the value in the row. Without this, the
//!   reproducibility receipt is forgeable: a later edit to the
//!   story would retroactively rewrite what the agent was
//!   supposedly working from.
//!
//! Red today: natural. The recorder types do not yet exist.

use agentic_runtime::{Outcome, RunRecorder, RunRecorderConfig};
use agentic_store::{MemStore, Store};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn story_yaml_snapshot_equals_sha256_hex_at_launch_and_does_not_drift_after_disk_mutation() {
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Compose the story YAML on disk so we can mutate it AFTER start.
    let story_dir = TempDir::new().expect("story dir");
    let story_path = story_dir.path().join("15.yml");
    let initial_bytes = b"id: 15\ntitle: fixture at launch\n".to_vec();
    fs::write(&story_path, &initial_bytes).expect("write initial story");

    // Capture the expected hash: sha256 of the launch bytes, 64
    // lowercase hex chars.
    let expected_hash = {
        let mut hasher = Sha256::new();
        hasher.update(&initial_bytes);
        let digest = hasher.finalize();
        let mut s = String::with_capacity(64);
        for b in digest {
            s.push_str(&format!("{b:02x}"));
        }
        s
    };
    assert_eq!(
        expected_hash.len(),
        64,
        "sanity: reference hash must be 64 chars"
    );

    let run_id = "dddd4444-eeee-4fff-8000-111122223333".to_string();

    let cfg = RunRecorderConfig {
        store: Arc::clone(&store),
        runs_root: runs_root_tmp.path().to_path_buf(),
        run_id: run_id.clone(),
        story_id: 15,
        story_yaml_bytes: initial_bytes.clone(),
        signer: "sandbox:stub@run-hash".to_string(),
        build_config: json!({}),
    };

    let recorder = RunRecorder::start(cfg).expect("start should succeed");

    // Mutate the story YAML on disk AFTER start. The snapshot is
    // pinned at start; this mutation must NOT change the row's
    // `story_yaml_snapshot`.
    fs::write(
        &story_path,
        b"id: 15\ntitle: mutated after launch\nadditional: fields\n",
    )
    .expect("mutate story");

    recorder
        .finish(Outcome::Green {
            signing_run_id: "stub-signing-1".to_string(),
        })
        .expect("finish should succeed");

    let rows = store
        .query("runs", &|doc| doc["run_id"] == json!(run_id))
        .expect("query");
    assert_eq!(rows.len(), 1, "one row for this run_id; got {rows:?}");

    let snapshot = rows[0]["story_yaml_snapshot"]
        .as_str()
        .expect("story_yaml_snapshot must be a string");

    assert_eq!(
        snapshot, expected_hash,
        "story_yaml_snapshot must equal sha256(launch bytes) as 64-char \
         lowercase hex; got {snapshot:?}, expected {expected_hash:?}. \
         Mutations to the story YAML on disk AFTER recorder start MUST NOT \
         change the value in the row — the snapshot is a content hash of \
         the bytes at launch, not a reference to the file."
    );

    // Defensive: lowercase and length (the justification names both).
    assert_eq!(snapshot.len(), 64);
    assert!(
        snapshot.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "snapshot must be lowercase hex; got {snapshot:?}"
    );
}
