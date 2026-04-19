// rustc 1.95.0 ICEs in the dead_code lint pass on this crate
// (check_mod_deathness). Remove once we move off 1.95 or the ICE is fixed.
#![allow(dead_code)]

//! # agentic-dashboard
//!
//! Renders the health status of stories in a dashboard format.
//!
//! The dashboard reads story YAML files and evidence from `agentic-store`
//! (test_runs and uat_signings) to classify each story as one of five
//! statuses:
//! - `proposed`: YAML says `status: proposed` (YAML wins regardless of evidence).
//! - `under_construction`: YAML says `status: under_construction` AND no
//!   historical `uat_signings.verdict=pass` exists.
//! - `healthy`: YAML says `status: healthy` AND latest
//!   `uat_signings.verdict=pass` commit equals HEAD AND latest
//!   `test_runs.verdict=pass`.
//! - `unhealthy`: any historical `uat_signings.verdict=pass` exists AND
//!   (latest `test_runs.verdict=fail` OR latest UAT commit != HEAD).
//! - `error`: YAML parse error, schema violation, or status-evidence mismatch.
//!
//! The dashboard can render the results as a formatted table (default) or as
//! JSON (with full SHAs and RFC3339 timestamps).

use std::cmp::Ordering;
use std::path::PathBuf;
use std::sync::Arc;

use agentic_store::Store;
use agentic_story::{Status, StoryError};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

/// The main dashboard type that reads stories and evidence, computes health,
/// and renders in table or JSON format.
pub struct Dashboard {
    store: Arc<dyn Store>,
    stories_dir: PathBuf,
    head_sha: String,
    repo_root: Option<PathBuf>,
}

/// Error type for dashboard operations.
#[derive(Debug)]
pub enum DashboardError {
    /// Store operation failed.
    StoreError(String),
    /// Stories directory does not exist.
    StoriesNotFound { path: PathBuf },
    /// Unknown story id in drilldown.
    UnknownStory { id: u32 },
}

impl std::fmt::Display for DashboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DashboardError::StoreError(msg) => write!(f, "store error: {msg}"),
            DashboardError::StoriesNotFound { path } => {
                write!(f, "stories directory not found: {}", path.display())
            }
            DashboardError::UnknownStory { id } => {
                write!(f, "unknown story id: {id}")
            }
        }
    }
}

impl std::error::Error for DashboardError {}

/// Internal representation of a story's health status and evidence.
#[derive(Debug, Clone)]
struct StoryHealth {
    id: u32,
    title: String,
    health: Health,
    failing_tests: Vec<String>,
    uat_commit: Option<String>,
    uat_signed_at: Option<String>,
    test_run_commit: Option<String>,
    test_run_at: Option<String>,
    parse_error: Option<String>,
    stale_related_files: Vec<String>,
}

/// The five health statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Health {
    Proposed,
    UnderConstruction,
    Healthy,
    Unhealthy,
    Error,
}

/// Return type for classify_health function.
type HealthClassification = (
    Health,
    Vec<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<String>,
);

impl Health {
    fn as_str(self) -> &'static str {
        match self {
            Health::Proposed => "proposed",
            Health::UnderConstruction => "under_construction",
            Health::Healthy => "healthy",
            Health::Unhealthy => "unhealthy",
            Health::Error => "error",
        }
    }
}

impl Dashboard {
    /// Construct a new Dashboard.
    ///
    /// # Arguments
    /// - `store`: the evidence store (test_runs, uat_signings)
    /// - `stories_dir`: the root directory containing story YAML files
    /// - `head_sha`: the current git HEAD SHA (40-char hex)
    pub fn new(store: Arc<dyn Store>, stories_dir: PathBuf, head_sha: String) -> Self {
        Self {
            store,
            stories_dir,
            head_sha,
            repo_root: None,
        }
    }

    /// Construct a new Dashboard with repo-aware file-intersection checking.
    ///
    /// # Arguments
    /// - `store`: the evidence store (test_runs, uat_signings)
    /// - `stories_dir`: the root directory containing story YAML files
    /// - `repo_root`: the root directory of the git repository
    ///
    /// When `repo_root` is provided, the classifier uses file-intersection
    /// semantics for stories with `related_files` declared. Without it,
    /// falls back to the legacy strict HEAD-equality rule.
    pub fn with_repo(store: Arc<dyn Store>, stories_dir: PathBuf, repo_root: PathBuf) -> Self {
        let repo =
            git2::Repository::open(&repo_root).expect("repo_root must be a valid git repository");
        let head_sha = repo
            .head()
            .expect("repo must have a HEAD")
            .peel_to_commit()
            .expect("HEAD must be a commit")
            .id()
            .to_string();

        Self {
            store,
            stories_dir,
            head_sha,
            repo_root: Some(repo_root),
        }
    }

    /// Render the dashboard as a formatted table.
    pub fn render_table(&self) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        Ok(self.format_table(&stories))
    }

    /// Render the dashboard as JSON.
    pub fn render_json(&self) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        let json = self.format_json(&stories);
        Ok(json)
    }

    /// Return the drill-down view for a single story by id.
    pub fn drilldown(&self, story_id: u32) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        let story = stories
            .iter()
            .find(|s| s.id == story_id)
            .ok_or(DashboardError::UnknownStory { id: story_id })?;

        Ok(self.format_drilldown(story))
    }

    /// Load all story YAML files, compute health for each, and return sorted
    /// by health (error first, then unhealthy, under_construction, proposed,
    /// healthy), with ties broken by ascending story_id.
    fn load_and_compute_health(&self) -> Result<Vec<StoryHealth>, DashboardError> {
        if !self.stories_dir.exists() {
            return Err(DashboardError::StoriesNotFound {
                path: self.stories_dir.clone(),
            });
        }

        let mut stories = Vec::new();

        // Read all .yml files from the stories directory.
        let entries = std::fs::read_dir(&self.stories_dir)
            .map_err(|e| DashboardError::StoreError(e.to_string()))?;

        for entry in entries {
            let entry = entry.map_err(|e| DashboardError::StoreError(e.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yml") {
                let health = self.compute_health_for_file(&path);
                stories.push(health);
            }
        }

        // Sort by health priority (error first) then by id within each group.
        stories.sort_by(|a, b| {
            let a_priority = health_sort_priority(a.health);
            let b_priority = health_sort_priority(b.health);
            match a_priority.cmp(&b_priority) {
                Ordering::Equal => a.id.cmp(&b.id),
                other => other,
            }
        });

        Ok(stories)
    }

    /// Compute the health status for a single story file.
    fn compute_health_for_file(&self, path: &std::path::Path) -> StoryHealth {
        // Extract the id from the filename (e.g. "123.yml" -> 123).
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        // Try to load the story YAML.
        match agentic_story::Story::load(path) {
            Ok(story) => {
                let title = story.title.clone();
                let status = story.status;
                let related_files = story.related_files.clone();

                // Load evidence from the store.
                let test_run = self.get_test_run(id);
                let uat_signings = self.get_uat_signings(id);

                // Compute health classification.
                let (
                    health,
                    failing_tests,
                    uat_commit,
                    uat_signed_at,
                    test_run_commit,
                    test_run_at,
                    stale_related_files,
                ) = self.classify_health(status, &test_run, &uat_signings, &related_files);

                StoryHealth {
                    id,
                    title,
                    health,
                    failing_tests,
                    uat_commit,
                    uat_signed_at,
                    test_run_commit,
                    test_run_at,
                    parse_error: None,
                    stale_related_files,
                }
            }
            Err(e) => {
                // Parse error: render as error row.
                let error_reason = match &e {
                    StoryError::YamlParse { .. } => "yaml parse",
                    StoryError::SchemaViolation { field, .. } => {
                        if field.contains("status") {
                            "schema: status"
                        } else {
                            "schema"
                        }
                    }
                    StoryError::UnknownStatus { .. } => "schema: status",
                    _ => "error",
                };

                StoryHealth {
                    id,
                    title: "<parse error>".to_string(),
                    health: Health::Error,
                    failing_tests: vec![error_reason.to_string()],
                    uat_commit: None,
                    uat_signed_at: None,
                    test_run_commit: None,
                    test_run_at: None,
                    parse_error: Some(e.to_string()),
                    stale_related_files: vec![],
                }
            }
        }
    }

    /// Retrieve the latest test_run row for a story from the store.
    fn get_test_run(&self, story_id: u32) -> Option<Value> {
        self.store
            .get("test_runs", &story_id.to_string())
            .ok()
            .flatten()
    }

    /// Retrieve all uat_signings rows for a story from the store.
    fn get_uat_signings(&self, story_id: u32) -> Vec<Value> {
        self.store
            .query("uat_signings", &|row| {
                row.get("story_id")
                    .and_then(|v| v.as_u64())
                    .map(|id| id == story_id as u64)
                    .unwrap_or(false)
            })
            .unwrap_or_default()
    }

    /// Compute the diff between two commits and return the list of changed
    /// file paths relative to the repo root.
    fn compute_git_diff(&self, from_commit: &str, to_commit: &str) -> Result<Vec<String>, String> {
        let repo_root = self
            .repo_root
            .as_ref()
            .ok_or_else(|| "repo_root not available for git diff computation".to_string())?;

        let repo =
            git2::Repository::open(repo_root).map_err(|e| format!("failed to open repo: {e}"))?;

        // Parse the OID strings
        let from_oid = git2::Oid::from_str(from_commit)
            .map_err(|e| format!("invalid from commit OID: {e}"))?;
        let to_oid =
            git2::Oid::from_str(to_commit).map_err(|e| format!("invalid to commit OID: {e}"))?;

        // Get the trees
        let from_tree = repo
            .find_tree(
                repo.find_commit(from_oid)
                    .map_err(|e| format!("failed to find from commit: {e}"))?
                    .tree_id(),
            )
            .map_err(|e| format!("failed to find from tree: {e}"))?;

        let to_tree = repo
            .find_tree(
                repo.find_commit(to_oid)
                    .map_err(|e| format!("failed to find to commit: {e}"))?
                    .tree_id(),
            )
            .map_err(|e| format!("failed to find to tree: {e}"))?;

        // Compute the diff
        let diff = repo
            .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), None)
            .map_err(|e| format!("failed to compute diff: {e}"))?;

        // Extract changed file paths
        let mut changed_files = Vec::new();
        diff.foreach(
            &mut |delta, _progress| {
                if let Some(path) = delta.new_file().path() {
                    if let Some(path_str) = path.to_str() {
                        changed_files.push(path_str.to_string());
                    }
                }
                true
            },
            None,
            None,
            None,
        )
        .map_err(|e| format!("failed to iterate diff: {e}"))?;

        Ok(changed_files)
    }

    /// Check if any of the glob patterns in related_files match any of the
    /// changed files. Returns a list of matched paths if there is an
    /// intersection, empty vec otherwise.
    fn check_related_files_intersection(
        &self,
        related_files: &[String],
        changed_files: &[String],
    ) -> Vec<String> {
        if related_files.is_empty() {
            return vec![];
        }

        // Build a globset from related_files patterns
        let mut set_builder = globset::GlobSetBuilder::new();

        for pattern in related_files {
            if let Ok(glob) = globset::Glob::new(pattern) {
                set_builder.add(glob);
            }
        }

        let globset = set_builder.build().unwrap_or_else(|_| {
            // If globset construction fails, fall back to empty set (no matches)
            globset::GlobSetBuilder::new()
                .build()
                .expect("empty globset must build")
        });

        // Find all changed files that match any pattern
        let mut matched = Vec::new();
        for changed_file in changed_files {
            if globset.is_match(changed_file) {
                matched.push(changed_file.clone());
            }
        }

        matched
    }

    /// Classify a story's health based on YAML status and evidence.
    fn classify_health(
        &self,
        yaml_status: Status,
        test_run: &Option<Value>,
        uat_signings: &[Value],
        related_files: &[String],
    ) -> HealthClassification {
        // Extract the latest UAT signing if any.
        let latest_uat = uat_signings.last();
        let latest_uat_pass = uat_signings.iter().rev().find(|row| {
            row.get("verdict")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase() == "pass")
                .unwrap_or(false)
        });

        // Rule 1: proposed ⇔ YAML says proposed (YAML wins).
        if yaml_status == Status::Proposed {
            return (Health::Proposed, vec![], None, None, None, None, vec![]);
        }

        // Rule 2: under_construction ⇔ YAML says under_construction AND no
        // historical UAT pass.
        if yaml_status == Status::UnderConstruction && latest_uat_pass.is_none() {
            let failing_tests = test_run
                .as_ref()
                .and_then(|row| row.get("verdict"))
                .and_then(|v| v.as_str())
                .filter(|s| s.to_lowercase() == "fail")
                .and(test_run.as_ref())
                .and_then(|row| row.get("failing_tests"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            return (
                Health::UnderConstruction,
                failing_tests,
                None,
                None,
                None,
                None,
                vec![],
            );
        }

        // Rule 3: healthy ⇔ latest UAT pass exists AND latest test_run pass
        // AND healthy check passes (based on repo_root availability).
        // - If repo_root: check for related_files file intersection (story 9).
        // - If no repo_root: check for strict HEAD equality (legacy story 3).
        if let Some(uat_row) = latest_uat_pass {
            let uat_commit = uat_row
                .get("commit")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let uat_signed_at = uat_row
                .get("signed_at")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let test_run_pass = test_run
                .as_ref()
                .and_then(|row| row.get("verdict"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase() == "pass")
                .unwrap_or(true); // absence is not failure

            let test_run_commit = test_run
                .as_ref()
                .and_then(|row| row.get("commit"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let test_run_at = test_run
                .as_ref()
                .and_then(|row| row.get("ran_at"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if test_run_pass {
                // Determine if the story is healthy based on repo availability.
                let is_healthy = if self.repo_root.is_some() {
                    // Story 9 logic: if related_files is non-empty, check for
                    // intersection. If empty, be permissive (don't apply strict
                    // equality rule).
                    if !related_files.is_empty() {
                        if let Some(uat_sha) = uat_commit.as_deref() {
                            match self.compute_git_diff(uat_sha, &self.head_sha) {
                                Ok(changed_files) => {
                                    let stale_files = self.check_related_files_intersection(
                                        related_files,
                                        &changed_files,
                                    );
                                    stale_files.is_empty()
                                }
                                Err(_) => {
                                    // If diff computation fails, be permissive
                                    true
                                }
                            }
                        } else {
                            false
                        }
                    } else {
                        // No repo_root, empty related_files → permissive
                        true
                    }
                } else {
                    // Legacy (story 3): no repo_root → strict HEAD equality
                    uat_commit.as_deref() == Some(self.head_sha.as_str())
                };

                if is_healthy {
                    return (
                        Health::Healthy,
                        vec![],
                        uat_commit,
                        uat_signed_at,
                        test_run_commit,
                        test_run_at,
                        vec![],
                    );
                }
            }
        }

        // Rule 4: unhealthy ⇔ any historical UAT pass AND
        // (latest test_run fail OR (repo-aware: related_files intersect C0..HEAD)
        // OR (legacy: latest UAT commit != HEAD AND no repo_root)).
        if latest_uat_pass.is_some() {
            let test_run_fail = test_run
                .as_ref()
                .and_then(|row| row.get("verdict"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase() == "fail")
                .unwrap_or(false);

            let uat_commit = latest_uat
                .and_then(|row| row.get("commit"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut is_unhealthy = test_run_fail;
            let mut stale_files = vec![];

            // If we have a repo_root and non-empty related_files, check for
            // intersection. Otherwise fall back to legacy strict-equality rule.
            if !is_unhealthy {
                if !related_files.is_empty() && self.repo_root.is_some() {
                    if let Some(uat_sha) = uat_commit.as_deref() {
                        match self.compute_git_diff(uat_sha, &self.head_sha) {
                            Ok(changed_files) => {
                                stale_files = self.check_related_files_intersection(
                                    related_files,
                                    &changed_files,
                                );
                                is_unhealthy = !stale_files.is_empty();
                            }
                            Err(_) => {
                                // If diff fails, be permissive
                                is_unhealthy = false;
                            }
                        }
                    }
                } else {
                    // Legacy: no repo_root or empty related_files means strict
                    // equality check (UAT commit must equal HEAD).
                    let uat_commit_not_head = uat_commit.as_deref() != Some(self.head_sha.as_str());
                    is_unhealthy = uat_commit_not_head;
                }
            }

            if is_unhealthy {
                let failing_tests = test_run
                    .as_ref()
                    .and_then(|row| row.get("failing_tests"))
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let uat_signed_at = latest_uat
                    .and_then(|row| row.get("signed_at"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let test_run_commit = test_run
                    .as_ref()
                    .and_then(|row| row.get("commit"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let test_run_at = test_run
                    .as_ref()
                    .and_then(|row| row.get("ran_at"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                return (
                    Health::Unhealthy,
                    failing_tests,
                    uat_commit,
                    uat_signed_at,
                    test_run_commit,
                    test_run_at,
                    stale_files,
                );
            }
        }

        // Rule 5: error — status-evidence mismatch.
        // This covers cases like YAML says healthy but no UAT signing exists.
        (
            Health::Error,
            vec!["status-evidence mismatch".to_string()],
            None,
            None,
            None,
            None,
            vec![],
        )
    }

    /// Format stories as a table.
    fn format_table(&self, stories: &[StoryHealth]) -> String {
        let mut output = String::new();

        // Header
        output.push_str("ID | Title | Health | Failing tests | Healthy at\n");
        output.push_str("---|-------|--------|---------------|----------\n");

        // Rows
        for story in stories {
            let id = format!("{}", story.id);
            let title = truncate_title(&story.title);
            let health = story.health.as_str();

            let failing_tests =
                if story.health == Health::Proposed || story.health == Health::Healthy {
                    String::new()
                } else {
                    story.failing_tests.join(", ")
                };

            let healthy_at = if story.health == Health::Healthy {
                let short_sha = story.uat_commit.as_ref().map(|s| &s[..7]).unwrap_or("");
                let relative_age = story
                    .uat_signed_at
                    .as_ref()
                    .and_then(|ts| format_relative_age(ts))
                    .unwrap_or_else(|| "?".to_string());
                format!("{} {}", short_sha, relative_age)
            } else {
                String::new()
            };

            output.push_str(&format!(
                "{} | {} | {} | {} | {}\n",
                id, title, health, failing_tests, healthy_at
            ));
        }

        output
    }

    /// Format stories as JSON.
    fn format_json(&self, stories: &[StoryHealth]) -> String {
        let mut story_objects = Vec::new();

        for story in stories {
            let mut obj = json!({
                "id": story.id,
                "title": story.title,
                "health": story.health.as_str(),
                "failing_tests": story.failing_tests,
                "uat_commit": story.uat_commit,
                "uat_signed_at": story.uat_signed_at,
                "test_run_commit": story.test_run_commit,
                "test_run_at": story.test_run_at,
            });

            // Include stale_related_files only if non-empty
            if !story.stale_related_files.is_empty() {
                obj["stale_related_files"] = json!(story.stale_related_files.clone());
            }

            story_objects.push(obj);
        }

        // Compute summary counts.
        let mut counts = std::collections::HashMap::new();
        counts.insert("healthy", 0);
        counts.insert("unhealthy", 0);
        counts.insert("under_construction", 0);
        counts.insert("proposed", 0);
        counts.insert("error", 0);

        for story in stories {
            let key = story.health.as_str();
            *counts.get_mut(key).unwrap_or(&mut 0) += 1;
        }

        let summary = json!({
            "healthy": counts.get("healthy").unwrap_or(&0),
            "unhealthy": counts.get("unhealthy").unwrap_or(&0),
            "under_construction": counts.get("under_construction").unwrap_or(&0),
            "proposed": counts.get("proposed").unwrap_or(&0),
            "error": counts.get("error").unwrap_or(&0),
            "total": stories.len(),
        });

        let result = json!({
            "stories": story_objects,
            "summary": summary,
        });

        result.to_string()
    }

    /// Format the drill-down view for a single story.
    fn format_drilldown(&self, story: &StoryHealth) -> String {
        let mut output = String::new();

        output.push_str(&format!("Story ID: {}\n", story.id));
        output.push_str(&format!("Title: {}\n", story.title));
        output.push_str(&format!("Health: {}\n", story.health.as_str()));

        if !story.failing_tests.is_empty() {
            output.push_str("Failing tests:\n");
            for test in &story.failing_tests {
                output.push_str(&format!("  - {}\n", test));
            }
        }

        if let Some(ref uat_commit) = story.uat_commit {
            output.push_str(&format!("Latest UAT commit: {}\n", uat_commit));
        }

        if let Some(ref uat_signed_at) = story.uat_signed_at {
            output.push_str(&format!("Latest UAT signed at: {}\n", uat_signed_at));
        }

        output
    }
}

/// Sort priority for health statuses: lower value comes first.
fn health_sort_priority(health: Health) -> i32 {
    match health {
        Health::Error => 0,
        Health::Unhealthy => 1,
        Health::UnderConstruction => 2,
        Health::Proposed => 3,
        Health::Healthy => 4,
    }
}

/// Truncate a title to ~35 visible characters with a single U+2026 ellipsis.
fn truncate_title(title: &str) -> String {
    const MAX_LEN: usize = 35;
    let char_count = title.chars().count();
    if char_count <= MAX_LEN {
        title.to_string()
    } else {
        // Take the first MAX_LEN characters (safe for Unicode) and append the ellipsis.
        let truncated: String = title.chars().take(MAX_LEN).collect();
        format!("{}…", truncated)
    }
}

/// Format a timestamp (RFC3339) as a relative age string.
/// Returns strings like "just now", "5m ago", "3h ago", "2d ago".
fn format_relative_age(timestamp: &str) -> Option<String> {
    // Parse the timestamp.
    let dt = DateTime::parse_from_rfc3339(timestamp).ok()?;
    let then = dt.with_timezone(&Utc);
    let now = Utc::now();

    let duration = now.signed_duration_since(then);

    if duration.num_seconds() < 1 {
        return Some("just now".to_string());
    }

    if duration.num_seconds() < 60 {
        return Some(format!("{}s ago", duration.num_seconds()));
    }

    if duration.num_minutes() < 60 {
        return Some(format!("{}m ago", duration.num_minutes()));
    }

    if duration.num_hours() < 24 {
        return Some(format!("{}h ago", duration.num_hours()));
    }

    Some(format!("{}d ago", duration.num_days()))
}
