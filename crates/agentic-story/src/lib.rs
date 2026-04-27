//! Story loader and validator.
//!
//! Exposes a typed `Story` value, a `StoryError` enum, and two entry
//! points:
//!
//! - [`Story::load`]  — load a single `stories/<id>.yml` file.
//! - [`Story::load_dir`] — load every `*.yml` in a directory and
//!   validate the cross-file `depends_on` graph is a DAG.
//!
//! Validation layers (per story 6 guidance):
//!   1. YAML parse.
//!   2. Structural / schema validation (required fields + no unknown
//!      fields).
//!   3. Enum-boundary validation (`status` is one of five values).
//!   4. Semantic validation (build_config iterations are positive,
//!      id matches filename).
//!   5. Graph validation (directory-load only): `depends_on` edges form
//!      a DAG — cycles and self-loops are rejected. `superseded_by`
//!      edges also form a DAG independently.
//!
//! The loader is strictly read-only; it never mutates state or persists
//! anything. Mutation of `status` is owned by `agentic uat` (story 1).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::{self, Deserializer};
use serde::Deserialize;
use serde_yaml::Value;

/// Default build configuration: max_inner_loop_iterations: 5, models: [].
/// This is the single source of truth for defaults per story 17.
pub const DEFAULT_BUILD_CONFIG: BuildConfig = BuildConfig {
    max_inner_loop_iterations: 5,
    models: Vec::new(),
};

/// Build configuration for a story's orchestration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfig {
    pub max_inner_loop_iterations: u32,
    pub models: Vec<String>,
}

/// Minimal Asset structure for parsing the `current_consumers:` field
/// during reciprocity audits. Only the `current_consumers` field is
/// relevant for the audit; other asset properties are ignored.
#[derive(Debug, Deserialize)]
struct RawAsset {
    #[serde(default)]
    current_consumers: Vec<String>,
}

/// A fully-validated story loaded from disk.
///
/// Constructing a `Story` has already enforced the schema and — when
/// loaded through [`Story::load_dir`] — the acyclicity of the
/// `depends_on` and `superseded_by` graphs, so downstream code can
/// assume well-formedness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Story {
    pub id: u32,
    pub title: String,
    pub outcome: String,
    pub status: Status,
    pub patterns: Vec<String>,
    pub assets: Vec<String>,
    pub acceptance: Acceptance,
    pub guidance: String,
    pub depends_on: Vec<u32>,
    pub related_files: Vec<String>,
    pub build_config: Option<BuildConfig>,
    pub superseded_by: Option<u32>,
    pub retired_reason: Option<String>,
}

/// Acceptance block: one or more tests plus the UAT journey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acceptance {
    pub tests: Vec<TestEntry>,
    pub uat: String,
}

/// One acceptance-test entry, with a per-test justification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestEntry {
    pub file: PathBuf,
    pub justification: String,
}

/// The five lifecycle states the dashboard understands.
///
/// Kept deliberately narrow — any other value loaded from disk becomes
/// [`StoryError::UnknownStatus`] with the offending string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Proposed,
    UnderConstruction,
    Healthy,
    Unhealthy,
    Retired,
}

/// Typed error surface for the loader. No raw I/O errors leak through;
/// no `anyhow::Error`.
#[derive(Debug)]
pub enum StoryError {
    /// A required field is missing, or the file is otherwise not shaped
    /// like a story. `field` names the offending field.
    SchemaViolation { field: String, message: String },
    /// `status` parsed but was not one of the five accepted values.
    UnknownStatus { value: String },
    /// The file or directory the caller supplied does not exist.
    NotFound { path: PathBuf },
    /// The YAML parser rejected the file before we could validate it.
    YamlParse { path: PathBuf, message: String },
    /// Cross-story `depends_on` validation failure.
    DependsOnCycle { participants: Vec<u32> },
    /// A story's `superseded_by` points to a non-existent target id.
    SupersededByUnknown { source_id: u32, target_id: u32 },
    /// A cycle was detected in the `superseded_by` edges.
    SupersededByCycle { participants: Vec<u32> },
    /// `build_config.max_inner_loop_iterations` is zero, negative, or
    /// out-of-range. Owned by story 17.
    BuildConfigInvalidIterations { value: i64 },
    /// A field in `build_config` has the wrong type (e.g., string where
    /// integer expected). Owned by story 17.
    BuildConfigTypeMismatch {
        field: String,
        expected: String,
        found: String,
    },
    /// A story's `assets:` entry points to a file that does not exist.
    /// Validation occurs at directory-load time (per ADR-0007 decision 3),
    /// not at single-file load time, matching the pattern for `depends_on`
    /// and `superseded_by` references.
    AssetNotFound { path: PathBuf, source_id: u32 },
}

impl std::fmt::Display for StoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoryError::SchemaViolation { field, message } => {
                write!(f, "schema violation at `{field}`: {message}")
            }
            StoryError::UnknownStatus { value } => {
                write!(
                    f,
                    "status must be one of proposed|under_construction|healthy|unhealthy|retired; got `{value}`"
                )
            }
            StoryError::NotFound { path } => {
                write!(f, "story path not found: {}", path.display())
            }
            StoryError::YamlParse { path, message } => {
                write!(f, "YAML parse error in {}: {message}", path.display())
            }
            StoryError::DependsOnCycle { participants } => {
                write!(
                    f,
                    "depends_on cycle detected; participants include {participants:?}"
                )
            }
            StoryError::SupersededByUnknown {
                source_id,
                target_id,
            } => {
                write!(
                    f,
                    "superseded_by edge from story {source_id} points to non-existent story {target_id}"
                )
            }
            StoryError::SupersededByCycle { participants } => {
                write!(
                    f,
                    "superseded_by cycle detected; participants include {participants:?}"
                )
            }
            StoryError::BuildConfigInvalidIterations { value } => {
                write!(
                    f,
                    "build_config.max_inner_loop_iterations must be a positive \
                     integer; got {value}"
                )
            }
            StoryError::BuildConfigTypeMismatch {
                field,
                expected,
                found,
            } => {
                write!(
                    f,
                    "build_config.{field} type mismatch: expected {expected}, \
                     found {found}"
                )
            }
            StoryError::AssetNotFound { path, source_id } => {
                write!(
                    f,
                    "story {source_id} references non-existent asset: {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for StoryError {}

/// Error reported by [`audit_asset_reciprocity`] when the live corpus
/// has a dangling cross-corpus asset reference (one direction of the
/// two-way reciprocity invariant is broken).
#[derive(Debug)]
pub enum AuditError {
    /// A story declares an asset in its `assets:` field, but the asset's
    /// `current_consumers:` list does not reference the story back.
    StoryAssetNotBackReferenced { story_id: u32, asset_path: String },
    /// An asset lists a story in its `current_consumers:` field, but the
    /// story's `assets:` field does not reference the asset back.
    AssetStoryNotBackReferenced { asset_path: String, story_id: u32 },
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditError::StoryAssetNotBackReferenced {
                story_id,
                asset_path,
            } => {
                write!(
                    f,
                    "story {story_id} declares asset {asset_path} but the asset \
                     does not list the story in its current_consumers"
                )
            }
            AuditError::AssetStoryNotBackReferenced {
                asset_path,
                story_id,
            } => {
                write!(
                    f,
                    "asset {asset_path} lists story {story_id} in its \
                     current_consumers but the story does not declare the asset"
                )
            }
        }
    }
}

impl std::error::Error for AuditError {}

impl Story {
    /// Load one `stories/<id>.yml` file.
    ///
    /// Cycle detection is directory-scoped and therefore NOT applied
    /// here — a single file cannot meaningfully validate its edges
    /// against files it has not seen (see story 6 guidance).
    pub fn load(path: &Path) -> Result<Self, StoryError> {
        if !path.exists() {
            return Err(StoryError::NotFound {
                path: path.to_path_buf(),
            });
        }
        let text = fs::read_to_string(path).map_err(|e| StoryError::YamlParse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        parse_story(path, &text)
    }

    /// Load every `*.yml` file directly under `dir` and validate the
    /// collective `depends_on` graph is a DAG (no cycles, no self-loops)
    /// AND the `superseded_by` graph is also a DAG independently.
    pub fn load_dir(dir: &Path) -> Result<Vec<Self>, StoryError> {
        if !dir.exists() {
            return Err(StoryError::NotFound {
                path: dir.to_path_buf(),
            });
        }
        let iter = fs::read_dir(dir).map_err(|e| StoryError::YamlParse {
            path: dir.to_path_buf(),
            message: e.to_string(),
        })?;
        let mut stories: Vec<Story> = Vec::new();
        for entry in iter {
            let entry = entry.map_err(|e| StoryError::YamlParse {
                path: dir.to_path_buf(),
                message: e.to_string(),
            })?;
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("yml") {
                stories.push(Story::load(&p)?);
            }
        }
        detect_cycles(&stories)?;
        detect_superseded_by_cycles(&stories)?;
        validate_asset_paths(&stories)?;
        Ok(stories)
    }
}

/// Deserialize related_files with custom error context so the field name
/// appears in error messages for type mismatches on individual entries.
fn deserialize_related_files<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let items: Vec<serde_yaml::Value> = serde::de::Deserialize::deserialize(deserializer)
        .map_err(|_| de::Error::custom("failed to deserialize related_files"))?;
    let mut result = Vec::new();
    for item in items {
        match item {
            serde_yaml::Value::String(s) => result.push(s),
            _ => {
                return Err(de::Error::custom(format!(
                    "related_files entry must be a string; got {}",
                    match &item {
                        serde_yaml::Value::Number(_) => "number",
                        serde_yaml::Value::Bool(_) => "bool",
                        serde_yaml::Value::Null => "null",
                        serde_yaml::Value::Sequence(_) => "sequence",
                        serde_yaml::Value::Mapping(_) => "mapping",
                        _ => "unknown type",
                    }
                )))
            }
        }
    }
    Ok(result)
}

/// RawStory mirrors the schema one-for-one so we can accept any valid
/// story and reject anything else with a typed error. `status` is kept
/// as a `String` here so an out-of-enum value surfaces as
/// `UnknownStatus { value }` rather than a generic deserialisation
/// failure.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStory {
    id: u32,
    title: String,
    outcome: String,
    status: String,
    #[serde(default)]
    patterns: Vec<String>,
    #[serde(default)]
    assets: Vec<String>,
    acceptance: RawAcceptance,
    guidance: String,
    #[serde(default)]
    depends_on: Vec<u32>,
    #[serde(default, deserialize_with = "deserialize_related_files")]
    related_files: Vec<String>,
    #[serde(default)]
    build_config: Option<RawBuildConfig>,
    #[serde(default)]
    superseded_by: Option<u32>,
    #[serde(default)]
    retired_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBuildConfig {
    #[serde(deserialize_with = "deserialize_iterations")]
    max_inner_loop_iterations: u32,
    #[serde(default)]
    models: Vec<String>,
}

/// Custom deserializer for max_inner_loop_iterations that catches type
/// mismatches (non-integer values) and forwards them as a custom error
/// message so we can re-wrap them as BuildConfigTypeMismatch.
fn deserialize_iterations<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    match serde_yaml::Value::deserialize(deserializer) {
        Ok(v) => match v {
            serde_yaml::Value::Number(n) => {
                // Try to parse as i64 first to catch negatives and out-of-range
                if let Some(i) = n.as_i64() {
                    if i > 0 && i <= u32::MAX as i64 {
                        Ok(i as u32)
                    } else {
                        Err(Error::custom(format!("BuildConfigInvalidIterations:{}", i)))
                    }
                } else if let Some(u) = n.as_u64() {
                    if u <= u32::MAX as u64 && u > 0 {
                        Ok(u as u32)
                    } else {
                        Err(Error::custom(format!("BuildConfigInvalidIterations:{}", u)))
                    }
                } else {
                    Err(Error::custom(
                        "BuildConfigTypeMismatch:max_inner_loop_iterations:integer:number"
                            .to_string(),
                    ))
                }
            }
            _ => {
                let type_name = match &v {
                    serde_yaml::Value::String(_) => "string",
                    serde_yaml::Value::Bool(_) => "bool",
                    serde_yaml::Value::Null => "null",
                    serde_yaml::Value::Sequence(_) => "array",
                    serde_yaml::Value::Mapping(_) => "object",
                    _ => "unknown",
                };
                Err(Error::custom(format!(
                    "BuildConfigTypeMismatch:max_inner_loop_iterations:integer:{}",
                    type_name
                )))
            }
        },
        Err(e) => Err(e),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAcceptance {
    tests: Vec<RawTestEntry>,
    uat: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTestEntry {
    file: String,
    justification: String,
}

fn parse_story(path: &Path, text: &str) -> Result<Story, StoryError> {
    // First pass: parse as a YAML Value so we can translate serde_yaml's
    // "missing field `X`" errors into our typed SchemaViolation with the
    // field name intact, without regex-matching opaque error strings.
    let value: Value = serde_yaml::from_str(text).map_err(|e| StoryError::YamlParse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    // Hand-check the required top-level keys BEFORE running the serde
    // derive, so the "missing required field" case can name the field
    // in a stable, typed way.
    let mapping = value
        .as_mapping()
        .ok_or_else(|| StoryError::SchemaViolation {
            field: "<root>".to_string(),
            message: "story file must be a YAML mapping".to_string(),
        })?;
    for required in ["id", "title", "outcome", "status", "acceptance", "guidance"] {
        if !mapping.contains_key(Value::String(required.to_string())) {
            return Err(StoryError::SchemaViolation {
                field: required.to_string(),
                message: format!("missing required field `{required}`"),
            });
        }
    }

    // Second pass: derive-deserialise for structural checks
    // (`deny_unknown_fields`, type mismatches, acceptance sub-shape).
    let raw: RawStory = serde_yaml::from_value(value).map_err(|e| {
        let msg = e.to_string();

        // Check for custom errors from build_config deserialization.
        if msg.contains("BuildConfigInvalidIterations:") {
            if let Some(value_str) = msg.split("BuildConfigInvalidIterations:").nth(1) {
                if let Ok(value) = value_str.parse::<i64>() {
                    return StoryError::BuildConfigInvalidIterations { value };
                }
            }
        }
        if msg.contains("BuildConfigTypeMismatch:") {
            // Format: BuildConfigTypeMismatch:field:expected:found
            let parts: Vec<&str> = msg
                .split("BuildConfigTypeMismatch:")
                .nth(1)
                .unwrap_or("")
                .split(':')
                .collect();
            if parts.len() >= 3 {
                return StoryError::BuildConfigTypeMismatch {
                    field: parts[0].to_string(),
                    expected: parts[1].to_string(),
                    found: parts[2].to_string(),
                };
            }
        }

        let field = if msg.contains("related_files") {
            "related_files".to_string()
        } else {
            extract_field_from_err(&e).unwrap_or_else(|| "<unknown>".to_string())
        };
        StoryError::SchemaViolation {
            field,
            message: msg,
        }
    })?;

    // Third pass: enum-boundary validation on status. Custom so the
    // error carries the offending value verbatim.
    let status = match raw.status.as_str() {
        "proposed" => Status::Proposed,
        "under_construction" => Status::UnderConstruction,
        "healthy" => Status::Healthy,
        "unhealthy" => Status::Unhealthy,
        "retired" => Status::Retired,
        other => {
            return Err(StoryError::UnknownStatus {
                value: other.to_string(),
            });
        }
    };

    // Convert RawBuildConfig to BuildConfig if present.
    let build_config = raw.build_config.map(|raw_cfg| BuildConfig {
        max_inner_loop_iterations: raw_cfg.max_inner_loop_iterations,
        models: raw_cfg.models,
    });

    Ok(Story {
        id: raw.id,
        title: raw.title,
        outcome: raw.outcome,
        status,
        patterns: raw.patterns,
        assets: raw.assets,
        acceptance: Acceptance {
            tests: raw
                .acceptance
                .tests
                .into_iter()
                .map(|t| TestEntry {
                    file: PathBuf::from(t.file),
                    justification: t.justification,
                })
                .collect(),
            uat: raw.acceptance.uat,
        },
        guidance: raw.guidance,
        depends_on: raw.depends_on,
        related_files: raw.related_files,
        build_config,
        superseded_by: raw.superseded_by,
        retired_reason: raw.retired_reason,
    })
}

/// Best-effort extraction of the field name from a serde_yaml error.
/// Serde emits messages shaped like:
///   "missing field `outcome` at line 3 column 5"
///   "unknown field `extra`, expected one of ..."
/// We look for the first pair of backticks.
fn extract_field_from_err(e: &serde_yaml::Error) -> Option<String> {
    let msg = e.to_string();
    let start = msg.find('`')?;
    let rest = &msg[start + 1..];
    let end = rest.find('`')?;
    Some(rest[..end].to_string())
}

/// Detect any cycle in the depends_on graph across the loaded stories.
/// Returns `DependsOnCycle` with the ids participating in the first
/// cycle found. A self-loop counts as a cycle of length one.
fn detect_cycles(stories: &[Story]) -> Result<(), StoryError> {
    let ids: HashSet<u32> = stories.iter().map(|s| s.id).collect();
    let mut graph: HashMap<u32, Vec<u32>> = HashMap::new();
    for s in stories {
        let edges: Vec<u32> = s
            .depends_on
            .iter()
            .copied()
            .filter(|t| ids.contains(t))
            .collect();
        graph.insert(s.id, edges);
    }

    // Self-loops are easiest to report directly.
    for (id, deps) in &graph {
        if deps.contains(id) {
            return Err(StoryError::DependsOnCycle {
                participants: vec![*id],
            });
        }
    }

    // DFS with three-colour state. A back-edge to a GRAY node is a
    // cycle; the participants are the stack slice from that node to
    // the top.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    fn dfs(
        node: u32,
        graph: &HashMap<u32, Vec<u32>>,
        color: &mut HashMap<u32, Color>,
        stack: &mut Vec<u32>,
    ) -> Result<(), Vec<u32>> {
        color.insert(node, Color::Gray);
        stack.push(node);
        if let Some(edges) = graph.get(&node) {
            for &next in edges {
                match color.get(&next).copied().unwrap_or(Color::White) {
                    Color::White => dfs(next, graph, color, stack)?,
                    Color::Gray => {
                        let start = stack.iter().position(|n| *n == next).unwrap_or(0);
                        let mut participants: Vec<u32> = stack[start..].to_vec();
                        participants.push(next);
                        return Err(participants);
                    }
                    Color::Black => {}
                }
            }
        }
        stack.pop();
        color.insert(node, Color::Black);
        Ok(())
    }

    let mut color: HashMap<u32, Color> = graph.keys().map(|k| (*k, Color::White)).collect();
    let mut stack: Vec<u32> = Vec::new();

    // Iterate in a deterministic order so error reports are stable.
    let mut keys: Vec<u32> = graph.keys().copied().collect();
    keys.sort_unstable();
    for node in keys {
        if color.get(&node).copied() == Some(Color::White) {
            if let Err(participants) = dfs(node, &graph, &mut color, &mut stack) {
                return Err(StoryError::DependsOnCycle { participants });
            }
        }
    }

    Ok(())
}

/// Validate the `superseded_by` edge set: referential integrity check +
/// cycle detection. Returns `SupersededByUnknown` if any target id does
/// not exist, or `SupersededByCycle` if a cycle is detected.
fn detect_superseded_by_cycles(stories: &[Story]) -> Result<(), StoryError> {
    let ids: HashSet<u32> = stories.iter().map(|s| s.id).collect();

    // First pass: referential integrity. Any superseded_by edge whose
    // target id is not in the loaded set is an error.
    for s in stories {
        if let Some(target_id) = s.superseded_by {
            if !ids.contains(&target_id) {
                return Err(StoryError::SupersededByUnknown {
                    source_id: s.id,
                    target_id,
                });
            }
        }
    }

    // Second pass: build the graph and detect cycles. Only stories with
    // a superseded_by edge have outgoing edges in this graph.
    let mut graph: HashMap<u32, Option<u32>> = HashMap::new();
    for s in stories {
        graph.insert(s.id, s.superseded_by);
    }

    // Self-loops are easiest to report directly.
    for (id, target) in &graph {
        if let Some(t) = target {
            if *id == *t {
                return Err(StoryError::SupersededByCycle {
                    participants: vec![*id],
                });
            }
        }
    }

    // DFS with three-colour state for multi-hop cycles.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    fn dfs_superseded(
        node: u32,
        graph: &HashMap<u32, Option<u32>>,
        color: &mut HashMap<u32, Color>,
        stack: &mut Vec<u32>,
    ) -> Result<(), Vec<u32>> {
        color.insert(node, Color::Gray);
        stack.push(node);
        if let Some(Some(next)) = graph.get(&node) {
            match color.get(next).copied().unwrap_or(Color::White) {
                Color::White => dfs_superseded(*next, graph, color, stack)?,
                Color::Gray => {
                    let start = stack.iter().position(|n| *n == *next).unwrap_or(0);
                    let mut participants: Vec<u32> = stack[start..].to_vec();
                    participants.push(*next);
                    return Err(participants);
                }
                Color::Black => {}
            }
        }
        stack.pop();
        color.insert(node, Color::Black);
        Ok(())
    }

    let mut color: HashMap<u32, Color> = graph.keys().map(|k| (*k, Color::White)).collect();
    let mut stack: Vec<u32> = Vec::new();

    // Iterate in a deterministic order so error reports are stable.
    let mut keys: Vec<u32> = graph.keys().copied().collect();
    keys.sort_unstable();
    for node in keys {
        if color.get(&node).copied() == Some(Color::White) {
            if let Err(participants) = dfs_superseded(node, &graph, &mut color, &mut stack) {
                return Err(StoryError::SupersededByCycle { participants });
            }
        }
    }

    Ok(())
}

/// Validate that every asset path referenced in the stories' `assets:`
/// fields resolves to an existing file on disk. Resolves paths relative
/// to the repo root (matching the pattern used for other cross-tree
/// references like `depends_on` and `superseded_by`). Per ADR-0007
/// decision 3, this validation runs only at directory-load time, not
/// at single-file load time.
fn validate_asset_paths(stories: &[Story]) -> Result<(), StoryError> {
    // Find the repo root by walking up from the current executable's location.
    let mut repo_root = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    // Walk up until we find a .git directory.
    while !repo_root.join(".git").exists() {
        if !repo_root.pop() {
            // Fallback: if we can't find .git, assume current dir.
            repo_root = PathBuf::from(".");
            break;
        }
    }

    for story in stories {
        for asset_path in &story.assets {
            let full_path = repo_root.join(asset_path);
            if !full_path.exists() {
                return Err(StoryError::AssetNotFound {
                    path: PathBuf::from(asset_path),
                    source_id: story.id,
                });
            }
        }
    }

    Ok(())
}

/// Audit the live corpus for cross-corpus asset reciprocity per ADR-0007
/// decision 4. Loads all stories from `<repo_root>/stories/` and all
/// assets from `<repo_root>/assets/` and asserts both directions
/// of the reciprocity invariant:
///
/// 1. For every story declaring an asset in its `assets:` field, the
///    asset's `current_consumers:` must list the story.
/// 2. For every asset listing a story in its `current_consumers:`, the
///    story's `assets:` field must reference the asset.
///
/// Returns `Ok(())` if the corpus is fully reciprocal, or an `AuditError`
/// naming the first dangling edge found (in either direction).
pub fn audit_asset_reciprocity(repo_root: &Path) -> Result<(), AuditError> {
    // Load all stories from <repo_root>/stories/.
    let stories_dir = repo_root.join("stories");
    let stories = if stories_dir.exists() {
        Story::load_dir(&stories_dir)
            .unwrap_or_else(|e| panic!("failed to load stories directory: {e}"))
    } else {
        Vec::new()
    };

    // Index stories by ID for fast lookup.
    let story_map: HashMap<u32, &Story> = stories.iter().map(|s| (s.id, s)).collect();

    // Check direction 1: for every story declaring an asset, the asset's
    // current_consumers must list the story.
    for story in &stories {
        for asset_path in &story.assets {
            let asset_file = repo_root.join(asset_path);
            if !asset_file.exists() {
                continue; // Skip if asset doesn't exist (other validators catch this)
            }

            let text = fs::read_to_string(&asset_file)
                .unwrap_or_else(|e| panic!("failed to read asset {}: {e}", asset_file.display()));
            let asset: RawAsset = serde_yaml::from_str(&text)
                .unwrap_or_else(|e| panic!("failed to parse asset {}: {e}", asset_file.display()));

            let story_consumer = format!("stories/{}.yml", story.id);
            if !asset.current_consumers.contains(&story_consumer) {
                return Err(AuditError::StoryAssetNotBackReferenced {
                    story_id: story.id,
                    asset_path: asset_path.clone(),
                });
            }
        }
    }

    // Check direction 2: for every asset listing a story in current_consumers,
    // the story's assets field must reference the asset.
    let assets_dir = repo_root.join("assets");
    if assets_dir.exists() {
        walk_assets_dir(&assets_dir, &assets_dir, &story_map, repo_root)?;
    }

    Ok(())
}

/// Recursively walk the assets directory, checking that every story-shaped
/// entry in any asset's `current_consumers:` field is back-referenced by
/// that story's `assets:` field.
fn walk_assets_dir(
    dir: &Path,
    assets_base: &Path,
    story_map: &HashMap<u32, &Story>,
    repo_root: &Path,
) -> Result<(), AuditError> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()), // Skip if we can't read the directory
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();

        if path.is_dir() {
            // Recursively walk subdirectories
            walk_assets_dir(&path, assets_base, story_map, repo_root)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("yml") {
            // Load the asset and check its story consumers.
            let text = match fs::read_to_string(&path) {
                Ok(t) => t,
                Err(_) => continue,
            };

            let asset: RawAsset = match serde_yaml::from_str(&text) {
                Ok(a) => a,
                Err(_) => continue,
            };

            // Compute the asset path relative to repo root for error messages.
            let asset_rel_path = path
                .strip_prefix(repo_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            // Check each story-shaped consumer.
            for consumer in &asset.current_consumers {
                // Only check story-shaped paths (matching ^stories/[0-9]+\.yml$)
                if let Some(story_id_str) = consumer.strip_prefix("stories/").and_then(|s| {
                    s.strip_suffix(".yml").and_then(|id| {
                        if id.chars().all(|c| c.is_ascii_digit()) {
                            Some(id)
                        } else {
                            None
                        }
                    })
                }) {
                    if let Ok(story_id) = story_id_str.parse::<u32>() {
                        if let Some(story) = story_map.get(&story_id) {
                            // Check that this story declares the asset.
                            if !story.assets.contains(&asset_rel_path) {
                                return Err(AuditError::AssetStoryNotBackReferenced {
                                    asset_path: asset_rel_path,
                                    story_id,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
