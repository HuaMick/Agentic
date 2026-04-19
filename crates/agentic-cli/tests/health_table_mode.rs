//! Story 8 acceptance test: the table-mode wiring end-to-end.
//!
//! Justification (from stories/8.yml): proves the table-mode wiring —
//! running the `agentic` binary with `stories health` against a fixture
//! `stories/` directory and an empty tempdir store writes the table
//! header row, zero data rows (because the fixture stories directory
//! is empty), and exits 0. Without this the CLI-to-`Dashboard::render_table`
//! wire is a library-level claim only — story 3 pins rendering
//! behaviour against `Dashboard` directly, not through the binary, so
//! a broken argv-to-Dashboard path would go unnoticed.
//!
//! The scaffold builds a `TempDir` containing an empty `stories/`
//! directory and a fresh git repo (HEAD required so the binary's own
//! startup path is well-formed if it queries git), invokes the
//! compiled `agentic` binary with `stories health --store <tempdir>`,
//! and asserts exit 0, the exact header vocabulary, and the absence of
//! any data row.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn stories_health_emits_table_header_with_zero_data_rows_and_exits_zero() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Seed a git repo so the binary has a HEAD to resolve if its
    // dashboard path needs one. Empty stories dir on purpose.
    init_repo_and_commit_seed(repo_root);

    let store_tmp = TempDir::new().expect("store tempdir");

    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve — the [[bin]] target in Cargo.toml is `agentic`")
        .current_dir(repo_root)
        .arg("stories")
        .arg("health")
        .arg("--store")
        .arg(store_tmp.path())
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status = output.status;

    assert!(
        status.success(),
        "`agentic stories health --store <tempdir>` must exit 0 on an empty \
         fixture stories directory; got status={status:?}\nstdout:\n{stdout}\n\
         stderr:\n{stderr}"
    );

    // Table header exactly as pinned by story 3 and named in story 8
    // guidance. The story's table columns are ID, Title, Health,
    // Failing tests, Healthy at — we assert each column label appears
    // in a single header line so the row is unambiguously the header.
    let header_line = stdout
        .lines()
        .find(|l| {
            l.contains("ID")
                && l.contains("Title")
                && l.contains("Health")
                && l.contains("Failing tests")
                && l.contains("Healthy at")
        })
        .unwrap_or_else(|| {
            panic!(
                "stdout must contain a table header line naming ID, Title, \
                 Health, Failing tests, Healthy at; got:\n{stdout}"
            )
        });
    assert!(
        header_line.contains('|'),
        "table header must be pipe-separated; got header line: {header_line:?}\n\
         full stdout:\n{stdout}"
    );

    // Zero data rows: the only line containing a pipe-separated shape
    // should be the header (and any decorative separator). A data row
    // would mention a story id as an integer; no fixture stories were
    // authored, so no integer row may appear.
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == header_line.trim() {
            continue;
        }
        // Accept separator rows (e.g. `----+----+----`) and the
        // resolved-store stderr-mirror line if the impl chose to
        // duplicate on stdout — but reject anything starting with an
        // ASCII digit, which would indicate a data row.
        let first = trimmed.chars().next().unwrap_or(' ');
        assert!(
            !first.is_ascii_digit(),
            "table must have zero data rows on an empty fixture; found row: {line:?}\n\
             full stdout:\n{stdout}"
        );
    }
}

/// Initialise a git repo at `root`, stage everything currently there,
/// commit a seed, and return the SHA. See uat_pass.rs in agentic-uat
/// for the same pattern.
fn init_repo_and_commit_seed(root: &Path) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder").expect("set user.name");
        cfg.set_str("user.email", "test@agentic.local")
            .expect("set user.email");
    }
    let mut index = repo.index().expect("repo index");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    let sig = repo.signature().expect("repo signature");
    let commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
    commit_oid.to_string()
}
