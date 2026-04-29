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

/// Classification of a scaffold under the three-gate amendment rule (ADR-0005).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaffoldClassification {
    /// Scaffold is first-authoring: no prior evidence row exists.
    FirstAuthoring,
    /// Scaffold qualifies for re-authoring: all three gates pass.
    ReAuthor,
    /// Scaffold is preserved: gates fail, or story is healthy, or grandfathered.
    Preserve,
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
    /// Classification failed (git error, store error, etc.).
    ClassificationFailed(String),
    /// Other errors: story loader failure, I/O, etc.
    Other(String),
}

/// Result of a successful record run.
#[derive(Debug)]
pub struct RecordOutcome {
    /// Paths of files recorded.
    recorded: Vec<PathBuf>,
    /// Verdicts for each recorded file in order.
    verdicts: Vec<(PathBuf, String)>,
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

    /// Classify a scaffold under the three-gate amendment rule (ADR-0005).
    ///
    /// Per the amendment, each scaffold is classified as one of:
    /// - FirstAuthoring: no prior evidence row exists for this story.
    /// - ReAuthor: all three gates pass (status under_construction, story YAML
    ///   newer than evidence commit, tree clean).
    /// - Preserve: any gate fails, or story is healthy, or grandfathered-bridge case,
    ///   or scaffold file exists in prior evidence (indicating it was tested before
    ///   and is unchanged).
    ///
    /// This is a pure read operation; no probing, no tree mutation.
    pub fn classify_scaffold(
        &self,
        story: &Story,
        scaffold_path: &Path,
        repo: &git2::Repository,
    ) -> ScaffoldClassification {
        self.classify_scaffold_internal(story, scaffold_path, repo)
    }

    /// Internal classification logic to avoid type-checking ICE.
    fn classify_scaffold_internal(
        &self,
        story: &Story,
        scaffold_path: &Path,
        repo: &git2::Repository,
    ) -> ScaffoldClassification {
        let story_id = story.id;
        let status = &story.status;

        // Check if this story has any prior evidence.
        let evidence_dir = self.repo_root.join(format!("evidence/runs/{story_id}"));
        let has_prior_evidence = evidence_dir.exists();

        // First-authoring case: no prior evidence at all.
        if !has_prior_evidence {
            // Grandfathered-bridge: story is healthy with no prior evidence.
            if matches!(status, agentic_story::Status::Healthy) {
                return ScaffoldClassification::Preserve;
            }
            // Otherwise: first-authoring (story is proposed or under_construction).
            return ScaffoldClassification::FirstAuthoring;
        }

        // Healthy stories never re-author; they preserve.
        if matches!(status, agentic_story::Status::Healthy) {
            return ScaffoldClassification::Preserve;
        }

        // Gate 1: story status must be under_construction.
        if !matches!(status, agentic_story::Status::UnderConstruction) {
            return ScaffoldClassification::Preserve;
        }

        // Gate 2: story YAML must be newer than the most recent evidence row's commit.
        let most_recent_commit = self.find_most_recent_evidence_commit_in_dir(&evidence_dir);
        let most_recent_evidence_commit = match most_recent_commit {
            Some(commit) => commit,
            None => return ScaffoldClassification::Preserve,
        };

        // Check if the story YAML has a commit newer than the evidence row.
        let story_yaml_path = format!("stories/{story_id}.yml");
        let yaml_is_newer = self.is_yaml_path_newer_than_commit(
            repo,
            &story_yaml_path,
            &most_recent_evidence_commit,
        );

        if yaml_is_newer {
            // All three gates pass (Gate 1, 2, and 3).
            // Check if the scaffold's justification has changed between the evidence commit and current story.
            // If unchanged, preserve it (test-builder may have deliberately left it alone).
            // If changed, it's eligible for re-authoring.
            if self.scaffold_justification_unchanged_since_evidence(
                story,
                &most_recent_evidence_commit,
                scaffold_path,
                repo,
            ) {
                // Justification unchanged → PRESERVE.
                ScaffoldClassification::Preserve
            } else {
                // Justification changed → RE-AUTHOR.
                ScaffoldClassification::ReAuthor
            }
        } else {
            ScaffoldClassification::Preserve
        }
    }



    /// Find the most recent evidence JSONL file path (without reading its contents).
    fn find_most_recent_evidence_jsonl(&self, evidence_dir: &Path) -> Option<PathBuf> {
        let entries = fs::read_dir(evidence_dir).ok()?;

        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|d| d.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with("-red.jsonl"))
                    .unwrap_or(false)
            })
            .collect();

        // Sort by filename (timestamp) in reverse order to get the most recent.
        files.sort_by(|a, b| {
            let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
            b_name.cmp(a_name)
        });

        files.into_iter().next()
    }

    /// Find the most recent evidence commit for a story by reading its latest JSONL.
    fn find_most_recent_evidence_commit_in_dir(&self, evidence_dir: &Path) -> Option<String> {
        let entries = fs::read_dir(evidence_dir).ok()?;

        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|d| d.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with("-red.jsonl"))
                    .unwrap_or(false)
            })
            .collect();

        // Sort by filename (timestamp) in reverse order to get the most recent.
        files.sort_by(|a, b| {
            let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
            b_name.cmp(a_name)
        });

        if let Some(path) = files.first() {
            let body = fs::read_to_string(path).ok()?;
            let line = body.lines().next()?;
            let row: serde_json::Value = serde_json::from_str(line).ok()?;
            row.get("commit")?.as_str().map(|s| s.to_string())
        } else {
            None
        }
    }

    /// Check if a scaffold's justification in the story YAML is unchanged since the evidence commit.
    fn scaffold_justification_unchanged_since_evidence(
        &self,
        story: &Story,
        evidence_commit_hash: &str,
        scaffold_path: &Path,
        repo: &git2::Repository,
    ) -> bool {
        // Parse the evidence commit hash.
        let oid = match git2::Oid::from_str(evidence_commit_hash) {
            Ok(o) => o,
            Err(_) => return false,
        };

        // Get the tree at the evidence commit.
        let evidence_tree = match repo.find_commit(oid) {
            Ok(c) => match c.tree() {
                Ok(t) => t,
                Err(_) => return false,
            },
            Err(_) => return false,
        };

        // Get the story YAML from the evidence commit.
        let story_id = story.id;
        let story_yaml_path = format!("stories/{story_id}.yml");
        let yaml_entry = match evidence_tree.get_path(std::path::Path::new(&story_yaml_path)) {
            Ok(e) => e,
            Err(_) => return false,
        };

        let yaml_content_string = match yaml_entry.to_object(repo) {
            Ok(obj) => match obj.as_blob() {
                Some(blob) => match std::str::from_utf8(blob.content()) {
                    Ok(content) => content.to_string(),
                    Err(_) => return false,
                },
                None => return false,
            },
            Err(_) => return false,
        };

        let yaml_content = yaml_content_string.as_str();

        // Parse the historical YAML.
        let historical_yaml: serde_yaml::Value = match serde_yaml::from_str(yaml_content) {
            Ok(parsed) => parsed,
            Err(_) => return false,
        };

        // Extract the justification from the historical YAML for this scaffold.
        let scaffold_path_str = scaffold_path.to_string_lossy();
        let historical_justification = if let Some(tests) = historical_yaml
            .get("acceptance")
            .and_then(|a| a.get("tests"))
            .and_then(|t| t.as_sequence())
        {
            let mut found = None;
            for test in tests {
                if let Some(file) = test.get("file").and_then(|f| f.as_str()) {
                    if file == scaffold_path_str {
                        found = test
                            .get("justification")
                            .and_then(|j| j.as_str())
                            .map(|s| s.trim().to_string());
                        break;
                    }
                }
            }
            found
        } else {
            None
        };

        // Get the current justification from the story.
        let current_justification = story
            .acceptance
            .tests
            .iter()
            .find(|test| test.file.to_string_lossy() == scaffold_path_str)
            .map(|test| test.justification.trim().to_string());

        // Compare: unchanged if both exist and match.
        match (historical_justification, current_justification) {
            (Some(hist), Some(curr)) => hist == curr,
            (None, None) => true,  // Neither had justification (unlikely)
            _ => false, // One exists, the other doesn't
        }
    }

    /// Check if story YAML is newer than the given evidence commit.
    fn is_yaml_path_newer_than_commit(
        &self,
        repo: &git2::Repository,
        story_yaml_path: &str,
        evidence_commit_hash: &str,
    ) -> bool {
        let mut revwalk = match repo.revwalk() {
            Ok(r) => r,
            Err(_) => return false,
        };

        if revwalk.push_head().is_err() {
            return false;
        }

        for oid_result in revwalk {
            let oid = match oid_result {
                Ok(o) => o,
                Err(_) => continue,
            };

            let commit = match repo.find_commit(oid) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let yaml_oid_str = oid.to_string();

            // Early return if we find the evidence commit.
            if yaml_oid_str == evidence_commit_hash {
                return false;
            }

            // Check if this commit touches the story YAML file.
            let touches_yaml = if commit.parent_count() == 0 {
                // Root commit
                let tree = match commit.tree() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                tree.get_path(std::path::Path::new(story_yaml_path)).is_ok()
            } else {
                // Non-root commit
                let parent = match commit.parent(0) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let parent_tree = match parent.tree() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let commit_tree = match commit.tree() {
                    Ok(t) => t,
                    Err(_) => continue,
                };

                let diff =
                    match repo.diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), None) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };

                let mut found = false;
                for delta in diff.deltas() {
                    if let Some(path) = delta.new_file().path() {
                        if path.to_string_lossy() == story_yaml_path {
                            found = true;
                            break;
                        }
                    }
                    if let Some(path) = delta.old_file().path() {
                        if path.to_string_lossy() == story_yaml_path {
                            found = true;
                            break;
                        }
                    }
                }
                found
            };

            if touches_yaml {
                // Found the YAML commit; it's newer than evidence if we got here.
                return true;
            }
        }

        false
    }

    /// Record red-state evidence for user-authored scaffolds.
    /// Requires a clean tree. Probes each scaffold and writes an atomic
    /// evidence JSONL on success, or returns a typed refusal on any error.
    /// Now supports mixed verdicts: red, preserved, and re-authored per scaffold.
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

        // Open the repo for classification.
        let repo = git2::Repository::open(&self.repo_root)
            .map_err(|e| TestBuilderError::ClassificationFailed(e.to_string()))?;

        // Plan the scaffolds.
        let plan = Self::plan(&story);

        // Classify and probe each scaffold, collecting verdicts.
        let mut verdicts = Vec::new();
        let mut recorded_with_verdicts = Vec::new();
        for entry in plan.iter() {
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

            // Classify the scaffold.
            let classification = self.classify_scaffold(&story, Path::new(&entry.file), &repo);

            // Determine the verdict string first.
            let verdict_string = match classification {
                ScaffoldClassification::Preserve => "preserved".to_string(),
                ScaffoldClassification::ReAuthor => "re-authored".to_string(),
                ScaffoldClassification::FirstAuthoring => "red".to_string(),
            };

            // Build the verdict entry based on classification.
            let verdict_entry = match classification {
                ScaffoldClassification::Preserve => {
                    // Preserved scaffolds are not probed; emit the preserved-shape verdict.
                    json!({
                        "file": entry.file,
                        "verdict": "preserved",
                    })
                }
                ScaffoldClassification::FirstAuthoring | ScaffoldClassification::ReAuthor => {
                    // First-authoring and re-author scaffolds must probe red.
                    // Probe the scaffold.
                    let (red_path, diagnostic) =
                        probe_scaffold(&self.repo_root, &entry.target_crate, &test_path)?;

                    // Check that the probe actually came back red.
                    if red_path == "green" {
                        return Err(TestBuilderError::ScaffoldNotRed {
                            file: test_path,
                            probe: "compile".to_string(),
                        });
                    }

                    json!({
                        "file": entry.file,
                        "verdict": &verdict_string,
                        "red_path": red_path,
                        "diagnostic": diagnostic,
                    })
                }
            };

            verdicts.push(verdict_entry);
            recorded_with_verdicts.push((PathBuf::from(&entry.file), verdict_string));
        }

        // Write evidence atomically.
        let run_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let commit = get_head_commit(&self.repo_root)?;

        let evidence_dir = self.repo_root.join(format!("evidence/runs/{story_id}"));

        // Construct the filename from timestamp.
        let filename = format!(
            "{}-red.jsonl",
            timestamp.replace(":", "-").replace(".", "-")
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

        Ok(RecordOutcome {
            recorded: plan.iter().map(|e| PathBuf::from(&e.file)).collect(),
            verdicts: recorded_with_verdicts,
        })
    }
}

impl RecordOutcome {
    /// Paths of files recorded.
    pub fn recorded_paths(&self) -> &[PathBuf] {
        &self.recorded
    }

    /// Paths and verdicts of files recorded.
    pub fn recorded_with_verdicts(&self) -> &[(PathBuf, String)] {
        &self.verdicts
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
            || path_normalized.starts_with("evidence/")
            || path_normalized == ".bin"
            || path_normalized == ".agentic-cache"
            || path_normalized == "target"
            || path_normalized == "evidence"
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
    test_file: &Path,
) -> Result<(String, String), TestBuilderError> {
    // Track what needs cleanup
    let cargo_lock_path = repo_root.join("Cargo.lock");
    let cargo_lock_existed = cargo_lock_path.exists();

    // Defect 2 fix: Run `cargo test` on the specific test file only.
    // This ensures that when probing multiple scaffolds in the same crate,
    // each scaffold's diagnostic is a snapshot of its own probe, not aliased
    // to another scaffold's output. Cargo will compile only the requested test.
    let test_name = test_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("*");
    let test_output = Command::new("cargo")
        .args([
            "test",
            "--package",
            crate_name,
            "--test",
            test_name,
            "--",
            "--nocapture",
        ])
        .current_dir(repo_root)
        .output()
        .map_err(|e| TestBuilderError::Other(format!("failed to run cargo test: {e}")))?;

    let stdout = String::from_utf8_lossy(&test_output.stdout);
    let stderr = String::from_utf8_lossy(&test_output.stderr);
    let combined: Vec<&str> = stdout.lines().chain(stderr.lines()).collect();

    // Defect 1 fix: Key on `cargo test`'s exit code as the authoritative signal
    // for compile vs runtime failure. If the test didn't compile, cargo test will
    // exit non-zero BEFORE trying to run the test. This is stable across rustc's
    // diagnostic-renderer ICE (rustc 1.95 StyledBuffer::replace panic).

    // First, look for panic output (runtime-red). This takes precedence because
    // if the test compiled, we trust the panic output.
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
            // Case B: newer format — collect the next few non-empty lines as the message.
            // For assert_eq failures, we want to capture "assertion ... failed", "left: X", "right: Y"
            let mut msg_lines = Vec::new();
            for next_line in combined[i + 1..].iter() {
                let trimmed = next_line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                msg_lines.push(trimmed);
                // Collect up to 3 lines of output for rich diagnostics.
                if msg_lines.len() >= 3 {
                    break;
                }
            }
            if !msg_lines.is_empty() {
                // Clean up Cargo.lock before returning
                if !cargo_lock_existed && cargo_lock_path.exists() {
                    let _ = fs::remove_file(&cargo_lock_path);
                }
                return Ok(("runtime".to_string(), msg_lines.join(" ")));
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

    // Test did not panic — the scaffold is green (success case).
    if test_output.status.success() {
        // Clean up Cargo.lock before returning
        if !cargo_lock_existed && cargo_lock_path.exists() {
            let _ = fs::remove_file(&cargo_lock_path);
        }
        return Ok(("green".to_string(), "test passed".to_string()));
    }

    // Test exited non-zero. Determine if it was compile-red or runtime-red.
    // Look for compile error patterns in the output.
    for line in combined.iter() {
        let trimmed = line.trim();
        if trimmed.contains("error[E") {
            // This is a compile error (or a rustc-style error in output).
            if !cargo_lock_existed && cargo_lock_path.exists() {
                let _ = fs::remove_file(&cargo_lock_path);
            }
            return Ok(("compile".to_string(), trimmed.to_string()));
        }
    }

    // Defect 3 fix: Test failed but we didn't find a clear panic or compile error.
    // Synthesize a meaningful diagnostic by capturing the first error/failure line,
    // or use the exit code. Never silently return a vacuous placeholder.
    for line in combined.iter() {
        let trimmed = line.trim();
        if !trimmed.is_empty()
            && (trimmed.to_lowercase().contains("failed")
                || trimmed.to_lowercase().contains("error")
                || trimmed.to_lowercase().contains("exit")
                || trimmed.to_lowercase().contains("abort"))
        {
            if !cargo_lock_existed && cargo_lock_path.exists() {
                let _ = fs::remove_file(&cargo_lock_path);
            }
            return Ok(("runtime".to_string(), trimmed.to_string()));
        }
    }

    // Last resort: capture exit code in diagnostic rather than a vacuous placeholder.
    if !cargo_lock_existed && cargo_lock_path.exists() {
        let _ = fs::remove_file(&cargo_lock_path);
    }
    Ok((
        "runtime".to_string(),
        format!(
            "test exited with code {:?} but captured no output",
            test_output.status.code()
        ),
    ))
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
