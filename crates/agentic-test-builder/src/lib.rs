//! agentic-test-builder: Plan and record red-state evidence for stories.
//!
//! This library provides two functions to test-builder users:
//! - [`TestBuilder::plan`]: pure read-only function that returns a plan
//!   (no side effects, no I/O attestation, safe on dirty tree).
//! - [`TestBuilder::record`]: verifies user-authored scaffolds probe red
//!   and writes an atomic JSONL evidence row.
//!
//! The key invariant: record runs on a clean git working tree
//! (fail-closed-on-dirty-tree pattern). On a dirty tree it returns
//! [`TestBuilderError::DirtyTree`] without writing any files or evidence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use agentic_story::Story;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// The test-builder: plans and records red-state evidence.
#[derive(Debug)]
pub struct TestBuilder {
    repo_root: PathBuf,
}

/// A single entry in the plan: one scaffold the user is expected to author.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEntry {
    /// The scaffold path from `acceptance.tests[].file`.
    pub file: String,
    /// Derived from path: `crates/<name>/tests/...` implies `<name>`.
    pub target_crate: String,
    /// The story's `acceptance.tests[].justification` verbatim.
    pub justification: String,
    /// One of `compile` or `runtime` (a hint, not a pre-commitment).
    pub expected_red_path: String,
    /// Possibly-empty array of preconditions extracted from guidance.
    pub fixture_preconditions: Vec<String>,
}

/// Error variants for test-builder operations.
#[derive(Debug, PartialEq, Eq)]
pub enum TestBuilderError {
    /// Working tree has uncommitted or untracked changes outside scaffold paths.
    DirtyTree,
    /// Story has zero acceptance tests.
    NoAcceptanceTests,
    /// A justification is too thin (empty, single token, or "TODO").
    ThinJustification { index: usize },
    /// A planned scaffold does not exist on disk.
    ScaffoldMissing { file: PathBuf },
    /// A scaffold exists but does not parse as Rust.
    ScaffoldParseError { file: PathBuf, parse_error: String },
    /// A scaffold parses but probes green (compile or runtime).
    ScaffoldNotRed { file: PathBuf, probe: String },
    /// Other errors: story loader failure, I/O, etc.
    Other(String),
}

/// Result of a successful record run.
#[derive(Debug)]
pub struct RecordOutcome {
    /// Paths of files recorded.
    recorded: Vec<PathBuf>,
}

impl TestBuilder {
    /// Construct a new [`TestBuilder`] rooted at the given repository directory.
    pub fn new(repo_root: impl AsRef<Path>) -> Self {
        Self {
            repo_root: repo_root.as_ref().to_path_buf(),
        }
    }

    /// Plan the scaffolds for a story. Pure read-only function with no side effects.
    /// Works on any tree state (clean or dirty). Returns a vector of plan entries,
    /// one per acceptance test, with no I/O beyond reading the story file.
    pub fn plan(story: &Story) -> Vec<PlanEntry> {
        let mut entries = Vec::new();

        for test in &story.acceptance.tests {
            let target_crate =
                extract_crate_name(&test.file).unwrap_or_else(|_| "unknown".to_string());

            let expected_red_path = guess_expected_red_path(&test.justification);

            let fixture_preconditions = extract_fixture_preconditions(&story.guidance);

            entries.push(PlanEntry {
                file: test.file.to_string_lossy().into_owned(),
                target_crate,
                justification: test.justification.trim().to_string(),
                expected_red_path,
                fixture_preconditions,
            });
        }

        entries
    }

    /// Record red-state evidence for user-authored scaffolds.
    /// Requires a clean tree. Probes each scaffold and writes an atomic
    /// evidence JSONL on success, or returns a typed refusal on any error.
    pub fn record(&self, story_id: u32) -> Result<RecordOutcome, TestBuilderError> {
        // Load and validate the story first.
        let story_path = self.repo_root.join(format!("stories/{story_id}.yml"));
        let story = Story::load(&story_path).map_err(|e| TestBuilderError::Other(e.to_string()))?;

        // Validate: non-empty acceptance tests.
        if story.acceptance.tests.is_empty() {
            return Err(TestBuilderError::NoAcceptanceTests);
        }

        // Validate: all justifications are substantive.
        for (index, test) in story.acceptance.tests.iter().enumerate() {
            if is_thin_justification(&test.justification) {
                return Err(TestBuilderError::ThinJustification { index });
            }
        }

        // Fail-closed on dirty tree: check before any write.
        if !is_tree_clean(&self.repo_root, &story) {
            return Err(TestBuilderError::DirtyTree);
        }

        // Plan the scaffolds.
        let plan = Self::plan(&story);

        // Probe each scaffold, collecting verdicts.
        let mut verdicts = Vec::new();
        for (idx, entry) in plan.iter().enumerate() {
            let test_path = self.repo_root.join(&entry.file);

            // Check file exists.
            if !test_path.exists() {
                return Err(TestBuilderError::ScaffoldMissing { file: test_path });
            }

            // Parse as Rust.
            let scaffold_body = fs::read_to_string(&test_path).map_err(|e| {
                TestBuilderError::ScaffoldParseError {
                    file: test_path.clone(),
                    parse_error: format!("failed to read: {e}"),
                }
            })?;

            if syn::parse_file(&scaffold_body).is_err() {
                return Err(TestBuilderError::ScaffoldParseError {
                    file: test_path.clone(),
                    parse_error: "not valid Rust source".to_string(),
                });
            }

            // Probe the scaffold.
            let (red_path, diagnostic) = probe_scaffold(&self.repo_root, &entry.target_crate, idx)?;

            // Check that the probe actually came back red.
            if red_path == "green" {
                return Err(TestBuilderError::ScaffoldNotRed {
                    file: test_path,
                    probe: "compile".to_string(),
                });
            }

            verdicts.push(json!({
                "file": entry.file,
                "verdict": "red",
                "red_path": red_path,
                "diagnostic": diagnostic,
            }));
        }

        // Write evidence atomically.
        let run_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let commit = get_head_commit(&self.repo_root)?;

        let evidence_dir = self.repo_root.join(format!("evidence/runs/{story_id}"));

        // Construct the filename from timestamp.
        let filename = format!(
            "{}-red.jsonl",
            timestamp
                .replace(":", "-")
                .split('.')
                .next()
                .unwrap_or(&timestamp)
        );

        let evidence_path = evidence_dir.join(&filename);
        let evidence_row = json!({
            "run_id": run_id,
            "story_id": story_id,
            "commit": commit,
            "timestamp": timestamp,
            "verdicts": verdicts,
        });

        // Write to a tempfile first, then rename for atomicity.
        let temp_path = evidence_dir.join(format!("{}.tmp", filename));

        // Create the directory if needed.
        fs::create_dir_all(&evidence_dir).map_err(|e| TestBuilderError::Other(e.to_string()))?;

        // Write to tempfile.
        fs::write(&temp_path, format!("{}\n", evidence_row))
            .map_err(|e| TestBuilderError::Other(e.to_string()))?;

        // Atomically rename.
        fs::rename(&temp_path, &evidence_path)
            .map_err(|e| TestBuilderError::Other(e.to_string()))?;

        let mut recorded = Vec::new();
        for entry in plan {
            recorded.push(PathBuf::from(&entry.file));
        }

        Ok(RecordOutcome { recorded })
    }
}

impl RecordOutcome {
    /// Paths of files recorded.
    pub fn recorded_paths(&self) -> &[PathBuf] {
        &self.recorded
    }
}

/// Check if the git working tree is clean outside the scaffold paths in the story.
fn is_tree_clean(repo_root: &Path, story: &Story) -> bool {
    let repo = match git2::Repository::open(repo_root) {
        Ok(r) => r,
        Err(_) => return false,
    };

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .include_ignored(false)
        .exclude_submodules(true);

    let statuses = match repo.statuses(Some(&mut opts)) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Collect the scaffold paths that the user is expected to stage.
    let mut scaffold_paths = std::collections::HashSet::new();
    for test in &story.acceptance.tests {
        scaffold_paths.insert(test.file.to_string_lossy().into_owned());
    }

    for entry in statuses.iter() {
        if entry.status().contains(git2::Status::IGNORED) {
            continue;
        }

        let path_str = entry.path().unwrap_or("");
        let path_normalized = path_str.replace('\\', "/");

        // Exclude test fixture and temporary directories from dirty check.
        if path_normalized.starts_with(".bin/")
            || path_normalized.starts_with(".agentic-cache/")
            || path_normalized.starts_with("target/")
            || path_normalized == ".bin"
            || path_normalized == ".agentic-cache"
            || path_normalized == "target"
            || path_normalized == "Cargo.lock"
        {
            continue;
        }

        // Allow dirty scaffold paths (the user is expected to stage them).
        if scaffold_paths.contains(&path_normalized) {
            continue;
        }

        // Any other non-ignored file makes the tree dirty.
        return false;
    }
    true
}

/// Check if a justification is "thin" (empty, single token, or TODO-like).
fn is_thin_justification(text: &str) -> bool {
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return true;
    }

    // Single whitespace-delimited token.
    if !trimmed.contains(' ') && !trimmed.contains('\n') && trimmed.split('\t').count() == 1 {
        return true;
    }

    // TODO-like patterns.
    if matches!(trimmed.to_uppercase().as_str(), "TODO" | "TBD" | "...") {
        return true;
    }

    false
}

/// Extract crate name from a test file path like "crates/foo/tests/bar.rs".
fn extract_crate_name(test_file: &Path) -> Result<String, TestBuilderError> {
    test_file
        .components()
        .nth(1)
        .and_then(|c| c.as_os_str().to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| TestBuilderError::Other("cannot extract crate name".to_string()))
}

/// Extract test name from a test file path (the file stem).
#[allow(dead_code)]
fn extract_test_name(test_file: &Path) -> Result<String, TestBuilderError> {
    test_file
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| TestBuilderError::Other("cannot extract test name".to_string()))
}

/// Guess the expected red path from the justification text.
/// Heuristic: if the justification names a symbol the target crate likely does not declare,
/// guess `compile`; otherwise guess `runtime`.
fn guess_expected_red_path(justification: &str) -> String {
    let first_line = justification.lines().next().unwrap_or(justification);

    let implies_missing_symbol = first_line.contains("public function")
        || first_line.contains("public struct")
        || first_line.contains("symbol")
        || first_line.contains("function")
        || first_line.contains("method")
        || (first_line.contains('`') && first_line.contains("not yet"))
        || (first_line.contains('`')
            && (first_line.contains("undefined") || first_line.contains("undeclared")));

    if implies_missing_symbol {
        "compile".to_string()
    } else {
        "runtime".to_string()
    }
}

/// Extract fixture preconditions from the story's guidance.
/// Looks for paragraphs containing "fixture" or "precondition" near the top.
fn extract_fixture_preconditions(guidance: &str) -> Vec<String> {
    let mut preconditions = Vec::new();
    let lines: Vec<&str> = guidance.lines().collect();

    // Scan the first 20 lines or so for precondition paragraphs.
    for line in lines.iter().take(20) {
        let lower = line.to_lowercase();
        if (lower.contains("fixture") || lower.contains("precondition")) && !line.trim().is_empty()
        {
            preconditions.push(line.trim().to_string());
        }
    }

    preconditions
}

/// Probe the scaffold via `cargo check` and `cargo test`.
/// Returns (red_path, diagnostic) where red_path is "compile" or "runtime",
/// or an error if the probe fails non-red or the scaffold is green.
fn probe_scaffold(
    repo_root: &Path,
    crate_name: &str,
    _test_index: usize,
) -> Result<(String, String), TestBuilderError> {
    // Track what needs cleanup
    let cargo_lock_path = repo_root.join("Cargo.lock");
    let cargo_lock_existed = cargo_lock_path.exists();

    // First try `cargo check` to detect compile-red.
    let check_output = Command::new("cargo")
        .args(["check", "--package", crate_name])
        .current_dir(repo_root)
        .output()
        .map_err(|e| TestBuilderError::Other(format!("failed to run cargo check: {e}")))?;

    let stderr = String::from_utf8_lossy(&check_output.stderr);

    // Check for rustc errors (compile-red).
    for line in stderr.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("error[E") {
            return Ok(("compile".to_string(), trimmed.to_string()));
        }
        if line.contains(": error[E") {
            return Ok(("compile".to_string(), line.to_string()));
        }
    }

    // Compile succeeded, now try `cargo test` on all tests in the crate.
    let test_output = Command::new("cargo")
        .args(["test", "--package", crate_name, "--", "--nocapture"])
        .current_dir(repo_root)
        .output()
        .map_err(|e| {
            // Clean up Cargo.lock if we created it
            if !cargo_lock_existed && cargo_lock_path.exists() {
                let _ = fs::remove_file(&cargo_lock_path);
            }
            TestBuilderError::Other(format!("failed to run cargo test: {e}"))
        })?;

    let stdout = String::from_utf8_lossy(&test_output.stdout);
    let stderr = String::from_utf8_lossy(&test_output.stderr);

    // Look for panic output (runtime-red).
    let combined: Vec<&str> = stdout.lines().chain(stderr.lines()).collect();
    for (i, line) in combined.iter().enumerate() {
        if let Some(idx) = line.find("panicked at") {
            let after = &line[idx + "panicked at".len()..];
            let inline = after.trim().trim_start_matches('\'').trim_end_matches(',');
            // Case A: older format "panicked at 'msg', file:line"
            if let Some(end) = inline.find('\'') {
                let msg = &inline[..end];
                if !msg.is_empty() {
                    // Clean up Cargo.lock before returning
                    if !cargo_lock_existed && cargo_lock_path.exists() {
                        let _ = fs::remove_file(&cargo_lock_path);
                    }
                    return Ok(("runtime".to_string(), msg.to_string()));
                }
            }
            // Case B: newer format — the next non-empty line is the message.
            if let Some(msg_line) = combined[i + 1..].iter().find(|l| !l.trim().is_empty()) {
                // Clean up Cargo.lock before returning
                if !cargo_lock_existed && cargo_lock_path.exists() {
                    let _ = fs::remove_file(&cargo_lock_path);
                }
                return Ok(("runtime".to_string(), msg_line.trim().to_string()));
            }
            // Fallback: whatever followed "panicked at" on the same line.
            if !inline.is_empty() {
                // Clean up Cargo.lock before returning
                if !cargo_lock_existed && cargo_lock_path.exists() {
                    let _ = fs::remove_file(&cargo_lock_path);
                }
                return Ok(("runtime".to_string(), inline.to_string()));
            }
        }
    }

    // Test did not panic — the scaffold is green (error condition).
    if test_output.status.success() {
        // Clean up Cargo.lock before returning
        if !cargo_lock_existed && cargo_lock_path.exists() {
            let _ = fs::remove_file(&cargo_lock_path);
        }
        return Ok(("green".to_string(), "test passed".to_string()));
    }

    // Test failed but no panic detected — treat as runtime-red fallback.
    if !cargo_lock_existed && cargo_lock_path.exists() {
        let _ = fs::remove_file(&cargo_lock_path);
    }
    Ok(("runtime".to_string(), "test failed".to_string()))
}

/// Get the current HEAD commit hash.
fn get_head_commit(repo_root: &Path) -> Result<String, TestBuilderError> {
    let repo =
        git2::Repository::open(repo_root).map_err(|e| TestBuilderError::Other(e.to_string()))?;

    let head = repo
        .head()
        .map_err(|e| TestBuilderError::Other(e.to_string()))?;

    head.target()
        .ok_or_else(|| TestBuilderError::Other("cannot get HEAD commit".to_string()))
        .map(|oid| oid.to_string())
}
