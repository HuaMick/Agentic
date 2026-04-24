//! Story 17 acceptance test: the single-source-of-truth constant
//! `DEFAULT_BUILD_CONFIG` lives exactly once in the workspace — in
//! `agentic-story` — and its value is the documented defaults.
//!
//! Justification (from stories/17.yml): proves the defaults-locus
//! decision — a single public constant `DEFAULT_BUILD_CONFIG:
//! BuildConfig` (name exact) is exported from `agentic-story`, carrying
//! the documented defaults (`max_inner_loop_iterations: 5`, `models:
//! vec![]`). No other crate re-declares these values; `grep -r
//! "max_inner_loop_iterations: 5"` across the workspace returns
//! exactly ONE match — the constant's definition in `agentic-story`.
//! Without this, the "single source of truth for defaults" decision
//! degrades into "several crates hard-code the same numbers and they
//! drift" — the exact duplicate-constant failure mode CLAUDE.md's
//! "reference, don't restate" rule names.
//!
//! Per the story's guidance the constant lives at
//! `crates/agentic-story/src/build_config.rs` (or wherever `BuildConfig`
//! lives) and is re-exported from the crate root. Red today is
//! compile-red: `BuildConfig` and `DEFAULT_BUILD_CONFIG` do not yet
//! exist on `agentic-story`, so the scaffold's `use` does not resolve.

use std::fs;
use std::path::{Path, PathBuf};

use agentic_story::{BuildConfig, DEFAULT_BUILD_CONFIG};

/// Walk up from `CARGO_MANIFEST_DIR` (this crate's root) to the
/// workspace root. The workspace is identified by the presence of a
/// `Cargo.toml` carrying a `[workspace]` section; for this repo that's
/// exactly one directory above.
fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/agentic-story -> crates -> workspace root
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR must have at least two ancestors")
        .to_path_buf()
}

/// Recursively collect every `*.rs` file under the given directory.
/// This is the surface area for the duplicate-constant check. `target/`
/// and hidden dirs are deliberately NOT walked — the former is build
/// output, the latter includes VCS metadata.
///
/// The caller scopes the walk to `crates/agentic-story/src/` only,
/// because story 17 Decision 2's "single source of truth for defaults"
/// is an src-only concern: the constant's value is a Rust literal that
/// must exist in exactly one production-source file. Test fixtures
/// under `crates/*/tests/` that happen to embed the same literal
/// inside YAML strings (e.g. story 6's loader fixtures) are NOT
/// duplicate-constant drift — they are behavioural assertions about
/// specific numeric inputs, and the story's justification is about the
/// defaults-locus in src, not about the string `"max_inner_loop_iterations: 5"`
/// appearing anywhere in the workspace.
fn collect_rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let p = entry.path();
        if p.is_dir() {
            // Skip target/ (build output) and hidden dirs.
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if name == "target" || name.starts_with('.') {
                    continue;
                }
            }
            collect_rust_sources(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

#[test]
fn default_build_config_constant_is_the_only_defaults_source() {
    // Pin the constant's value. These are the numbers stories 17, 19,
    // and future consumers all read THROUGH this constant — changing
    // the value changes the default budget the runtime seeds when an
    // author declines to opine.
    assert_eq!(
        DEFAULT_BUILD_CONFIG.max_inner_loop_iterations, 5,
        "DEFAULT_BUILD_CONFIG.max_inner_loop_iterations must be 5 per \
         story 17's Decision 2; got {}",
        DEFAULT_BUILD_CONFIG.max_inner_loop_iterations
    );
    assert!(
        DEFAULT_BUILD_CONFIG.models.is_empty(),
        "DEFAULT_BUILD_CONFIG.models must be an empty Vec per story 17's \
         Decision 2 (runtime picks default at consumption time); got {:?}",
        DEFAULT_BUILD_CONFIG.models
    );

    // Pin the constant against a reconstructed expected value. This
    // catches any additional field the implementer accidentally
    // populates (the `BuildConfig` struct shape is pinned by story 17
    // at exactly two public fields).
    let expected = BuildConfig {
        max_inner_loop_iterations: 5,
        models: Vec::new(),
    };
    assert_eq!(
        DEFAULT_BUILD_CONFIG, expected,
        "DEFAULT_BUILD_CONFIG must equal BuildConfig {{ \
         max_inner_loop_iterations: 5, models: vec![] }}; got {:?}",
        DEFAULT_BUILD_CONFIG
    );

    // Single-source check: scan every `*.rs` under
    // `crates/agentic-story/src/` for the literal
    // `max_inner_loop_iterations: 5` pattern. Exactly one match — the
    // constant's own definition — is required. Any additional hit in
    // src is a duplicate-constant drift.
    //
    // The scope is deliberately `src/` only (not `crates/**/*.rs`):
    // story 17 Decision 2 pins the defaults-locus as a SOURCE-of-truth
    // constant, which is an src-only concern. YAML-string fixtures in
    // `crates/*/tests/` that happen to embed `max_inner_loop_iterations: 5`
    // as an input to a loader test (e.g. story 6's
    // `load_build_config_is_parsed.rs`,
    // `load_build_config_optional_defaults_apply.rs`) are not
    // alternate declarations of the default — they are behavioural
    // assertions about specific numeric inputs that the loader must
    // round-trip. Including them as "hits" would conflate
    // "defaults live in one place" (the story's actual claim) with
    // "this string appears in one place in the workspace" (a stricter
    // and accidentally false claim).
    let root = workspace_root();
    let agentic_story_src = root
        .join("crates")
        .join("agentic-story")
        .join("src");
    assert!(
        agentic_story_src.is_dir(),
        "`crates/agentic-story/src/` must exist (looked under {})",
        agentic_story_src.display()
    );

    let mut sources: Vec<PathBuf> = Vec::new();
    collect_rust_sources(&agentic_story_src, &mut sources);

    // The needle must match the `<field>: <value>` Rust literal the
    // constant's definition would contain. Whitespace around the colon
    // varies by formatter; the canonical form is a single space.
    const NEEDLE: &str = "max_inner_loop_iterations: 5";

    let mut hits: Vec<PathBuf> = Vec::new();
    for src in &sources {
        let Ok(contents) = fs::read_to_string(src) else {
            continue;
        };
        if contents.contains(NEEDLE) {
            hits.push(src.clone());
        }
    }

    // Exactly one hit inside `crates/agentic-story/src/`: the
    // constant's own definition.
    assert_eq!(
        hits.len(),
        1,
        "exactly one file in `crates/agentic-story/src/` must contain \
         the literal `{NEEDLE}` (the constant's definition); got {} \
         matches: {hits:?}",
        hits.len()
    );
}
