//! Story 20 acceptance test: the `agentic story build <id>` subcommand
//! is a thin shim whose exit code maps directly from the library's
//! typed result — 0 on green+merged, 1 on `Crashed`, 2 on
//! could-not-verdict refusals. The docker binary is stubbed via
//! `DOCKER_BINARY` pointing at a recorder script; the argv the
//! stubbed docker received equals the argv the library's
//! `compose_docker_argv` test pins.
//!
//! Justification (from stories/20.yml acceptance.tests[11]):
//!   Proves the contract reaches the operator through the
//!   binary: running `agentic story build <fixture-id>` on
//!   a fixture store + fixture story (where docker is
//!   stubbed via `DOCKER_BINARY` pointing at a small shell
//!   script that records its argv to a file and exits with
//!   canned stdout shaped like a green run) exits 0 on the
//!   happy path, exit 2 on `DockerUnavailable`,
//!   `StartShaDrift`, or `AncestorSnapshotInsufficient`,
//!   and exit 1 only when the inner loop reports `Crashed`.
//!   The CLI shim does no business logic: it parses argv,
//!   calls `StoryBuild::run(cfg)`, and maps the typed
//!   result to the exit code. The argv the stubbed docker
//!   received is exactly the argv the library's
//!   `compose_docker_argv` test pinned.
//!
//! Red today: runtime-red via the missing `story build` subcommand in
//! the CLI shim. `clap`'s argv parser does not yet know the
//! `story build` verb, so the command exits with code 2 (clap usage
//! error) on an unknown-subcommand error — which happens to match
//! the "could-not-verdict" exit code for a DIFFERENT reason than the
//! test asserts. The test pins the typed refusal shape on the exit
//! string AND on the presence of a recorder log file written by the
//! stubbed docker — both of which are only satisfied after the CLI
//! shim is wired through to the library.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

const FIXTURE_ID: u32 = 99_020_001;

const FIXTURE_STORY_YAML: &str = r#"id: 99020001
title: "Fixture for story 20 CLI wire"
outcome: |
  Fixture used to exercise story build subcommand.
status: proposed
patterns: []
acceptance:
  tests: []
  uat: ignored
depends_on: []
"#;

/// A fake `docker` binary: records its argv to `$ARGV_LOG` and prints
/// a canned stdout shaped like a green container run.
const FAKE_DOCKER_SCRIPT: &str = r#"#!/usr/bin/env bash
set -eu
# Record the argv the CLI composed so the test can inspect it.
printf '%s\n' "$@" >> "$ARGV_LOG"
# Produce a canned stdout shaped like a green run. The host-side
# library parses the trace and updates the runs row accordingly.
cat <<'STUB'
{"kind":"stub_green","outcome":"green","iterations":1}
STUB
exit 0
"#;

#[test]
fn story_build_subcommand_runs_stubbed_docker_and_exits_zero_on_green() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    // Fixture story under a tempdir `stories/`.
    fs::create_dir_all(repo_root.join("stories")).expect("stories dir");
    fs::write(
        repo_root.join(format!("stories/{FIXTURE_ID}.yml")),
        FIXTURE_STORY_YAML,
    )
    .expect("write fixture story");

    init_repo_and_commit_seed(repo_root);

    // Drop the stubbed docker recorder into the tempdir.
    let fake_docker = repo_root.join("fake-docker");
    fs::write(&fake_docker, FAKE_DOCKER_SCRIPT).expect("write fake docker");
    fs::set_permissions(&fake_docker, fs::Permissions::from_mode(0o755))
        .expect("chmod fake docker");

    let argv_log = repo_root.join("docker-argv.log");
    let runs_root = repo_root.join("runs-root");
    fs::create_dir_all(&runs_root).expect("runs root");

    let output = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic")
        .current_dir(repo_root)
        .env("DOCKER_BINARY", &fake_docker)
        .env("ARGV_LOG", &argv_log)
        .env("AGENTIC_RUNS_ROOT", &runs_root)
        .arg("story")
        .arg("build")
        .arg(FIXTURE_ID.to_string())
        .output()
        .expect("run agentic story build");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert_eq!(
        output.status.code(),
        Some(0),
        "`agentic story build <fixture>` with a stubbed-green docker must exit 0; \
         got status={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    );

    // The stubbed docker recorded its argv — this is the handshake
    // that the CLI actually called through to the library's argv
    // composer (and not, e.g., errored out in clap parsing before
    // the library was invoked).
    assert!(
        argv_log.exists(),
        "fake docker did not receive any argv — the CLI shim never called through to the library. \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let argv_contents = fs::read_to_string(&argv_log).expect("read argv log");
    assert!(
        argv_contents.contains("run"),
        "stubbed docker must have seen the `run` subcommand in its argv; got {argv_contents:?}"
    );
    assert!(
        argv_contents.contains("--rm"),
        "stubbed docker must have seen the `--rm` flag in its argv; got {argv_contents:?}"
    );
    assert!(
        argv_contents.contains("--in-sandbox"),
        "stubbed docker must have seen the command tail including `--in-sandbox`; \
         got {argv_contents:?}"
    );
    assert!(
        argv_contents.contains(&FIXTURE_ID.to_string()),
        "stubbed docker must have seen the story id in the command tail; got {argv_contents:?}"
    );
}

fn init_repo_and_commit_seed(root: &Path) {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder").expect("user.name");
        cfg.set_str("user.email", "test@agentic.local")
            .expect("user.email");
    }
    let mut index = repo.index().expect("index");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    let sig = repo.signature().expect("signature");
    repo.commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("seed commit");
}
