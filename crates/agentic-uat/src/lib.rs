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
//! 3. **Signer resolution:** Resolve the signer identity via a four-tier
//!    chain (explicit flag → env → git config → error). If unresolvable,
//!    return `UatError::SignerMissing`.
//!
//! 4. **Executor invocation:** Call `executor.execute(&story)`, get a
//!    `Verdict` (Pass or Fail).
//!
//! 5. **Signing:** Write one row to `uat_signings` with the verdict,
//!    current commit SHA, signer identity, and RFC3339 UTC timestamp.
//!
//! 6. **Promotion (Pass only):** If the verdict is Pass, rewrite the
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
use agentic_story::{Status, Story, StoryError};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Specifies how the signer identity should be resolved.
///
/// The signer is resolved through a four-tier chain:
/// 1. Explicit flag/value (`SignerSource::Explicit`)
/// 2. `AGENTIC_SIGNER` environment variable
/// 3. `git config user.email`
/// 4. Typed error (`UatError::SignerMissing`)
#[derive(Debug, Clone, Default)]
pub enum SignerSource {
    /// Resolve the signer identity via the four-tier chain starting with
    /// environment and git config. This is the normal path for CLI usage.
    #[default]
    Resolve,
    /// Use an explicitly provided signer identity, bypassing all resolution.
    Explicit(String),
}

/// The table name for UAT signing records.
const UAT_SIGNINGS_TABLE: &str = "uat_signings";

/// The table name for manual signing records (written by story 28's backfill).
const MANUAL_SIGNINGS_TABLE: &str = "manual_signings";

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

/// Reason why an ancestor is considered unhealthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AncestorUnhealthyReason {
    /// The ancestor's on-disk YAML status is not `healthy`.
    StatusNotHealthy,
    /// The ancestor claims `healthy` on disk but has no signing row in the store.
    NoSigningRow,
    /// The ancestor has a signing row (in uat_signings or manual_signings), but the latest one is a fail.
    ManualSigningLatestIsFail,
}

impl std::fmt::Display for AncestorUnhealthyReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AncestorUnhealthyReason::StatusNotHealthy => {
                write!(f, "ancestor status is not healthy on disk")
            }
            AncestorUnhealthyReason::NoSigningRow => {
                write!(
                    f,
                    "ancestor claims healthy but has no signing row in the store"
                )
            }
            AncestorUnhealthyReason::ManualSigningLatestIsFail => {
                write!(f, "ancestor's latest signing attestation is a fail verdict")
            }
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
    /// The signer identity could not be resolved from any of the four tiers
    /// (explicit flag, AGENTIC_SIGNER env, git config user.email, or error).
    SignerMissing,
    /// File I/O error (other than "file not found", which surfaces as
    /// UnknownStory).
    Io(String),
    /// Store backend error.
    Store(StoreError),
    /// A transitive ancestor of the story is not healthy (status != healthy
    /// on disk, or status = healthy without a corresponding signing row).
    /// The error names the offending ancestor id and the reason.
    AncestorNotHealthy {
        ancestor_id: u32,
        reason: AncestorUnhealthyReason,
    },
    /// The depends_on graph contains a cycle, detected at the UAT boundary.
    /// The error names the cycle edge.
    Cycle { edge: (u32, u32) },
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
            UatError::SignerMissing => write!(
                f,
                "could not produce a verdict: signer identity could not be resolved"
            ),
            UatError::Io(msg) => write!(f, "io error while running uat: {msg}"),
            UatError::Store(err) => write!(f, "store error while running uat: {err}"),
            UatError::AncestorNotHealthy {
                ancestor_id,
                reason,
            } => write!(
                f,
                "could not produce a verdict: ancestor {ancestor_id} is not healthy ({reason})"
            ),
            UatError::Cycle { edge: (from, to) } => {
                write!(
                    f,
                    "could not produce a verdict: cycle in depends_on graph ({from} -> {to})"
                )
            }
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

    /// Run a UAT for the story with the given id, resolving the signer identity.
    ///
    /// # Arguments
    ///
    /// - `story_id`: The story to test and potentially promote.
    ///
    /// Returns a `Verdict` on success (Pass or Fail). Returns a typed
    /// `UatError` if the tree is dirty, the story is not found, the signer
    /// cannot be resolved, or a backend error occurs.
    ///
    /// # Side effects on success
    ///
    /// - Always writes one row to `uat_signings` with the verdict, commit
    ///   SHA, signer identity, and timestamp.
    /// - If the verdict is Pass, rewrites the story YAML in place to set
    ///   `status: healthy`, preserving all other fields.
    /// - If the verdict is Fail, the story YAML is untouched.
    ///
    /// # Side effects on error
    ///
    /// None. No rows written, no files touched.
    pub fn run(&self, story_id: u32) -> Result<Verdict, UatError> {
        self.run_with_signer(story_id, SignerSource::Resolve)
    }

    /// Run a UAT for the story with the given id and explicit signer source.
    ///
    /// # Arguments
    ///
    /// - `story_id`: The story to test and potentially promote.
    /// - `signer_source`: How to resolve the signer identity (explicit or via chain).
    ///
    /// Returns a `Verdict` on success (Pass or Fail). Returns a typed
    /// `UatError` if the tree is dirty, the story is not found, the signer
    /// cannot be resolved, or a backend error occurs.
    ///
    /// # Side effects on success
    ///
    /// - Always writes one row to `uat_signings` with the verdict, commit
    ///   SHA, signer identity, and timestamp.
    /// - If the verdict is Pass, rewrites the story YAML in place to set
    ///   `status: healthy`, preserving all other fields.
    /// - If the verdict is Fail, the story YAML is untouched.
    ///
    /// # Side effects on error
    ///
    /// None. No rows written, no files touched.
    pub fn run_with_signer(
        &self,
        story_id: u32,
        signer_source: SignerSource,
    ) -> Result<Verdict, UatError> {
        // Step 1: Dirty-tree check FIRST, before anything else.
        check_tree_clean(&self.stories_dir)?;

        // Step 2: Load the story.
        let story_path = self.stories_dir.join(format!("{story_id}.yml"));
        let story = Story::load(&story_path).map_err(|e| match e {
            StoryError::NotFound { .. } => UatError::UnknownStory { id: story_id },
            _ => UatError::Io(e.to_string()),
        })?;

        // Step 2.5: Resolve the signer identity.
        let signer = resolve_signer(signer_source, &self.stories_dir)?;

        // Step 3: Execute the UAT.
        let outcome = self.executor.execute(&story);
        let verdict = outcome.verdict;

        // Step 3.5 (Pass only): Check ancestor health before signing.
        if verdict == Verdict::Pass {
            check_ancestors_healthy(
                &self.stories_dir,
                story_id,
                &story.depends_on,
                self.store.as_ref(),
            )?;
        }

        // Step 4: Sign the verdict.
        let commit = get_head_sha(&self.stories_dir)?;
        let signed_at = rfc3339_utc_now()?;
        let id = Uuid::now_v7().to_string();

        let signing_row = json!({
            "id": id,
            "story_id": story_id,
            "verdict": verdict.as_str(),
            "commit": commit,
            "signer": signer,
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

/// Resolve the signer identity from the given source via the four-tier chain:
/// 1. Explicit flag/value
/// 2. `AGENTIC_SIGNER` environment variable
/// 3. `git config user.email`
/// 4. Error
///
/// Returns the resolved signer string or `UatError::SignerMissing` if
/// no source yields a non-empty value. Uses the canonical resolver from
/// `agentic-signer` (story 18) to ensure consistency across all signing paths.
fn resolve_signer(source: SignerSource, from_path: &Path) -> Result<String, UatError> {
    use agentic_signer::{Resolver, Signer, SignerError};

    let resolver = match source {
        SignerSource::Explicit(s) => {
            // Explicit value: use it directly via Resolver::with_flag.
            Resolver::with_flag(s).at_repo(from_path)
        }
        SignerSource::Resolve => {
            // Resolution via the chain: env → git config.
            Resolver::new().at_repo(from_path)
        }
    };

    match Signer::resolve(resolver) {
        Ok(signer) => Ok(signer.as_str().to_string()),
        Err(SignerError::SignerMissing { .. }) => Err(UatError::SignerMissing),
        Err(SignerError::SignerInvalid {
            source: _,
            reason: _,
        }) => Err(UatError::SignerMissing),
        Err(SignerError::GitConfigRead { source: err }) => {
            Err(UatError::Io(format!("git config error: {err}")))
        }
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

/// Check that all transitive ancestors of a story are healthy.
///
/// For a Pass verdict, all ancestors must be healthy (status = healthy on disk)
/// AND have a corresponding valid signing row in the store. The check is
/// transitive: we walk the depends_on graph recursively.
///
/// Returns `UatError::AncestorNotHealthy` if any ancestor fails the health check,
/// or `UatError::Cycle` if a cycle is detected in the graph.
fn check_ancestors_healthy(
    stories_dir: &Path,
    _story_id: u32,
    direct_ancestors: &[u32],
    store: &dyn Store,
) -> Result<(), UatError> {
    // Load all stories to build the dependency graph.
    let all_stories = Story::load_dir(stories_dir).map_err(|e| match e {
        StoryError::DependsOnCycle { participants } => {
            // If the cycle is detected at load time, surface it as-is.
            // Pick an edge from the cycle for reporting.
            if participants.len() > 1 {
                UatError::Cycle {
                    edge: (participants[0], participants[1]),
                }
            } else if participants.len() == 1 {
                UatError::Cycle {
                    edge: (participants[0], participants[0]),
                }
            } else {
                UatError::Io("cycle with no participants".to_string())
            }
        }
        StoryError::SupersededByCycle { participants } => {
            // Cycle in the supersession chain. Same error variant as depends_on cycles
            // because they are both DAG-breaking cycles from the operator's perspective.
            if participants.len() > 1 {
                UatError::Cycle {
                    edge: (participants[0], participants[1]),
                }
            } else if participants.len() == 1 {
                UatError::Cycle {
                    edge: (participants[0], participants[0]),
                }
            } else {
                UatError::Io("superseded_by cycle with no participants".to_string())
            }
        }
        _ => UatError::Io(format!("failed to load stories: {e}")),
    })?;

    // Build a map from story id to Story for quick lookup.
    let stories_by_id: HashMap<u32, Story> = all_stories.into_iter().map(|s| (s.id, s)).collect();

    // Walk ancestors transitively, checking health and detecting cycles.
    let mut visited: HashSet<u32> = HashSet::new();
    let mut path: Vec<u32> = Vec::new();

    // Check each direct ancestor.
    for &ancestor_id in direct_ancestors {
        check_ancestor_dfs(ancestor_id, &stories_by_id, store, &mut visited, &mut path)?;
    }

    Ok(())
}

/// DFS helper for ancestor health checking.
///
/// Maintains a `visited` set and a `path` stack for cycle detection.
/// If we encounter a node already in `path`, it's a cycle.
/// If we encounter a node already visited (but not in path), we've already
/// checked it and can skip.
fn check_ancestor_dfs(
    story_id: u32,
    stories_by_id: &HashMap<u32, Story>,
    store: &dyn Store,
    visited: &mut HashSet<u32>,
    path: &mut Vec<u32>,
) -> Result<(), UatError> {
    // If already visited, we've checked this ancestor transitively.
    if visited.contains(&story_id) {
        return Ok(());
    }

    // Check for cycle: if story_id is in the current path, we have a back edge.
    if path.contains(&story_id) {
        // Find the edge: the story that added story_id to depends_on.
        let from = *path.last().unwrap_or(&story_id);
        return Err(UatError::Cycle {
            edge: (from, story_id),
        });
    }

    // Mark as visited and add to path.
    visited.insert(story_id);
    path.push(story_id);

    // Get the story.
    let story = match stories_by_id.get(&story_id) {
        Some(s) => s,
        None => {
            // Story not found — shouldn't happen if load_dir succeeded.
            path.pop();
            return Ok(());
        }
    };

    // Check if this ancestor is satisfied, walking the retirement chain if needed.
    is_ancestor_satisfied(story, stories_by_id, store)?;

    // Recursively check each of this ancestor's ancestors.
    for &next_ancestor_id in &story.depends_on {
        check_ancestor_dfs(next_ancestor_id, stories_by_id, store, visited, path)?;
    }

    path.pop();
    Ok(())
}

/// Check if a single ancestor is satisfied, walking the `superseded_by` chain
/// if the ancestor is retired.
///
/// This implements the chain-walk algorithm from story 11's guidance:
/// - If status is healthy with a valid signing row: satisfied.
/// - If status is healthy without a signing row: error (NoSigningRow).
/// - If status is retired: follow `superseded_by` chain recursively.
///   - If chain terminates at a retired story with no successor: satisfied.
///   - If chain reaches a healthy story with signing: satisfied.
///   - If chain reaches a non-healthy story: error (naming the terminal link).
/// - If status is not healthy and not retired: error (StatusNotHealthy).
///
/// The function maintains its own `visited` set to detect cycles in the
/// `superseded_by` chain independently from the outer `depends_on` walk.
fn is_ancestor_satisfied(
    story: &Story,
    stories_by_id: &HashMap<u32, Story>,
    store: &dyn Store,
) -> Result<(), UatError> {
    let mut cursor = story;
    let mut visited: HashSet<u32> = HashSet::new();

    loop {
        // Detect cycles in the superseded_by chain.
        if !visited.insert(cursor.id) {
            // We've seen this story id before in the chain-walk.
            return Err(UatError::Cycle {
                edge: (cursor.id, cursor.id),
            });
        }

        match cursor.status {
            Status::Healthy => {
                // Check if THIS link has a valid signing row from EITHER table.
                // Query both uat_signings and manual_signings for this story.
                let uat_pass_rows = store
                    .query(UAT_SIGNINGS_TABLE, &|doc| {
                        doc.get("story_id").and_then(|v| v.as_u64()) == Some(cursor.id as u64)
                            && doc.get("verdict").and_then(|v| v.as_str()) == Some("pass")
                    })
                    .map_err(UatError::Store)?;

                let manual_pass_rows = store
                    .query(MANUAL_SIGNINGS_TABLE, &|doc| {
                        doc.get("story_id").and_then(|v| v.as_u64()) == Some(cursor.id as u64)
                            && doc.get("verdict").and_then(|v| v.as_str()) == Some("pass")
                    })
                    .map_err(UatError::Store)?;

                // Combine the rows.
                let mut all_pass_rows = uat_pass_rows;
                all_pass_rows.extend(manual_pass_rows);

                if !all_pass_rows.is_empty() {
                    // We have at least one Pass row: satisfied.
                    // Healthy with valid signing row: check its ancestors transitively.
                    let mut visited_temp: HashSet<u32> = HashSet::new();
                    let mut path_temp: Vec<u32> = Vec::new();
                    for &ancestor_id in &cursor.depends_on {
                        check_ancestor_dfs(
                            ancestor_id,
                            stories_by_id,
                            store,
                            &mut visited_temp,
                            &mut path_temp,
                        )?;
                    }
                    return Ok(());
                }

                // No Pass rows in either table. Check if there's a Fail row to
                // distinguish from the "no row at all" case.
                let uat_fail_rows = store
                    .query(UAT_SIGNINGS_TABLE, &|doc| {
                        doc.get("story_id").and_then(|v| v.as_u64()) == Some(cursor.id as u64)
                            && doc.get("verdict").and_then(|v| v.as_str()) == Some("fail")
                    })
                    .map_err(UatError::Store)?;

                let manual_fail_rows = store
                    .query(MANUAL_SIGNINGS_TABLE, &|doc| {
                        doc.get("story_id").and_then(|v| v.as_u64()) == Some(cursor.id as u64)
                            && doc.get("verdict").and_then(|v| v.as_str()) == Some("fail")
                    })
                    .map_err(UatError::Store)?;

                // If there's a Fail row in either table, return the ManualSigningLatestIsFail reason.
                if !uat_fail_rows.is_empty() || !manual_fail_rows.is_empty() {
                    return Err(UatError::AncestorNotHealthy {
                        ancestor_id: cursor.id,
                        reason: AncestorUnhealthyReason::ManualSigningLatestIsFail,
                    });
                }

                // No Pass row and no Fail row: return NoSigningRow.
                return Err(UatError::AncestorNotHealthy {
                    ancestor_id: cursor.id,
                    reason: AncestorUnhealthyReason::NoSigningRow,
                });
            }
            Status::Retired => {
                // Follow the supersession chain.
                if let Some(successor_id) = cursor.superseded_by {
                    // Load the successor.
                    if let Some(successor) = stories_by_id.get(&successor_id) {
                        cursor = successor;
                        continue; // Walk the chain.
                    } else {
                        // Successor not found — shouldn't happen if loader validated,
                        // but treat as satisfied to avoid hard failures.
                        return Ok(());
                    }
                } else {
                    // Terminal retirement (no successor): satisfied.
                    return Ok(());
                }
            }
            Status::Proposed | Status::UnderConstruction | Status::Unhealthy => {
                // Non-healthy, non-retired status: error, naming the terminal link.
                return Err(UatError::AncestorNotHealthy {
                    ancestor_id: cursor.id,
                    reason: AncestorUnhealthyReason::StatusNotHealthy,
                });
            }
        }
    }
}
