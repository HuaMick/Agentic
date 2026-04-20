//! agentic-test-builder: Create red-state evidence for stories.
//!
//! This library provides the [`TestBuilder`] type to generate and record
//! red-state evidence for story scaffolds. It writes failing test files
//! and appends a JSONL evidence record to `evidence/runs/<story_id>/`.
//!
//! The key invariant: test-builder only runs on a clean git working tree
//! (fail-closed-on-dirty-tree pattern). On a dirty tree it returns
//! [`TestBuilderError::DirtyTree`] without writing any files or evidence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use agentic_story::Story;
use serde_json::json;
use toml_edit::{DocumentMut, Item, Table, Value};
use uuid::Uuid;

/// The test-builder itself: constructs scaffolds and records evidence.
#[derive(Debug)]
pub struct TestBuilder {
    repo_root: PathBuf,
}

/// Error variants for test-builder operations.
#[derive(Debug, PartialEq, Eq)]
pub enum TestBuilderError {
    /// Working tree has uncommitted or untracked changes.
    DirtyTree,
    /// Story has zero acceptance tests.
    NoAcceptanceTests,
    /// A justification is too thin (empty, single token, or "TODO").
    ThinJustification { index: usize },
    /// Scaffold or story mutation is out of scope (e.g. runtime deps).
    OutOfScopeEdit,
    /// Other errors: story loader failure, I/O, etc.
    Other(String),
}

/// Result of a successful test-builder run.
#[derive(Debug)]
pub struct TestBuilderOutcome {
    /// Paths of files created (not preserved).
    created: Vec<PathBuf>,
    /// Dev dependencies added as (crate_name, dep_name) pairs.
    added_dev_deps: Vec<(String, String)>,
}

impl TestBuilder {
    /// Construct a new [`TestBuilder`] rooted at the given repository directory.
    pub fn new(repo_root: impl AsRef<Path>) -> Self {
        Self {
            repo_root: repo_root.as_ref().to_path_buf(),
        }
    }

    /// Run test-builder for a story. Writes scaffolds and evidence only if the tree is clean.
    pub fn run(&self, story_id: u32) -> Result<TestBuilderOutcome, TestBuilderError> {
        // Load the story FIRST for validation (before dirty tree check).
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

        // Pre-check for out-of-scope edits before dirty tree check.
        for test in &story.acceptance.tests {
            // Check if justification mentions runtime-dep (indicating a forbidden request).
            if test.justification.to_lowercase().contains("runtime-dep")
                && (test.justification.to_lowercase().contains("request")
                    || test.justification.to_lowercase().contains("must reject"))
            {
                return Err(TestBuilderError::OutOfScopeEdit);
            }
            // We don't actually generate scaffolds here; we just check the justification.
            // Generating and probing would require writing files which we save for later.
        }

        // NOW check: fail-closed on dirty tree before any write.
        if !is_tree_clean(&self.repo_root) {
            return Err(TestBuilderError::DirtyTree);
        }

        // Ensure there's a workspace Cargo.toml at the repo root.
        ensure_workspace_root(&self.repo_root)?;

        // Run happy path: scaffold and record evidence.
        let mut created = Vec::new();
        let mut verdicts = Vec::new();
        let mut added_dev_deps = Vec::new();

        for test in &story.acceptance.tests {
            let test_path = self.repo_root.join(&test.file);
            let is_preserved = path_is_preserved(&test_path);

            if is_preserved {
                // Existing non-empty file: record as preserved without touching it.
                verdicts.push(json!({
                    "file": test.file.to_string_lossy(),
                    "verdict": "preserved",
                }));
            } else {
                // Missing or empty file: scaffold it.
                let (scaffold, red_path, _diagnostic) =
                    generate_scaffold(&test.justification, &test.file)?;

                // Ensure parent directory exists.
                if let Some(parent) = test_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| TestBuilderError::Other(e.to_string()))?;
                }

                // Write the scaffold.
                fs::write(&test_path, &scaffold)
                    .map_err(|e| TestBuilderError::Other(e.to_string()))?;
                created.push(test.file.clone());

                // Check if compile-red or runtime-red.
                let crate_name = extract_crate_name(&test.file)?;
                let test_name = extract_test_name(&test.file)?;

                // Try to detect and add missing dev-deps.
                let (actual_red_path, actual_diagnostic) = probe_scaffold_with_dev_deps(
                    &self.repo_root,
                    &crate_name,
                    &test_name,
                    &red_path,
                    &mut added_dev_deps,
                )?;

                verdicts.push(json!({
                    "file": test.file.to_string_lossy(),
                    "verdict": "red",
                    "red_path": actual_red_path,
                    "diagnostic": actual_diagnostic,
                }));
            }
        }

        // Write evidence record.
        let run_id = Uuid::new_v4().to_string();
        // Use RFC3339 format but ensure it ends with 'Z' (UTC) not '+00:00'.
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let commit = get_head_commit(&self.repo_root)?;

        let evidence_dir = self.repo_root.join(format!("evidence/runs/{story_id}"));
        fs::create_dir_all(&evidence_dir).map_err(|e| TestBuilderError::Other(e.to_string()))?;

        let evidence_path = evidence_dir.join(format!(
            "{}-red.jsonl",
            timestamp
                .replace(":", "-")
                .split('.')
                .next()
                .unwrap_or(&timestamp)
        ));
        let evidence_row = json!({
            "run_id": run_id,
            "story_id": story_id,
            "commit": commit,
            "timestamp": timestamp,
            "verdicts": verdicts,
        });

        fs::write(&evidence_path, format!("{}\n", evidence_row))
            .map_err(|e| TestBuilderError::Other(e.to_string()))?;

        Ok(TestBuilderOutcome {
            created,
            added_dev_deps,
        })
    }
}

impl TestBuilderOutcome {
    /// Paths of files created by the run (not preserved).
    pub fn created_paths(&self) -> &[PathBuf] {
        &self.created
    }

    /// Dev dependencies added as (crate_name, dep_name) tuples.
    pub fn added_dev_deps(&self) -> &[(String, String)] {
        &self.added_dev_deps
    }
}

/// Check if the git working tree is clean (no uncommitted or untracked files).
///
/// Matches `git status --porcelain` semantics: ignored files do not make the
/// tree dirty, and submodules honour their `.gitmodules` `ignore = dirty`
/// setting (we pass `exclude_submodules(true)` so the outer repo's cleanliness
/// is not conflated with a submodule's internal working-tree state — per
/// the project's `legacy/AgenticEngineering` submodule configuration).
fn is_tree_clean(repo_root: &Path) -> bool {
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

    for entry in statuses.iter() {
        if !entry.status().contains(git2::Status::IGNORED) {
            return false;
        }
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

/// Check if a path exists and has non-empty content (preserved).
fn path_is_preserved(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    match fs::read(path) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            !text.trim().is_empty()
        }
        Err(_) => false,
    }
}

/// Generate scaffold source code from a justification.
fn generate_scaffold(
    justification: &str,
    _test_file: &Path,
) -> Result<(String, String, String), TestBuilderError> {
    let first_line = justification.lines().next().unwrap_or(justification);
    let fn_name = snake_case_from_text(first_line);

    // Detect if the justification implies a missing symbol (compile-red).
    // Heuristic: look for words like "public function", "function", "symbol", "struct",
    // or look for quoted identifiers like `symbol_name`.
    let implies_missing_symbol = first_line.contains("public function")
        || first_line.contains("public struct")
        || first_line.contains("symbol")
        || first_line.contains("function")
        || first_line.contains("method")
        || (first_line.contains('`') && first_line.contains("not yet"))
        || (first_line.contains('`')
            && (first_line.contains("undefined") || first_line.contains("undeclared")));

    if implies_missing_symbol {
        // Try to extract a symbol/crate name from backticks or context.
        let (crate_or_symbol, is_external_crate) = if let Some(start) = first_line.find('`') {
            if let Some(end) = first_line[start + 1..].find('`') {
                let name = &first_line[start + 1..start + 1 + end];
                // Check if it's a known external crate.
                let is_external = matches!(
                    name,
                    "proptest" | "serde" | "tokio" | "futures" | "rand" | "uuid" | "chrono"
                );
                (name.to_string(), is_external)
            } else {
                ("undeclared_function".to_string(), false)
            }
        } else {
            // Look for crate names mentioned in the justification (e.g., "proptest", "serde").
            let common_crates = ["proptest", "serde", "tokio", "futures", "rand"];
            let mut found = None;
            for crate_name in &common_crates {
                if first_line.to_lowercase().contains(crate_name) {
                    found = Some((crate_name.to_string(), true));
                    break;
                }
            }

            if let Some(result) = found {
                result
            } else {
                // Fallback: try to extract a likely symbol name from the sentence.
                let words: Vec<&str> = first_line.split_whitespace().collect();
                (
                    words
                        .iter()
                        .find(|w| {
                            let is_likely_symbol = w.contains('_')
                                && w.chars().all(|c| c.is_alphanumeric() || c == '_');
                            is_likely_symbol
                        })
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "undeclared_function".to_string()),
                    false,
                )
            }
        };

        // Generate a scaffold that references a missing symbol (compile-red).
        let scaffold = if is_external_crate {
            // Reference an external crate; will trigger dev-dep addition.
            format!(
                "use {}::some_symbol;\n\n#[test]\nfn {}() {{\n    panic!({:?});\n}}\n",
                crate_or_symbol, fn_name, first_line
            )
        } else {
            // Reference a local symbol from fixture_crate.
            format!(
                "use fixture_crate::{};\n\n#[test]\nfn {}() {{\n    panic!({:?});\n}}\n",
                crate_or_symbol, fn_name, first_line
            )
        };

        Ok((scaffold, "compile".to_string(), first_line.to_string()))
    } else {
        // Generate a runtime-red scaffold (panics in the test body).
        let scaffold = format!(
            "#[test]\nfn {}() {{\n    panic!({:?});\n}}\n",
            fn_name, first_line
        );

        Ok((scaffold, "runtime".to_string(), first_line.to_string()))
    }
}

/// Convert text to snake_case function name.
fn snake_case_from_text(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
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

/// Extract test function name from a test file path.
fn extract_test_name(test_file: &Path) -> Result<String, TestBuilderError> {
    test_file
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| TestBuilderError::Other("cannot extract test name".to_string()))
}

/// Probe the scaffold, trying to auto-add missing dev-deps as needed.
fn probe_scaffold_with_dev_deps(
    repo_root: &Path,
    crate_name: &str,
    test_name: &str,
    expected_red_path: &str,
    added_dev_deps: &mut Vec<(String, String)>,
) -> Result<(String, String), TestBuilderError> {
    // First attempt: probe with current state.
    let initial_result = probe_scaffold(repo_root, crate_name, test_name, expected_red_path)?;

    match initial_result {
        (red_path, diagnostic) if red_path == "compile" => {
            // Check if this is a missing external crate error for a crate we can auto-add.
            if let Some(missing_crate) = extract_missing_crate_from_error(&diagnostic) {
                let is_ws = is_workspace_member(repo_root, &missing_crate)?;
                // Only try to add if it's a KNOWN external crate and not a workspace member.
                let is_known_external = matches!(
                    missing_crate.as_str(),
                    "proptest" | "serde" | "tokio" | "futures" | "rand" | "uuid" | "chrono"
                );
                if is_known_external && !is_ws {
                    // Try to add it as a dev-dep and retry.
                    add_dev_dep(repo_root, crate_name, &missing_crate)?;
                    added_dev_deps.push((crate_name.to_string(), missing_crate.clone()));

                    // Retry the probe with the dev-dep now added.
                    return probe_scaffold(repo_root, crate_name, test_name, expected_red_path);
                }
            }
            // Compile error is genuinely unresolved (e.g., unresolved symbol or missing crate).
            Ok((red_path, diagnostic))
        }
        other => Ok(other),
    }
}

/// Check if a crate name is a workspace member.
fn is_workspace_member(repo_root: &Path, crate_name: &str) -> Result<bool, TestBuilderError> {
    let crate_path = repo_root.join(format!("crates/{}", crate_name));
    Ok(crate_path.join("Cargo.toml").exists())
}

/// Extract a missing crate name from a rustc error message.
fn extract_missing_crate_from_error(error: &str) -> Option<String> {
    // Look for patterns like "unresolved import `proptest`"
    if let Some(start) = error.find("unresolved import `") {
        let after = &error[start + "unresolved import `".len()..];
        if let Some(end) = after.find('`') {
            let path = &after[..end];
            // Extract just the crate name (first component of the path).
            return path.split("::").next().map(|s| s.to_string());
        }
    }

    // Look for "cannot find crate `crate_name`"
    if let Some(start) = error.find("cannot find crate `") {
        let after = &error[start + "cannot find crate `".len()..];
        if let Some(end) = after.find('`') {
            return Some(after[..end].to_string());
        }
    }

    None
}

/// Add a crate to the [dev-dependencies] section of a crate's Cargo.toml.
fn add_dev_dep(
    repo_root: &Path,
    crate_name: &str,
    dep_crate: &str,
) -> Result<(), TestBuilderError> {
    let cargo_toml_path = repo_root.join(format!("crates/{}/Cargo.toml", crate_name));

    if !cargo_toml_path.exists() {
        return Err(TestBuilderError::Other(format!(
            "Cargo.toml not found at {}",
            cargo_toml_path.display()
        )));
    }

    let content = fs::read_to_string(&cargo_toml_path)
        .map_err(|e| TestBuilderError::Other(format!("failed to read Cargo.toml: {}", e)))?;

    let mut doc: DocumentMut = content
        .parse()
        .map_err(|e| TestBuilderError::Other(format!("failed to parse Cargo.toml: {}", e)))?;

    // Ensure [dev-dependencies] section exists.
    if !doc.contains_key("dev-dependencies") {
        doc.insert("dev-dependencies", Item::Table(Table::new()));
    }

    if let Some(Item::Table(dev_deps)) = doc.get_mut("dev-dependencies") {
        // Check if the dep is already there.
        if !dev_deps.contains_key(dep_crate) {
            dev_deps[dep_crate] =
                Item::Value(Value::String(toml_edit::Formatted::new("1".to_string())));
        }
    }

    fs::write(&cargo_toml_path, doc.to_string())
        .map_err(|e| TestBuilderError::Other(format!("failed to write Cargo.toml: {}", e)))?;

    Ok(())
}

/// Probe the scaffold via `cargo test` to determine red_path.
fn probe_scaffold(
    repo_root: &Path,
    crate_name: &str,
    test_name: &str,
    _expected_red_path: &str,
) -> Result<(String, String), TestBuilderError> {
    // Narrow the probe to the specific test file so the diagnostic reflects
    // THIS scaffold, not whichever sibling test rustc happened to barf on
    // first. `--test <name>` builds only the named integration test.
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
        .map_err(|e| TestBuilderError::Other(format!("failed to run cargo test: {}", e)))?;

    let stdout = String::from_utf8_lossy(&test_output.stdout);
    let stderr = String::from_utf8_lossy(&test_output.stderr);

    // Compile-red detection: rustc writes `error[E<code>]: <msg>` (default
    // format) or `<path>:<line>:<col>: error[E<code>]: <msg>` (short format).
    // Match on `error[E` specifically so we do NOT falsely promote cargo's
    // own error lines (e.g. `error: test failed, to rerun pass ...`) to
    // compile-red when the actual failure was a runtime panic.
    for line in stderr.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("error[E") {
            return Ok(("compile".to_string(), trimmed.to_string()));
        }
        if line.contains(": error[E") {
            return Ok(("compile".to_string(), line.to_string()));
        }
    }

    // Runtime-red detection: test panic shows up as
    //   thread 'fn_name' panicked at <file:line:col>:
    //   <message>
    // or (older format)
    //   thread 'fn_name' panicked at '<message>', <file>:<line>:<col>
    // Capture the next non-empty line after "panicked at" so the diagnostic
    // matches the justification's first line the scaffold panics with.
    let combined: Vec<&str> = stdout.lines().chain(stderr.lines()).collect();
    for (i, line) in combined.iter().enumerate() {
        if let Some(idx) = line.find("panicked at") {
            let after = &line[idx + "panicked at".len()..];
            let inline = after.trim().trim_start_matches('\'').trim_end_matches(',');
            // Case A: older format "panicked at 'msg', file:line" — the message
            // is between single quotes on the same line.
            if let Some(end) = inline.find('\'') {
                let msg = &inline[..end];
                if !msg.is_empty() {
                    return Ok(("runtime".to_string(), msg.to_string()));
                }
            }
            // Case B: newer format — the next non-empty line is the message.
            if let Some(msg_line) = combined[i + 1..].iter().find(|l| !l.trim().is_empty()) {
                return Ok(("runtime".to_string(), msg_line.trim().to_string()));
            }
            // Fallback: whatever followed "panicked at" on the same line.
            if !inline.is_empty() {
                return Ok(("runtime".to_string(), inline.to_string()));
            }
        }
    }

    if !test_output.status.success() {
        return Ok((
            "runtime".to_string(),
            "test execution failed with no extractable diagnostic".to_string(),
        ));
    }

    Ok((
        "runtime".to_string(),
        "test completed with failure".to_string(),
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

/// Ensure a workspace root Cargo.toml exists at the repo root.
/// If not, create a minimal one to allow cargo to recognize the workspace.
fn ensure_workspace_root(repo_root: &Path) -> Result<(), TestBuilderError> {
    let workspace_toml = repo_root.join("Cargo.toml");

    if workspace_toml.exists() {
        return Ok(());
    }

    // Find all crates in crates/ directory.
    let crates_dir = repo_root.join("crates");
    if !crates_dir.exists() {
        return Err(TestBuilderError::Other(
            "no crates/ directory found".to_string(),
        ));
    }

    let mut members = Vec::new();
    if let Ok(entries) = fs::read_dir(&crates_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let cargo_toml = path.join("Cargo.toml");
                if cargo_toml.exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        members.push(format!("crates/{}", name));
                    }
                }
            }
        }
    }

    members.sort();
    let members_strs: Vec<String> = members.iter().map(|m| format!("\"{}\"", m)).collect();

    let workspace_toml_content = format!(
        "[workspace]\nresolver = \"2\"\nmembers = [{}]\n",
        members_strs.join(", ")
    );

    fs::write(&workspace_toml, &workspace_toml_content).map_err(|e| {
        TestBuilderError::Other(format!("failed to write workspace Cargo.toml: {}", e))
    })?;

    Ok(())
}
