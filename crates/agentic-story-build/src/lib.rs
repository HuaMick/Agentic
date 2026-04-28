//! # agentic-story-build
//!
//! Host-and-sandbox driver for `agentic story build <id>`. The public
//! surface hosts:
//!
//! - `StoryBuild` — the orchestration type that composes the `docker
//!   run` argv on the host, drives the container-side seeding /
//!   gate-check / inner-loop lifecycle in `--in-sandbox` mode, and
//!   performs the post-run auto-merge on green.
//! - `BuildConfig` — the caller-owned configuration (resolved image
//!   tag, runs root, credentials path, ancestor snapshot path, docker
//!   binary resolver).
//! - `InSandboxConfig` — the container-side configuration read from
//!   env + mounted files.
//! - `StoryBuildError` — the typed failure enum
//!   (`DockerUnavailable`, `GitIdentityMissing`, `StartShaDrift`,
//!   `AncestorSnapshotInsufficient`, `ImageTagNotFound`,
//!   `CredentialsMissing`, `RunsRootInvalid`,
//!   `InnerLoopExhausted`, `Crashed`).
//! - `Outcome` / `MergeReport` — the value types the CLI shim maps
//!   onto exit codes (0 on green+merged, 1 on `InnerLoopExhausted` /
//!   `Crashed`, 2 on every could-not-verdict refusal).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use agentic_runtime::Runtime;
use agentic_store::Store;

/// Simple stdout event sink for NDJSON output
struct StdoutSink;

impl agentic_runtime::EventSink for StdoutSink {
    fn emit(&mut self, line: &str) {
        println!("{}", line);
    }
}

/// Host-side configuration for `agentic story build <id>`.
///
/// Carries all the resolved paths, image tag, docker binary,
/// and budget needed to compose the `docker run` argv and
/// launch the container.
#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub story_id: i64,
    pub run_id: String,
    pub model: String,
    pub image_tag: String,
    pub docker_binary: PathBuf,
    pub runs_root: PathBuf,
    pub story_yaml_path: PathBuf,
    pub snapshot_path: PathBuf,
    pub credentials_path: PathBuf,
    pub max_inner_loop_iterations: u32,
    pub start_sha: String,
}

/// Container-side configuration for `agentic story build --in-sandbox <id>`.
///
/// Carried from the host via mounted files and environment variables.
#[derive(Debug, Clone)]
pub struct InSandboxConfig {
    pub story_id: i64,
    pub run_id: String,
    pub signer: String,
    pub story_yaml_path: PathBuf,
    pub snapshot_path: PathBuf,
    pub runs_root: PathBuf,
    pub start_sha: String,
    pub max_inner_loop_iterations: u32,
    pub model: String,
}

/// Image tag resolution choice.
#[derive(Debug, Clone)]
pub enum ImageTagChoice {
    /// Per-commit SHA tag was found locally.
    PerSha { tag: String },

    /// Per-SHA tag not found; falling back to `:latest`.
    LatestFallback { tag: String, requested_sha: String },

    /// Neither per-SHA nor `:latest` was found.
    NotFound { requested_sha: String },
}

/// Resolver for image tags.
///
/// Encapsulates the logic of checking local docker images
/// for the per-SHA tag and falling back to `:latest`.
#[derive(Debug)]
pub struct ImageTagResolver {
    sha: String,
    local_tags: Vec<String>,
}

impl ImageTagResolver {
    /// Create a new resolver for the given commit SHA.
    pub fn new(sha: String) -> Self {
        ImageTagResolver {
            sha,
            local_tags: Vec::new(),
        }
    }

    /// Add a locally-present tag to the resolver (for testing).
    pub fn with_local_tag_present(mut self, tag: String) -> Self {
        self.local_tags.push(tag);
        self
    }

    /// Resolve the image tag.
    pub fn resolve(&self) -> ImageTagChoice {
        let per_sha_tag = format!("agentic-sandbox:{}", self.sha);
        let latest_tag = "agentic-sandbox:latest".to_string();

        if self.local_tags.iter().any(|t| t == &per_sha_tag) {
            ImageTagChoice::PerSha { tag: per_sha_tag }
        } else if self.local_tags.iter().any(|t| t == &latest_tag) {
            ImageTagChoice::LatestFallback {
                tag: latest_tag,
                requested_sha: self.sha.clone(),
            }
        } else {
            ImageTagChoice::NotFound {
                requested_sha: self.sha.clone(),
            }
        }
    }
}

/// Typed errors from the story-build flow.
#[derive(Debug)]
pub enum StoryBuildError {
    /// Docker binary is not found at the resolved path.
    DockerUnavailable { binary: PathBuf },

    /// Git user.email is not configured.
    GitIdentityMissing,

    /// Image tag is not present locally.
    ImageTagNotFound { tag: String },

    /// Credentials file is missing or unreadable.
    CredentialsMissing,

    /// Runs root is invalid or unreadable.
    RunsRootInvalid { path: PathBuf },

    /// Ancestor snapshot is insufficient — missing a required signing.
    AncestorSnapshotInsufficient { missing_ancestor: i64 },

    /// Start SHA drifted — main has moved since sandbox launch.
    StartShaDrift {
        expected_start_sha: String,
        actual_main_sha: String,
    },

    /// Inner loop exhausted its budget without reaching green.
    InnerLoopExhausted { iterations: u32, reason: String },

    /// Inner loop crashed (subprocess exited non-zero).
    Crashed { reason: String },

    /// Store operation failed.
    Store(String),

    /// Runtime operation failed.
    Runtime(String),

    /// IO error.
    Io(String),

    /// Git operation failed.
    Git(String),
}

/// Outcome of a sandbox run — returned by `run_in_sandbox`.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// Inner loop reached green: tests pass and UAT signed.
    Green {
        run_id: String,
        signing_signer: String,
    },

    /// Inner loop exhausted budget without reaching green.
    InnerLoopExhausted { iterations: u32 },

    /// Inner loop crashed.
    Crashed { reason: String },
}

/// The result of auto-merging a green run onto main.
#[derive(Debug, Clone)]
pub struct MergeReport {
    pub merged: bool,
    pub merge_shas: Vec<String>,
    pub error: Option<String>,
}

/// Host-side orchestrator for `agentic story build <id>`.
///
/// Composes docker argv, launches the container, reads back the run row,
/// and optionally auto-merges on green.
pub struct StoryBuild {
    cfg: BuildConfig,
}

impl StoryBuild {
    /// Construct a StoryBuild from a BuildConfig.
    ///
    /// This is a construction-time operation; runtime validation
    /// (docker availability, etc.) happens in `run()`.
    pub fn from_config(cfg: BuildConfig) -> Result<Self, StoryBuildError> {
        Ok(StoryBuild { cfg })
    }

    /// Compose the docker run argv for the host.
    ///
    /// Returns a Vec<String> with stable ordering of flags.
    pub fn compose_docker_argv(&self) -> Vec<String> {
        let mut argv = vec![
            self.cfg.docker_binary.to_string_lossy().to_string(),
            "run".to_string(),
            "--rm".to_string(),
        ];

        // Mounts (in stable order):
        // 1. story.yml (read-only)
        // 2. credentials (read-only)
        // 3. snapshot (read-only)
        // 4. runs root (read-write)
        argv.push("-v".to_string());
        argv.push(format!(
            "{}:/work/story.yml:ro",
            self.cfg.story_yaml_path.display()
        ));

        argv.push("-v".to_string());
        argv.push(format!(
            "{}:/work/.claude/.credentials.json:ro",
            self.cfg.credentials_path.display()
        ));

        argv.push("-v".to_string());
        argv.push(format!(
            "{}:/work/snapshot.json:ro",
            self.cfg.snapshot_path.display()
        ));

        argv.push("-v".to_string());
        argv.push(format!("{}:/output/runs", self.cfg.runs_root.display()));

        // Environment variables (in stable order)
        argv.push("-e".to_string());
        argv.push(format!(
            "AGENTIC_SIGNER=sandbox:{}@{}",
            self.cfg.model, self.cfg.run_id
        ));

        argv.push("-e".to_string());
        argv.push(format!("AGENTIC_RUN_ID={}", self.cfg.run_id));

        // Image tag
        argv.push(self.cfg.image_tag.clone());

        // Command tail
        argv.extend(vec![
            "agentic".to_string(),
            "story".to_string(),
            "build".to_string(),
            "--in-sandbox".to_string(),
            self.cfg.story_id.to_string(),
        ]);

        argv
    }

    /// Run the story build on the host (launches docker container).
    ///
    /// Validates inputs, creates the runs root, launches the container,
    /// waits for completion, reads back the run row, and optionally
    /// auto-merges on green.
    pub fn run(&self, _store: Arc<dyn Store>) -> Result<MergeReport, StoryBuildError> {
        // Validation: docker binary exists
        if !self.cfg.docker_binary.exists() {
            return Err(StoryBuildError::DockerUnavailable {
                binary: self.cfg.docker_binary.clone(),
            });
        }

        // TODO: Implement host-side orchestration
        // - Create runs root
        // - Snapshot for story
        // - Compose docker argv
        // - Spawn docker run
        // - Read back run row
        // - Auto-merge on green
        todo!("host-side run orchestration")
    }

    /// Run the story build inside the container (--in-sandbox mode).
    ///
    /// Initializes embedded store, restores ancestor snapshot, checks gate,
    /// spawns inner-loop agent, writes run row + trace.
    pub async fn run_in_sandbox(
        _cfg: InSandboxConfig,
        _store: Arc<dyn Store>,
    ) -> Result<Outcome, StoryBuildError> {
        // Default runtime: ClaudeCodeRuntime
        todo!("in-sandbox run orchestration with default runtime")
    }

    /// Run the story build inside the container with an injected runtime.
    ///
    /// Used by tests to stub the runtime.
    pub async fn run_in_sandbox_with_runtime(
        _cfg: InSandboxConfig,
        _store: Arc<dyn Store>,
        _runtime: Arc<dyn Runtime>,
    ) -> Result<Outcome, StoryBuildError> {
        // TODO: Full implementation
        todo!("in-sandbox run orchestration with injected runtime")
    }

    /// Auto-merge a green run onto main as a squash commit.
    ///
    /// Takes the run ID, looks up the run row, verifies the start SHA
    /// matches main's current HEAD, creates a squash commit with the
    /// documented body format, and updates the run row.
    pub fn merge_run_if_green(
        _repo_path: &Path,
        _store: Arc<dyn Store>,
        _run_id: &str,
        _story_title: &str,
    ) -> Result<MergeReport, StoryBuildError> {
        // TODO: Implement merge orchestration
        // - Query run row from store
        // - Check outcome == green
        // - Check start_sha == current main HEAD
        // - Squash-merge branch onto main
        // - Update run row with merged=true + merge_shas
        todo!("merge orchestration")
    }

    /// Record a failed run (exhausted or crashed) without merging.
    ///
    /// Updates the run row but does not merge onto main.
    pub fn record_failed_run(
        _repo_path: &Path,
        _store: Arc<dyn Store>,
        _run_id: &str,
    ) -> Result<(), StoryBuildError> {
        // TODO: Implement failed-run recording
        // - Query run row
        // - Verify outcome is not green
        // - Mark merged=false
        todo!("failed-run recording")
    }
}
