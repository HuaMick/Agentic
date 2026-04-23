//! Story 20 acceptance test: `StoryBuild::compose_docker_argv` returns
//! a strictly-shaped `Vec<String>` whose elements a test can parse
//! positionally — the host-vs-container contract pinned at the
//! library boundary.
//!
//! Justification (from stories/20.yml acceptance.tests[0]):
//!   Proves the host-side argv contract at the library
//!   boundary: given a fixture story id, a `BuildConfig`
//!   carrying `max_inner_loop_iterations: 3`, a resolved
//!   image tag `agentic-sandbox:<sha>`, a runs root, an
//!   ancestor snapshot path, and a credentials path,
//!   `StoryBuild::compose_docker_argv` returns a
//!   `Vec<String>` whose first element is `docker` (or the
//!   resolved docker binary path), whose subcommand is
//!   `run`, whose `--rm` is present, whose `-v` mounts
//!   include the documented `/work/...:ro` shapes and the
//!   read-write runs root, whose `-e` env vars include
//!   `AGENTIC_SIGNER=sandbox:<model>@<run_id>` and
//!   `AGENTIC_RUN_ID=<run-id>`, whose image argument is
//!   `agentic-sandbox:<sha>`, and whose command tail is
//!   `agentic story build --in-sandbox <id>`. The order of
//!   `-v` / `-e` flags is stable across invocations so a
//!   strict parser in the test can compare.
//!
//! Red today: compile-red via the missing `agentic_story_build`
//! public surface (`StoryBuild`, `BuildConfig`). Once the symbols
//! land, the assertions below pin the argv shape verbatim.

use std::path::PathBuf;

use agentic_story_build::{BuildConfig, StoryBuild};

#[test]
fn compose_docker_argv_emits_stable_ordered_mounts_env_and_command_tail() {
    let runs_root = PathBuf::from("/tmp/fixture-runs-root");
    let story_yaml_path = PathBuf::from("/tmp/fixture-stories/20.yml");
    let snapshot_path = PathBuf::from("/tmp/fixture-runs-root/run-42/snapshot.json");
    let credentials_path = PathBuf::from("/home/dev/.claude/.credentials.json");

    let cfg = BuildConfig {
        story_id: 20,
        run_id: "run-42".to_string(),
        model: "claude-sonnet-4-6".to_string(),
        image_tag: "agentic-sandbox:deadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
        docker_binary: PathBuf::from("docker"),
        runs_root: runs_root.clone(),
        story_yaml_path: story_yaml_path.clone(),
        snapshot_path: snapshot_path.clone(),
        credentials_path: credentials_path.clone(),
        max_inner_loop_iterations: 3,
        start_sha: "a09aaed609cdab88ca8dcb0a8be5c7928befbabc".to_string(),
    };

    let build = StoryBuild::from_config(cfg.clone()).expect("StoryBuild::from_config");

    // First observation: two consecutive calls must return byte-identical
    // argv. The order of `-v` and `-e` flags is part of the contract
    // (the justification pins "stable across invocations so a strict
    // parser in the test can compare").
    let argv_a = build.compose_docker_argv();
    let argv_b = build.compose_docker_argv();
    assert_eq!(
        argv_a, argv_b,
        "compose_docker_argv must return byte-identical Vec<String> on repeated calls; \
         got argv_a={argv_a:?}\nargv_b={argv_b:?}"
    );

    // Element 0 is the resolved docker binary.
    assert_eq!(
        argv_a[0], "docker",
        "argv[0] must be the resolved docker binary (or its absolute path); got {:?}",
        argv_a[0]
    );

    // Element 1 is the `run` subcommand.
    assert_eq!(
        argv_a[1], "run",
        "argv[1] must be the `run` subcommand; got {:?}",
        argv_a[1]
    );

    // `--rm` must appear somewhere in the flags section (between
    // `run` and the image tag).
    assert!(
        argv_a.iter().any(|s| s == "--rm"),
        "argv must include `--rm` so containers don't leak between runs; got {argv_a:?}"
    );

    // Three read-only `-v` mounts in the documented shape + one
    // read-write mount for the runs root.
    let story_mount = format!("{}:/work/story.yml:ro", story_yaml_path.display());
    let creds_mount = format!(
        "{}:/work/.claude/.credentials.json:ro",
        credentials_path.display()
    );
    let snapshot_mount = format!("{}:/work/snapshot.json:ro", snapshot_path.display());
    let runs_mount = format!("{}:/output/runs", runs_root.display());

    for expected in [&story_mount, &creds_mount, &snapshot_mount, &runs_mount] {
        let mount_ok = argv_a.windows(2).any(|w| w[0] == "-v" && &w[1] == expected);
        assert!(
            mount_ok,
            "argv must include the `-v` mount {expected:?}; got {argv_a:?}"
        );
    }

    // Two `-e` env entries in the documented shape.
    let signer_env = "AGENTIC_SIGNER=sandbox:claude-sonnet-4-6@run-42".to_string();
    let run_id_env = "AGENTIC_RUN_ID=run-42".to_string();
    for expected in [&signer_env, &run_id_env] {
        let env_ok = argv_a.windows(2).any(|w| w[0] == "-e" && &w[1] == expected);
        assert!(
            env_ok,
            "argv must include the `-e` env {expected:?}; got {argv_a:?}"
        );
    }

    // The image tag appears after the flags section and before the
    // command tail. Its position: the first occurrence of the literal
    // image tag marks that boundary.
    let image_tag = &cfg.image_tag;
    let image_pos = argv_a
        .iter()
        .position(|s| s == image_tag)
        .unwrap_or_else(|| {
            panic!(
                "argv must include the image tag {image_tag:?} as a positional arg; got {argv_a:?}"
            )
        });

    // Command tail: `agentic story build --in-sandbox <id>` follows
    // the image tag, in that exact order, ending the vec.
    let tail = &argv_a[image_pos + 1..];
    assert_eq!(
        tail,
        &[
            "agentic".to_string(),
            "story".to_string(),
            "build".to_string(),
            "--in-sandbox".to_string(),
            cfg.story_id.to_string(),
        ],
        "argv tail after the image tag must be `agentic story build --in-sandbox <id>`; \
         got tail={tail:?}\nfull argv={argv_a:?}"
    );
}
