//! Story 20 acceptance test: `StoryBuild::resolve_image_tag` defaults
//! to `agentic-sandbox:<sha>` (the sha of the host's current HEAD at
//! invocation time) and falls back to `agentic-sandbox:latest` only
//! when the per-sha tag is not locally present. The `#[ignore]`-d
//! companion case exercises the real `docker build` / `docker image
//! inspect` tag-plurality contract; the always-on case tests the
//! resolver's logic via a stubbed presence oracle.
//!
//! Justification (from stories/20.yml acceptance.tests[10]):
//!   Proves the image-tagging contract: the image built
//!   from `infra/sandbox/Dockerfile` via the documented
//!   build command produces exactly two tags that resolve
//!   to the same image ID — `agentic-sandbox:<sha>` and
//!   `agentic-sandbox:latest`.
//!   `StoryBuild::resolve_image_tag` defaults to
//!   `agentic-sandbox:<sha-at-host-invocation>` and falls
//!   back to `agentic-sandbox:latest` only when the per-sha
//!   tag is not locally present. A `docker image inspect
//!   agentic-sandbox:<sha>` returns an image whose labels
//!   include `agentic.commit=<sha>`.
//!
//! Red today: compile-red via the missing `agentic_story_build`
//! public surface (`StoryBuild::resolve_image_tag`, `ImageTagResolver`,
//! `ImageTagChoice`).
//!
//! This scaffold deliberately stubs the docker-presence oracle via
//! `ImageTagResolver::with_local_tag_present` rather than shelling
//! out to a real docker daemon. Per story 20 guidance ("Integration
//! tests that actually spawn docker are gated behind a `#[ignore]`
//! attribute and a `cargo test -- --ignored` opt-in; the
//! `acceptance.tests[]` entries above do NOT require a running
//! Docker daemon to execute — they stub the runtime and the docker
//! process"), an acceptance scaffold must fail on its OWN merits,
//! not on a missing daemon.

use agentic_story_build::{ImageTagChoice, ImageTagResolver};

#[test]
fn resolve_image_tag_prefers_per_sha_then_falls_back_to_latest() {
    let sha = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string();

    // Case 1: per-sha tag IS locally present — resolver returns it.
    let resolver_with_sha =
        ImageTagResolver::new(sha.clone()).with_local_tag_present(format!("agentic-sandbox:{sha}"));
    let choice_with_sha = resolver_with_sha.resolve();
    match &choice_with_sha {
        ImageTagChoice::PerSha { tag } => {
            assert_eq!(
                tag,
                &format!("agentic-sandbox:{sha}"),
                "PerSha tag must be agentic-sandbox:<sha>; got {tag:?}"
            );
        }
        other => {
            panic!("resolve_image_tag must prefer the per-sha tag when present; got {other:?}")
        }
    }

    // Case 2: per-sha tag is NOT present, `:latest` IS — resolver
    // returns the LatestFallback variant (it is NOT silent; the
    // caller is expected to emit a stderr warning).
    let resolver_fallback = ImageTagResolver::new(sha.clone())
        .with_local_tag_present("agentic-sandbox:latest".to_string());
    let choice_fallback = resolver_fallback.resolve();
    match &choice_fallback {
        ImageTagChoice::LatestFallback { tag, requested_sha } => {
            assert_eq!(tag, "agentic-sandbox:latest");
            assert_eq!(
                requested_sha, &sha,
                "LatestFallback must carry the sha that was not found locally; got {requested_sha:?}"
            );
        }
        other => panic!(
            "resolve_image_tag must fall back to :latest when the per-sha tag is absent; got {other:?}"
        ),
    }

    // Case 3: neither present — the resolver returns a typed
    // `NotFound` variant so the caller can refuse with
    // `StoryBuildError::ImageTagNotFound` at the CLI boundary.
    let resolver_none = ImageTagResolver::new(sha.clone());
    let choice_none = resolver_none.resolve();
    match &choice_none {
        ImageTagChoice::NotFound { requested_sha } => {
            assert_eq!(requested_sha, &sha);
        }
        other => panic!(
            "resolve_image_tag must return NotFound when neither per-sha nor :latest is present; got {other:?}"
        ),
    }
}
