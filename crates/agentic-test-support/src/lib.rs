//! Shared test fixture primitives for the agentic workspace.
//!
//! This crate provides setup, fixture, and stub-executor material that
//! would otherwise be reimplemented across multiple test files. It ships
//! fixture machinery only — no assertion helpers. See the README for the
//! catalogue of available primitives.
//!
//! # Single-file by design
//!
//! The kit is intentionally a single `lib.rs` with five public names.
//! The interface boundary is the `pub` surface — what callers `use` —
//! not the file layout behind it. Splitting into per-primitive modules
//! would invert the deep-modules principle this kit exists to embody:
//! treating internal seams as architecturally load-bearing inside a
//! kit whose whole thesis is the opposite. Revisit the split when the
//! kit grows past ~6 primitives or ~800-1000 lines; at that point the
//! split is justified by accumulated friction. Until then, the
//! catalogue lives here, in one file, with banner comments separating
//! the five primitives.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agentic_ci_record::{ExecutorOutcome, TestExecutor};
use agentic_uat::{ExecutionOutcome, UatExecutor, Verdict};
use agentic_story::Story;
use tempfile::TempDir;

// ============================================================================
// FixtureCorpus
// ============================================================================

/// A temporary directory containing a `stories/` subdirectory,
/// used for authoring and validating fixture story YAML.
///
/// Manages the lifecycle of the tempdir; it is deleted when the
/// `FixtureCorpus` is dropped. Callers typically construct a corpus
/// via `new()`, write fixture stories via `write_story()`, and pass
/// the `stories_dir()` to a loader.
pub struct FixtureCorpus {
    _tempdir: TempDir,
    stories_path: PathBuf,
}

impl FixtureCorpus {
    /// Create a new temporary corpus rooted at a tempdir with a
    /// `stories/` subdirectory already created.
    pub fn new() -> Self {
        let tempdir = TempDir::new().expect("create tempdir for FixtureCorpus");
        let stories_path = tempdir.path().join("stories");
        std::fs::create_dir(&stories_path)
            .expect("create stories/ subdirectory in FixtureCorpus");

        Self {
            _tempdir: tempdir,
            stories_path,
        }
    }

    /// Return the path to the root tempdir.
    pub fn path(&self) -> &Path {
        self._tempdir.path()
    }

    /// Return the path to the `stories/` subdirectory.
    pub fn stories_dir(&self) -> PathBuf {
        self.stories_path.clone()
    }

    /// Author a fixture story at `stories/<id>.yml` with the given
    /// dependencies. Returns a `StoryFixture` handle pointing to the
    /// written file.
    pub fn write_story(&self, id: u32, depends_on: &[u32]) -> StoryFixture {
        let fixture = StoryFixture::new(id)
            .with_depends_on(depends_on.to_vec());

        let story_path = self.stories_path.join(format!("{}.yml", id));
        let yaml = fixture.to_yaml();
        std::fs::write(&story_path, yaml)
            .expect("write story fixture to disk");

        StoryFixture {
            id,
            title: fixture.title,
            outcome: fixture.outcome,
            status: fixture.status,
            depends_on: fixture.depends_on,
            path: story_path,
        }
    }
}

impl Default for FixtureCorpus {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// StoryFixture
// ============================================================================

/// An in-memory representation of a fixture story, with YAML authoring
/// and serialization support.
///
/// Constructed via `StoryFixture::new(id)` with optional builder-style
/// setters for title, outcome, status, and depends_on. The `to_yaml()`
/// method produces schema-clean YAML that round-trips through the
/// production `agentic_story::Story::load_dir` loader.
#[derive(Debug, Clone)]
pub struct StoryFixture {
    pub id: u32,
    pub title: String,
    pub outcome: String,
    pub status: String,
    pub depends_on: Vec<u32>,
    path: PathBuf,
}

impl StoryFixture {
    /// Create a new fixture story with the given id. Defaults to
    /// reasonable minimal values for title, outcome, and status.
    pub fn new(id: u32) -> Self {
        Self {
            id,
            title: format!("Fixture story {}", id),
            outcome: format!("Outcome for fixture story {}", id),
            status: "under_construction".to_string(),
            depends_on: Vec::new(),
            path: PathBuf::new(),
        }
    }

    /// Set the title (builder-style).
    pub fn with_title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    /// Set the outcome (builder-style).
    pub fn with_outcome(mut self, outcome: String) -> Self {
        self.outcome = outcome;
        self
    }

    /// Set the status (builder-style).
    pub fn with_status(mut self, status: String) -> Self {
        self.status = status;
        self
    }

    /// Set the depends_on list (builder-style).
    pub fn with_depends_on(mut self, depends_on: Vec<u32>) -> Self {
        self.depends_on = depends_on;
        self
    }

    /// Return the path to the written YAML file.
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// Produce schema-clean YAML that round-trips through the production
    /// `agentic_story::Story::load_dir` loader.
    pub fn to_yaml(&self) -> String {
        let depends_on_yaml = if self.depends_on.is_empty() {
            String::new()
        } else {
            let ids = self
                .depends_on
                .iter()
                .map(|id| format!("- {}", id))
                .collect::<Vec<_>>()
                .join("\n");
            format!("depends_on:\n{}\n", ids)
        };

        format!(
            r#"id: {}
title: {}
outcome: |
  {}
status: {}
patterns: []
assets: []
acceptance:
  tests: []
  uat: ""
guidance: ""
{}
"#,
            self.id, self.title, self.outcome, self.status, depends_on_yaml
        )
    }
}

// ============================================================================
// FixtureRepo
// ============================================================================

/// A git repository initialized with a committer email and one seed commit.
///
/// Used to provide a stable, deterministic git context for tests that
/// need to capture a commit SHA or verify git state. Initialized via
/// `init()` or `init_with_email()`, which sets up a minimal git repo
/// and lands one initial commit.
pub struct FixtureRepo {
    repo: git2::Repository,
    committer_email: String,
}

impl std::fmt::Debug for FixtureRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixtureRepo")
            .field("committer_email", &self.committer_email)
            .finish()
    }
}

impl FixtureRepo {
    /// Initialize a git repository at the given path with a default
    /// committer email of "test@example.com", and land one seed commit.
    pub fn init(path: &Path) -> Self {
        Self::init_with_email(path, "test@example.com")
    }

    /// Initialize a git repository at the given path with the given
    /// committer email, and land one seed commit.
    pub fn init_with_email(path: &Path, email: &str) -> Self {
        let repo = git2::Repository::init(path).expect("initialize git repo");

        // Set committer email in the repo config
        let mut config = repo.config().expect("get repo config");
        config
            .set_str("user.email", email)
            .expect("set user.email");

        // Create a minimal commit
        Self::create_seed_commit(&repo);

        Self {
            repo,
            committer_email: email.to_string(),
        }
    }

    /// Create the initial seed commit.
    fn create_seed_commit(repo: &git2::Repository) {
        // Create a minimal file
        let seed_path = repo.path().parent().unwrap().join("seed.txt");
        std::fs::write(&seed_path, "seed").expect("write seed file");

        // Add to index
        let mut index = repo.index().expect("get index");
        index
            .add_path(std::path::Path::new("seed.txt"))
            .expect("add seed.txt to index");
        index.write().expect("write index");

        // Commit
        let tree_id = index.write_tree().expect("write tree");
        let tree = repo.find_tree(tree_id).expect("find tree");

        let signature = git2::Signature::now("Test User", "test@example.com")
            .expect("create signature");

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )
        .expect("commit");
    }

    /// Return the committer email this repo was initialized with.
    pub fn committer_email(&self) -> &str {
        &self.committer_email
    }

    /// Return the full 40-character lowercase hex SHA of HEAD.
    ///
    /// The returned SHA matches the canonical `commit` form in
    /// `agents/assets/definitions/identifier-forms.yml`:
    /// `^[0-9a-f]{40}$` — full SHA, no abbreviation.
    pub fn head_sha(&self) -> String {
        let head = self.repo.head().expect("get HEAD");
        let oid = head.target().expect("get HEAD OID");
        format!("{}", oid)
    }

    /// Commit a seed file and return its SHA.
    ///
    /// Useful for tests that need multiple commits with distinct SHAs.
    /// Each call appends to the seed file and creates a new commit.
    pub fn commit_seed(&self) -> String {
        let seed_path = self.repo.path().parent().unwrap().join("seed.txt");

        // Append to the seed file
        let mut content = std::fs::read_to_string(&seed_path).unwrap_or_default();
        content.push_str("\nmore seed");
        std::fs::write(&seed_path, content).expect("update seed file");

        // Add and commit
        let mut index = self.repo.index().expect("get index");
        index
            .add_path(std::path::Path::new("seed.txt"))
            .expect("add updated seed to index");
        index.write().expect("write index");

        let tree_id = index.write_tree().expect("write tree");
        let tree = self.repo.find_tree(tree_id).expect("find tree");

        let parent_commit = self.repo.head().expect("get HEAD");
        let parent_oid = parent_commit.target().expect("get HEAD OID");
        let parent = self.repo.find_commit(parent_oid).expect("find parent");

        let signature = git2::Signature::now("Test User", &self.committer_email)
            .expect("create signature");

        let oid = self
            .repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                "Seed commit",
                &tree,
                &[&parent],
            )
            .expect("commit");

        format!("{}", oid)
    }
}

// ============================================================================
// RecordedCall
// ============================================================================

/// A per-call record of an executor invocation, exposing the arguments
/// the executor was called with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedCall {
    /// The story ID the executor was invoked with.
    pub story_id: u32,
    /// The test files (absolute paths) the executor was invoked with.
    pub files: Vec<PathBuf>,
}

impl RecordedCall {
    /// Create a new recorded call with the given story ID and files.
    pub fn new(story_id: u32, files: Vec<PathBuf>) -> Self {
        Self { story_id, files }
    }
}

// ============================================================================
// RecordingExecutor
// ============================================================================

/// A stub executor that records every invocation for later inspection.
///
/// Implements both `TestExecutor` and `UatExecutor` traits. Each method
/// records the invocation parameters into an interior `Arc<Mutex<Vec<>>>`,
/// then returns a sensible default outcome (Pass for both).
///
/// Constructed via `default()` or `new()`. Reading recorded calls back
/// via `recorded_calls()` is non-destructive and can be called multiple
/// times.
#[derive(Clone)]
pub struct RecordingExecutor {
    calls: Arc<Mutex<Vec<RecordedCall>>>,
}

impl std::fmt::Debug for RecordingExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordingExecutor")
            .field("calls_recorded", &self.calls.lock().map(|c| c.len()).unwrap_or(0))
            .finish()
    }
}

impl RecordingExecutor {
    /// Create a new `RecordingExecutor` with an empty call history.
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Return a snapshot of the recorded calls.
    ///
    /// This is non-destructive — calling `recorded_calls()` multiple times
    /// returns the same sequence.
    pub fn recorded_calls(&self) -> Vec<RecordedCall> {
        let calls = self.calls.lock().expect("lock recorded calls");
        calls.clone()
    }
}

impl Default for RecordingExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl TestExecutor for RecordingExecutor {
    fn run_tests(&self, story_id: u32, test_files: &[PathBuf]) -> ExecutorOutcome {
        let call = RecordedCall {
            story_id,
            files: test_files.to_vec(),
        };

        let mut calls = self.calls.lock().expect("lock recorded calls");
        calls.push(call);

        ExecutorOutcome::pass()
    }
}

impl UatExecutor for RecordingExecutor {
    fn execute(&self, story: &Story) -> ExecutionOutcome {
        let call = RecordedCall {
            story_id: story.id,
            files: Vec::new(),
        };

        let mut calls = self.calls.lock().expect("lock recorded calls");
        calls.push(call);

        ExecutionOutcome {
            verdict: Verdict::Pass,
            transcript: "recording executor stub".to_string(),
        }
    }
}
