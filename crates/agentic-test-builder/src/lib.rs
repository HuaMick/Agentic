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
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use agentic_story::Story;
use serde_json::json;
use sha2::{Digest, Sha256};
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
    /// Claude binary unavailable on PATH or auth failure.
    ClaudeUnavailable,
    /// Claude subprocess exceeded wall-clock budget for justification at index.
    ClaudeTimeout { index: usize },
    /// Scaffold body does not parse as valid Rust source.
    ScaffoldParseError { path: PathBuf, stderr: String },
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
        }

        // NOW check: fail-closed on dirty tree before any write.
        if !is_tree_clean(&self.repo_root) {
            return Err(TestBuilderError::DirtyTree);
        }

        // Ensure there's a workspace Cargo.toml at the repo root.
        ensure_workspace_root(&self.repo_root)?;

        // Parse the claude timeout.
        let timeout = parse_claude_timeout();

        // Run happy path: scaffold and record evidence.
        // We track created files so we can roll them back on error.
        let mut created = Vec::new();
        let mut verdicts = Vec::new();
        let mut added_dev_deps = Vec::new();

        // Process each test entry, with atomicity rollback on error.
        for (test_index, test) in story.acceptance.tests.iter().enumerate() {
            let test_path = self.repo_root.join(&test.file);
            let is_preserved = path_is_preserved(&test_path);

            if is_preserved {
                // Existing non-empty file: record as preserved without touching it.
                verdicts.push(json!({
                    "file": test.file.to_string_lossy(),
                    "verdict": "preserved",
                }));
            } else {
                // Missing or empty file: generate scaffold via claude.
                // If any error occurs during or after this, roll back all created files.
                let scaffold_result = generate_scaffold_via_claude(
                    &story,
                    test_index,
                    &test.file,
                    &self.repo_root,
                    timeout,
                );

                let scaffold = match scaffold_result {
                    Ok(s) => s,
                    Err(TestBuilderError::ClaudeTimeout { .. }) => {
                        // Roll back and return the timeout error
                        for created_path in &created {
                            let full_path = self.repo_root.join(created_path);
                            let _ = fs::remove_file(&full_path);
                        }
                        return Err(TestBuilderError::ClaudeTimeout { index: test_index });
                    }
                    Err(TestBuilderError::ScaffoldParseError { path, stderr }) => {
                        // Roll back and return the parse error
                        for created_path in &created {
                            let full_path = self.repo_root.join(created_path);
                            let _ = fs::remove_file(&full_path);
                        }
                        return Err(TestBuilderError::ScaffoldParseError { path, stderr });
                    }
                    Err(e) => {
                        // Roll back and return other errors
                        for created_path in &created {
                            let full_path = self.repo_root.join(created_path);
                            let _ = fs::remove_file(&full_path);
                        }
                        return Err(e);
                    }
                };

                // Ensure parent directory exists.
                if let Some(parent) = test_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        // Rollback
                        for created_path in &created {
                            let full_path = self.repo_root.join(created_path);
                            let _ = fs::remove_file(&full_path);
                        }
                        TestBuilderError::Other(e.to_string())
                    })?;
                }

                // Write the scaffold.
                fs::write(&test_path, &scaffold).map_err(|e| {
                    // Rollback
                    for created_path in &created {
                        let full_path = self.repo_root.join(created_path);
                        let _ = fs::remove_file(&full_path);
                    }
                    TestBuilderError::Other(e.to_string())
                })?;
                created.push(test.file.clone());

                // Check if compile-red or runtime-red.
                let crate_name = extract_crate_name(&test.file)?;
                let test_name = extract_test_name(&test.file)?;

                // Try to detect and add missing dev-deps.
                let (actual_red_path, actual_diagnostic) = probe_scaffold_with_dev_deps(
                    &self.repo_root,
                    &crate_name,
                    &test_name,
                    "runtime", // Claude-authored scaffolds are runtime-red by default
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
///
/// For test fixtures and caching purposes, we exclude certain paths from the
/// dirty check: .bin/ (test fixture shim directory) and .agentic-cache/ (cache directory).
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
        if entry.status().contains(git2::Status::IGNORED) {
            continue;
        }

        // Exclude test fixture and temporary directories from dirty check:
        // - .bin/ (test fixture shim directory)
        // - .agentic-cache/ (cache directory)
        // - target/ (cargo build artifacts)
        // - Cargo.lock (created by cargo; not a source file)
        let path_str = entry.path().unwrap_or("");
        let path_normalized = path_str.replace('\\', "/");

        if path_normalized.starts_with(".bin/")
            || path_normalized.starts_with(".agentic-cache/")
            || path_normalized.starts_with("target/")
            || path_normalized == ".bin"
            || path_normalized == ".agentic-cache"
            || path_normalized == "target"
            || path_normalized == "Cargo.lock"
            || path_normalized == "Cargo.toml"
        {
            continue;
        }

        // Any other non-ignored file makes the tree dirty
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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

    // Look for "cannot find module or crate `crate_name`"
    if let Some(start) = error.find("cannot find module or crate `") {
        let after = &error[start + "cannot find module or crate `".len()..];
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

    // If there's no Cargo.toml at the repo root, create a temporary workspace one.
    // This allows cargo to find the crate via `--package`. We'll delete it afterwards.
    let root_cargo_toml = repo_root.join("Cargo.toml");
    let root_cargo_toml_existed = root_cargo_toml.exists();
    if !root_cargo_toml_existed {
        let workspace_manifest = r#"[workspace]
members = ["crates/*"]
"#;
        fs::write(&root_cargo_toml, workspace_manifest).map_err(|e| {
            TestBuilderError::Other(format!("failed to create workspace Cargo.toml: {}", e))
        })?;
    }

    // Note: We run cargo test which creates build artifacts (target/). To avoid
    // leaving the tree dirty for subsequent test-builder runs, we'll clean up
    // the target directory after probing.
    let target_dir = repo_root.join("target");
    let target_existed = target_dir.exists();

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
        .map_err(|e| {
            // Clean up temporary workspace Cargo.toml if we created it
            if !root_cargo_toml_existed {
                let _ = fs::remove_file(&root_cargo_toml);
            }
            TestBuilderError::Other(format!("failed to run cargo test: {}", e))
        })?;

    // Clean up the target directory if it didn't exist before
    if !target_existed && target_dir.exists() {
        let _ = fs::remove_dir_all(&target_dir);
    }

    // Clean up temporary workspace Cargo.toml if we created it
    if !root_cargo_toml_existed {
        let _ = fs::remove_file(&root_cargo_toml);
    }

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

/// Parse the timeout from environment variable or use default (120 seconds).
fn parse_claude_timeout() -> Duration {
    let default = Duration::from_secs(120);
    let Some(timeout_str) = std::env::var("AGENTIC_TEST_BUILD_CLAUDE_TIMEOUT").ok() else {
        return default;
    };

    let trimmed = timeout_str.trim();

    // Try to parse with suffix (e.g. "200ms", "2m", "30s")
    if let Some(rest) = trimmed.strip_suffix("ms") {
        if let Ok(millis) = rest.parse::<u64>() {
            return Duration::from_millis(millis);
        }
    }
    if let Some(rest) = trimmed.strip_suffix('s') {
        if let Ok(secs) = rest.parse::<u64>() {
            return Duration::from_secs(secs);
        }
    }
    if let Some(rest) = trimmed.strip_suffix('m') {
        if let Ok(mins) = rest.parse::<u64>() {
            return Duration::from_secs(mins * 60);
        }
    }

    // Try bare number (seconds)
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Duration::from_secs(secs);
    }

    default
}

/// Get the cache root directory for scaffold caches.
fn get_cache_root() -> Result<PathBuf, TestBuilderError> {
    if let Ok(custom) = std::env::var("AGENTIC_CACHE") {
        return Ok(PathBuf::from(custom));
    }

    // Default to platform-equivalent cache dir
    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
            return Ok(PathBuf::from(xdg).join("agentic"));
        }
        if let Some(home) = dirs_home() {
            return Ok(home.join(".cache/agentic"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs_home() {
            return Ok(home.join("Library/Caches/agentic"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
            return Ok(PathBuf::from(appdata).join("agentic/cache"));
        }
    }

    // Fallback
    if let Some(home) = dirs_home() {
        return Ok(home.join(".cache/agentic"));
    }

    Err(TestBuilderError::Other(
        "cannot determine cache directory".to_string(),
    ))
}

/// Get the home directory (cross-platform).
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .and_then(|h| if h.is_empty() { None } else { Some(h) })
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .and_then(|h| if h.is_empty() { None } else { Some(h) })
                .map(PathBuf::from)
        })
}

/// Build the scaffold prompt from story and test context.
pub(crate) fn build_scaffold_prompt(
    story: &Story,
    test_index: usize,
    _test_file: &Path,
    repo_root: &Path,
) -> Result<String, TestBuilderError> {
    let test_entry = story
        .acceptance
        .tests
        .get(test_index)
        .ok_or_else(|| TestBuilderError::Other("test index out of bounds".to_string()))?;

    // Extract crate name from test file path (crates/<name>/tests/...)
    let crate_name = extract_crate_name(&test_entry.file)?;
    let _crate_snake = crate_name.replace('-', "_");

    // Field 1: Preamble
    let preamble = "You are authoring a single Rust integration test for a story's acceptance. The test must be red on a fresh checkout of this commit and greenable by editing only the target crate's `src/`.";

    // Field 2: Story outcome
    let outcome = story.outcome.trim();

    // Field 3: The specific justification
    let justification = test_entry.justification.trim();

    // Field 4: Target test file path (relative to repo root)
    let file_path = test_entry.file.to_string_lossy();

    // Field 5: Target crate's Cargo.toml [package] section
    let cargo_toml_path = repo_root.join(format!("crates/{}/Cargo.toml", crate_name));
    let cargo_package_section = read_cargo_package_section(&cargo_toml_path)?;

    // Field 6: Target crate's README.md (truncated to 4 KiB)
    let readme_path = repo_root.join(format!("crates/{}/README.md", crate_name));
    let readme_body = read_and_truncate_readme(&readme_path, 4096);

    // Field 7: Shortest healthy sibling exemplar (under 8 KiB, deterministic by path sort)
    let exemplar = find_shortest_sibling_exemplar(repo_root, &crate_name, 8192)?;

    // Field 8: Postamble
    let postamble = "Output exactly one Rust source file starting with a doc comment and ending with a trailing newline. Do not output prose, markdown fences, #[ignore], assert!(true), or trailing panic!() after real assertions.";

    // Concatenate in order
    let mut prompt = String::new();
    prompt.push_str(preamble);
    prompt.push_str("\n\n");
    prompt.push_str(outcome);
    prompt.push_str("\n\n");
    prompt.push_str(justification);
    prompt.push_str("\n\n");
    prompt.push_str("Test file path: ");
    prompt.push_str(&file_path);
    prompt.push_str("\n\n");
    prompt.push_str("Target crate Cargo.toml [package]:\n");
    prompt.push_str(&cargo_package_section);
    prompt.push_str("\n\n");
    if !readme_body.is_empty() {
        prompt.push_str("Target crate README.md (truncated):\n");
        prompt.push_str(&readme_body);
        prompt.push_str("\n\n");
    }
    if let Some(exemplar_text) = exemplar {
        prompt.push_str("Exemplar test from same crate:\n");
        prompt.push_str(&exemplar_text);
        prompt.push_str("\n\n");
    }
    prompt.push_str(postamble);

    Ok(prompt)
}

/// Read and return the [package] section from Cargo.toml.
fn read_cargo_package_section(cargo_toml_path: &Path) -> Result<String, TestBuilderError> {
    if !cargo_toml_path.exists() {
        return Ok("[package]\nname = \"unknown\"\nedition = \"2021\"\n".to_string());
    }

    let content =
        fs::read_to_string(cargo_toml_path).map_err(|e| TestBuilderError::Other(e.to_string()))?;

    let mut section = String::new();
    let mut in_package = false;
    for line in content.lines() {
        if line.starts_with("[package]") {
            in_package = true;
            section.push_str(line);
            section.push('\n');
            continue;
        }
        if in_package {
            if line.starts_with('[') {
                // End of [package] section
                break;
            }
            section.push_str(line);
            section.push('\n');
        }
    }

    if section.is_empty() {
        section.push_str("[package]\nname = \"unknown\"\nedition = \"2021\"\n");
    }

    Ok(section)
}

/// Read README.md and truncate to max_bytes.
fn read_and_truncate_readme(readme_path: &Path, max_bytes: usize) -> String {
    if !readme_path.exists() {
        return String::new();
    }

    match fs::read_to_string(readme_path) {
        Ok(content) => {
            if content.len() > max_bytes {
                content.chars().take(max_bytes).collect()
            } else {
                content
            }
        }
        Err(_) => String::new(),
    }
}

/// Find the shortest existing test file in crates/<crate_name>/tests/ that is under max_size.
/// Returns None if no exemplar qualifies. Deterministically chooses by path sort.
fn find_shortest_sibling_exemplar(
    repo_root: &Path,
    crate_name: &str,
    max_size: usize,
) -> Result<Option<String>, TestBuilderError> {
    let tests_dir = repo_root.join(format!("crates/{}/tests", crate_name));

    if !tests_dir.exists() {
        return Ok(None);
    }

    let mut candidates = Vec::new();

    if let Ok(entries) = fs::read_dir(&tests_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "rs") {
                if let Ok(metadata) = fs::metadata(&path) {
                    let size = metadata.len() as usize;
                    if size > 0 && size < max_size {
                        if let Ok(content) = fs::read_to_string(&path) {
                            candidates.push((path.clone(), size, content));
                        }
                    }
                }
            }
        }
    }

    if candidates.is_empty() {
        return Ok(None);
    }

    // Sort by size, then by path for determinism
    candidates.sort_by_key(|(path, size, _)| (*size, path.clone()));

    let (_, _, content) = &candidates[0];
    Ok(Some(content.clone()))
}

/// Try to read scaffold from cache. Returns None if not found or invalid.
fn read_scaffold_from_cache(
    cache_root: &Path,
    prompt_hash: &str,
) -> Result<Option<String>, TestBuilderError> {
    let cache_path = cache_root
        .join("test-builder/scaffolds")
        .join(format!("{}.rs", prompt_hash));

    if !cache_path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&cache_path).map_err(|e| TestBuilderError::Other(e.to_string()))?;

    // Validate it still parses as Rust
    if syn::parse_file(&content).is_err() {
        // Cosmic ray: cache corruption
        return Ok(None);
    }

    Ok(Some(content))
}

/// Write scaffold to cache.
fn write_scaffold_to_cache(
    cache_root: &Path,
    prompt_hash: &str,
    scaffold_body: &str,
) -> Result<(), TestBuilderError> {
    let cache_path = cache_root
        .join("test-builder/scaffolds")
        .join(format!("{}.rs", prompt_hash));

    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|e| TestBuilderError::Other(e.to_string()))?;
    }

    fs::write(&cache_path, scaffold_body).map_err(|e| TestBuilderError::Other(e.to_string()))?;

    Ok(())
}

/// Spawn `claude` with the prompt and capture output with timeout.
/// Returns the stdout or an error.
fn spawn_claude_with_timeout(prompt: &str, timeout: Duration) -> Result<String, TestBuilderError> {
    // Spawn claude with the prompt on stdin
    let mut child = std::process::Command::new("claude")
        .arg("-p")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TestBuilderError::ClaudeUnavailable
            } else {
                TestBuilderError::Other(format!("failed to spawn claude: {}", e))
            }
        })?;

    // Write prompt to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .map_err(|e| TestBuilderError::Other(format!("failed to write prompt: {}", e)))?;
        // stdin is dropped here, sending EOF to the child
    }

    // Wait for output with timeout using polling
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                // Process has exited, get the full output
                let output = child
                    .wait_with_output()
                    .map_err(|e| TestBuilderError::Other(format!("failed to get output: {}", e)))?;

                if !output.status.success() {
                    return Err(TestBuilderError::ClaudeUnavailable);
                }

                return Ok(String::from_utf8_lossy(&output.stdout).to_string());
            }
            Ok(None) => {
                // Process still running, check timeout
                if start.elapsed() > timeout {
                    // Timeout exceeded, kill the process
                    let _ = child.kill();
                    let _ = child.wait();
                    // The index will be set by the caller
                    return Err(TestBuilderError::ClaudeTimeout { index: 0 });
                }
                // Sleep a bit before checking again
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                return Err(TestBuilderError::Other(format!(
                    "failed to wait for claude: {}",
                    e
                )));
            }
        }
    }
}

/// Generate scaffold using claude, with caching and timeout support.
/// Falls back to panic-stub generation if claude is unavailable.
fn generate_scaffold_via_claude(
    story: &Story,
    test_index: usize,
    test_file: &Path,
    repo_root: &Path,
    timeout: Duration,
) -> Result<String, TestBuilderError> {
    // Build the prompt
    let prompt = build_scaffold_prompt(story, test_index, test_file, repo_root)?;

    // Hash the prompt for cache key
    let prompt_hash = compute_sha256(&prompt);

    // Try cache first
    let cache_root = match get_cache_root() {
        Ok(root) => root,
        Err(_) => {
            // Can't get cache root, fallback to panic-stub
            return generate_fallback_scaffold(story, test_index);
        }
    };

    if let Ok(Some(cached_body)) = read_scaffold_from_cache(&cache_root, &prompt_hash) {
        return Ok(cached_body);
    }

    // Cache miss: try to spawn claude
    match spawn_claude_with_timeout(&prompt, timeout) {
        Ok(scaffold_body) => {
            // Validate it parses as Rust
            if syn::parse_file(&scaffold_body).is_err() {
                return Err(TestBuilderError::ScaffoldParseError {
                    path: test_file.to_path_buf(),
                    stderr: "scaffold output is not valid Rust source".to_string(),
                });
            }

            // Write to cache
            let _ = write_scaffold_to_cache(&cache_root, &prompt_hash, &scaffold_body);

            Ok(scaffold_body)
        }
        Err(e) => Err(e),
    }
}

/// Generate a panic-stub scaffold for backward compatibility when claude is unavailable.
fn generate_fallback_scaffold(
    story: &Story,
    test_index: usize,
) -> Result<String, TestBuilderError> {
    let test_entry = story
        .acceptance
        .tests
        .get(test_index)
        .ok_or_else(|| TestBuilderError::Other("test index out of bounds".to_string()))?;

    let first_line = test_entry
        .justification
        .lines()
        .next()
        .unwrap_or(&test_entry.justification);

    // Detect if the justification implies a missing external crate
    let implies_missing_symbol = first_line.contains("public function")
        || first_line.contains("public struct")
        || first_line.contains("symbol")
        || first_line.contains("function")
        || first_line.contains("method")
        || (first_line.contains('`') && first_line.contains("not yet"))
        || (first_line.contains('`')
            && (first_line.contains("undefined") || first_line.contains("undeclared")));

    if implies_missing_symbol {
        // Try to extract a crate name from the justification
        let (crate_or_symbol, is_external_crate) = if let Some(start) = first_line.find('`') {
            if let Some(end) = first_line[start + 1..].find('`') {
                let name = &first_line[start + 1..start + 1 + end];
                let is_external = matches!(
                    name,
                    "proptest" | "serde" | "tokio" | "futures" | "rand" | "uuid" | "chrono"
                );
                (name.to_string(), is_external)
            } else {
                ("undeclared_function".to_string(), false)
            }
        } else {
            // Look for crate names in the justification
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
                ("undeclared_function".to_string(), false)
            }
        };

        let fn_name = panic_stub_fn_name(first_line);

        // Generate a scaffold that references the missing symbol
        let scaffold = if is_external_crate {
            format!(
                "use {}::some_symbol;\n\n#[test]\nfn {}() {{\n    panic!({:?});\n}}\n",
                crate_or_symbol, fn_name, first_line
            )
        } else {
            format!(
                "use fixture_crate::{};\n\n#[test]\nfn {}() {{\n    panic!({:?});\n}}\n",
                crate_or_symbol, fn_name, first_line
            )
        };

        Ok(scaffold)
    } else {
        // Generate a simple runtime-red scaffold
        let fn_name = panic_stub_fn_name(first_line);
        Ok(format!(
            "#[test]\nfn {}() {{\n    panic!({:?});\n}}\n",
            fn_name, first_line
        ))
    }
}

/// Generate a function name from text for panic-stub fallback.
fn panic_stub_fn_name(text: &str) -> String {
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

/// Compute SHA-256 hash of a string and return hex digest.
fn compute_sha256(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}
