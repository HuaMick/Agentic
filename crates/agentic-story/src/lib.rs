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
//!   3. Enum-boundary validation (`status` is one of four values).
//!   4. Graph validation (directory-load only): `depends_on` edges form
//!      a DAG — cycles and self-loops are rejected.
//!
//! The loader is strictly read-only; it never mutates state or persists
//! anything. Mutation of `status` is owned by `agentic uat` (story 1).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_yaml::Value;

/// A fully-validated story loaded from disk.
///
/// Constructing a `Story` has already enforced the schema and — when
/// loaded through [`Story::load_dir`] — the acyclicity of the
/// `depends_on` graph, so downstream code can assume well-formedness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Story {
    pub id: u32,
    pub title: String,
    pub outcome: String,
    pub status: Status,
    pub patterns: Vec<String>,
    pub acceptance: Acceptance,
    pub guidance: String,
    pub depends_on: Vec<u32>,
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

/// The four lifecycle states the dashboard understands.
///
/// Kept deliberately narrow — any other value loaded from disk becomes
/// [`StoryError::UnknownStatus`] with the offending string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Proposed,
    UnderConstruction,
    Healthy,
    Unhealthy,
}

/// Typed error surface for the loader. No raw I/O errors leak through;
/// no `anyhow::Error`.
#[derive(Debug)]
pub enum StoryError {
    /// A required field is missing, or the file is otherwise not shaped
    /// like a story. `field` names the offending field.
    SchemaViolation { field: String, message: String },
    /// `status` parsed but was not one of the four accepted values.
    UnknownStatus { value: String },
    /// The file or directory the caller supplied does not exist.
    NotFound { path: PathBuf },
    /// The YAML parser rejected the file before we could validate it.
    YamlParse { path: PathBuf, message: String },
    /// Cross-story `depends_on` validation failure.
    DependsOnCycle { participants: Vec<u32> },
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
                    "status must be one of proposed|under_construction|healthy|unhealthy; got `{value}`"
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
        }
    }
}

impl std::error::Error for StoryError {}

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
    /// collective `depends_on` graph is a DAG (no cycles, no self-loops).
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
        Ok(stories)
    }
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
    acceptance: RawAcceptance,
    guidance: String,
    #[serde(default)]
    depends_on: Vec<u32>,
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
    let raw: RawStory = serde_yaml::from_value(value).map_err(|e| StoryError::SchemaViolation {
        field: extract_field_from_err(&e).unwrap_or_else(|| "<unknown>".to_string()),
        message: e.to_string(),
    })?;

    // Third pass: enum-boundary validation on status. Custom so the
    // error carries the offending value verbatim.
    let status = match raw.status.as_str() {
        "proposed" => Status::Proposed,
        "under_construction" => Status::UnderConstruction,
        "healthy" => Status::Healthy,
        "unhealthy" => Status::Unhealthy,
        other => {
            return Err(StoryError::UnknownStatus {
                value: other.to_string(),
            });
        }
    };

    Ok(Story {
        id: raw.id,
        title: raw.title,
        outcome: raw.outcome,
        status,
        patterns: raw.patterns,
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
