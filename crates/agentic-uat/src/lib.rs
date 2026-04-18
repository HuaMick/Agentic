//! # agentic-uat
//!
//! The UAT (User Acceptance Test) signing and verdict gate for story
//! promotion to `healthy`.
//!
//! This library is the only path by which a story's `status` on disk
//! transitions to `healthy`. It is designed to be standalone-resilient:
//! it depends only on [`agentic_store::Store`], `git2`, `serde_yaml`,
//! `serde_json`, `uuid`, and `agentic_story` — no orchestrator, runtime,
//! sandbox, or CLI. This ensures that even if the rest of the system is
//! offline, a UAT verdict can still be signed and promoted.
//!
//! ## Core flow
//!
//! 1. **Dirty-tree check:** Before anything else, verify the git working
//!    tree is clean (using `git2::Repository::statuses()`). If dirty,
//!    return `UatError::DirtyTree` immediately, write no rows, touch no
//!    files.
//!
//! 2. **Story load:** Load the story YAML from `stories/<id>.yml` via
//!    `agentic_story::Story::load`. If not found, return
//!    `UatError::UnknownStory { id }`, write no rows, touch no files.
//!
//! 3. **Executor invocation:** Call `executor.execute(&story)`, get a
//!    `Verdict` (Pass or Fail).
//!
//! 4. **Signing:** Write one row to `uat_signings` with the verdict,
//!    current commit SHA, and RFC3339 UTC timestamp.
//!
//! 5. **Promotion (Pass only):** If the verdict is Pass, rewrite the
//!    story YAML in place to set `status: healthy`. On Fail, the YAML
//!    is untouched.
//!
//! ## Error handling
//!
//! The dirty-tree and unknown-story checks happen before signing. No
//! side effects on error. Store errors and IO errors are typed.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use agentic_store::{Store, StoreError};
use agentic_story::{Story, StoryError};
use serde_json::json;
use uuid::Uuid;

/// The table name for UAT signing records.
const UAT_SIGNINGS_TABLE: &str = "uat_signings";

/// Typed verdict from a UAT run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The story's UAT passed; it may be promoted to healthy.
    Pass,
    /// The story's UAT failed; it remains unhealthy or under_construction.
    Fail,
}

impl Verdict {
    /// The lowercase string representation of this verdict.
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Fail => "fail",
        }
    }
}

/// The typed outcome of a UAT executor's run against a story.
#[derive(Debug, Clone)]
pub struct ExecutionOutcome {
    /// The verdict: Pass or Fail.
    pub verdict: Verdict,
    /// Free-form transcript of the journey (e.g., steps taken, assertions
    /// checked). The signing row does NOT include this; transcript is
    /// handled separately per the story's guidance.
    pub transcript: String,
}

/// Trait for executing a UAT against a story.
///
/// The library defines the contract; the implementation is pluggable. For
/// story 1's tests, a `StubExecutor` returns a fixed verdict. A real impl
/// might drive a human through a prose journey or sub-agent orchestration.
pub trait UatExecutor: Send + Sync {
    /// Execute a UAT against the given story and return the outcome.
    fn execute(&self, story: &Story) -> ExecutionOutcome;
}

/// A stub executor that always returns Pass.
///
/// Used by tests to exercise the signing and promotion flow without
/// coupling to a real UAT implementation.
#[derive(Debug, Clone)]
pub struct StubExecutor {
    verdict: Verdict,
}

impl StubExecutor {
    /// Create a stub executor that always returns Pass.
    pub fn always_pass() -> Self {
        Self {
            verdict: Verdict::Pass,
        }
    }

    /// Create a stub executor that always returns Fail.
    pub fn always_fail() -> Self {
        Self {
            verdict: Verdict::Fail,
        }
    }
}

impl UatExecutor for StubExecutor {
    fn execute(&self, _story: &Story) -> ExecutionOutcome {
        ExecutionOutcome {
            verdict: self.verdict,
            transcript: String::new(),
        }
    }
}

/// Errors the UAT library can surface.
#[derive(Debug)]
#[non_exhaustive]
pub enum UatError {
    /// The git working tree has uncommitted changes, staged files, or
    /// untracked files that would affect the build. A signed verdict
    /// against an unknowable tree is worse than no verdict.
    DirtyTree,
    /// The story file was not found or could not be loaded. Carries the
    /// offending story id.
    UnknownStory { id: u32 },
    /// File I/O error (other than "file not found", which surfaces as
    /// UnknownStory).
    Io(String),
    /// Store backend error.
    Store(StoreError),
}

impl std::fmt::Display for UatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UatError::DirtyTree => write!(
                f,
                "could not produce a verdict: working tree has uncommitted changes"
            ),
            UatError::UnknownStory { id } => {
                write!(f, "could not produce a verdict: story {id} not found")
            }
            UatError::Io(msg) => write!(f, "io error while running uat: {msg}"),
            UatError::Store(err) => write!(f, "store error while running uat: {err}"),
        }
    }
}

impl std::error::Error for UatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            UatError::Store(err) => Some(err),
            _ => None,
        }
    }
}

impl From<StoreError> for UatError {
    fn from(err: StoreError) -> Self {
        UatError::Store(err)
    }
}

/// The UAT gate that signs verdicts and promotes stories to healthy.
///
/// Construct with `Uat::new(store, executor, stories_dir)`. The gate holds
/// handles to the store and executor, and the path to the stories directory.
/// It is cheap to clone and safe to share.
pub struct Uat<E: UatExecutor> {
    store: Arc<dyn Store>,
    executor: E,
    stories_dir: PathBuf,
}

impl<E: UatExecutor> Uat<E> {
    /// Construct a UAT gate.
    ///
    /// # Arguments
    ///
    /// - `store`: The store to write signing rows to.
    /// - `executor`: The UAT executor (e.g., `StubExecutor`, or a real impl).
    /// - `stories_dir`: Path to the `stories/` directory where YAML files live.
    pub fn new(store: Arc<dyn Store>, executor: E, stories_dir: PathBuf) -> Self {
        Self {
            store,
            executor,
            stories_dir,
        }
    }

    /// Run a UAT for the story with the given id.
    ///
    /// Returns a `Verdict` on success (Pass or Fail). Returns a typed
    /// `UatError` if the tree is dirty, the story is not found, or a
    /// backend error occurs.
    ///
    /// # Side effects on success
    ///
    /// - Always writes one row to `uat_signings` with the verdict, commit
    ///   SHA, and timestamp.
    /// - If the verdict is Pass, rewrites the story YAML in place to set
    ///   `status: healthy`, preserving all other fields.
    /// - If the verdict is Fail, the story YAML is untouched.
    ///
    /// # Side effects on error
    ///
    /// None. No rows written, no files touched.
    pub fn run(&self, story_id: u32) -> Result<Verdict, UatError> {
        // Step 1: Dirty-tree check FIRST, before anything else.
        check_tree_clean(&self.stories_dir)?;

        // Step 2: Load the story.
        let story_path = self.stories_dir.join(format!("{story_id}.yml"));
        let story = Story::load(&story_path).map_err(|e| match e {
            StoryError::NotFound { .. } => UatError::UnknownStory { id: story_id },
            _ => UatError::Io(e.to_string()),
        })?;

        // Step 3: Execute the UAT.
        let outcome = self.executor.execute(&story);
        let verdict = outcome.verdict;

        // Step 4: Sign the verdict.
        let commit = get_head_sha(&self.stories_dir)?;
        let signed_at = rfc3339_utc_now()?;
        let id = Uuid::now_v7().to_string();

        let signing_row = json!({
            "id": id,
            "story_id": story_id,
            "verdict": verdict.as_str(),
            "commit": commit,
            "signed_at": signed_at,
        });

        self.store
            .append(UAT_SIGNINGS_TABLE, signing_row)
            .map_err(UatError::Store)?;

        // Step 5: On Pass, promote the story to healthy.
        if verdict == Verdict::Pass {
            promote_story_to_healthy(&story_path)?;
        }

        Ok(verdict)
    }
}

/// Check that the git working tree is clean (no uncommitted changes,
/// no untracked files that would affect the build).
///
/// Returns `UatError::DirtyTree` if dirty, or another error if the
/// repository cannot be discovered or accessed.
fn check_tree_clean(stories_dir: &Path) -> Result<(), UatError> {
    // Discover the repo from the stories_dir and walk up if needed.
    let repo = git2::Repository::discover(stories_dir).map_err(|_| UatError::DirtyTree)?;

    // Check statuses. Non-ignored changes (modified, staged, untracked)
    // count as dirty.
    let statuses = repo.statuses(None).map_err(|_| UatError::DirtyTree)?;

    for entry in statuses.iter() {
        // Skip ignored files; they do not make the tree "dirty" for signing.
        if !entry.status().contains(git2::Status::IGNORED) {
            return Err(UatError::DirtyTree);
        }
    }

    Ok(())
}

/// Discover the git repository from the given path and return the full
/// 40-char hex SHA of HEAD.
fn get_head_sha(from_path: &Path) -> Result<String, UatError> {
    let repo = git2::Repository::discover(from_path)
        .map_err(|e| UatError::Io(format!("could not discover repo: {e}")))?;

    let head = repo
        .head()
        .map_err(|e| UatError::Io(format!("could not resolve HEAD: {e}")))?;

    let oid = head
        .target()
        .ok_or_else(|| UatError::Io("HEAD is not a direct reference".to_string()))?;

    Ok(oid.to_string())
}

/// Format the current system time as RFC3339 UTC (`YYYY-MM-DDTHH:MM:SSZ`).
///
/// Mirrors the implementation in `agentic-ci-record` so timestamps are
/// consistent across the system.
fn rfc3339_utc_now() -> Result<String, UatError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| UatError::Io(format!("system clock error: {e}")))?;

    let secs = now.as_secs() as i64;
    Ok(format_unix_to_rfc3339(secs))
}

/// Convert seconds-since-UNIX-epoch to `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Uses Howard Hinnant's algorithm (same as `agentic-ci-record`).
fn format_unix_to_rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(86400);
    let secs_of_day = secs.rem_euclid(86400);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;

    // Howard Hinnant's days_from_civil, inverted.
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z",
        year = year,
        month = month,
        day = day,
        hour = hour,
        minute = minute,
        second = second,
    )
}

/// Rewrite a story YAML file to set `status: healthy`, preserving all
/// other fields exactly (key order, comments, structure).
///
/// This uses `serde_yaml` to parse the YAML, locate the `status` field,
/// and write it back with a surgical edit to the `status` value.
fn promote_story_to_healthy(story_path: &Path) -> Result<(), UatError> {
    // Read the current file as a string so we can do a surgical replacement.
    let content = fs::read_to_string(story_path)
        .map_err(|e| UatError::Io(format!("could not read story file: {e}")))?;

    // Parse as YAML to ensure the file is valid and to locate the `status` field.
    let mut doc: serde_yaml::Value = serde_yaml::from_str(&content)
        .map_err(|e| UatError::Io(format!("could not parse story YAML: {e}")))?;

    // Ensure `status` exists and update it.
    if let Some(map) = doc.as_mapping_mut() {
        map.insert(
            serde_yaml::Value::String("status".to_string()),
            serde_yaml::Value::String("healthy".to_string()),
        );
    }

    // Serialize back to YAML with minimal indentation.
    let updated = serde_yaml::to_string(&doc)
        .map_err(|e| UatError::Io(format!("could not serialize story YAML: {e}")))?;

    // Write the updated YAML back.
    fs::write(story_path, updated)
        .map_err(|e| UatError::Io(format!("could not write story file: {e}")))?;

    Ok(())
}
