// rustc 1.95.0 ICEs in the dead_code lint pass on this crate
// (check_mod_deathness). Remove once we move off 1.95 or the ICE is fixed.
#![allow(dead_code)]

//! # agentic-dashboard
//!
//! Renders the health status of stories in a dashboard format.
//!
//! The dashboard reads story YAML files and evidence from `agentic-store`
//! (test_runs and uat_signings) to classify each story as one of five
//! statuses:
//! - `proposed`: YAML says `status: proposed` (YAML wins regardless of evidence).
//! - `under_construction`: YAML says `status: under_construction` AND no
//!   historical `uat_signings.verdict=pass` exists.
//! - `healthy`: YAML says `status: healthy` AND latest
//!   `uat_signings.verdict=pass` commit equals HEAD AND latest
//!   `test_runs.verdict=pass`.
//! - `unhealthy`: any historical `uat_signings.verdict=pass` exists AND
//!   (latest `test_runs.verdict=fail` OR latest UAT commit != HEAD).
//! - `error`: YAML parse error, schema violation, or status-evidence mismatch.
//!
//! The dashboard can render the results as a formatted table (default) or as
//! JSON (with full SHAs and RFC3339 timestamps).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use agentic_store::Store;
use agentic_story::{Status, StoryError};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

/// The main dashboard type that reads stories and evidence, computes health,
/// and renders in table or JSON format.
pub struct Dashboard {
    store: Arc<dyn Store>,
    stories_dir: PathBuf,
    head_sha: String,
    repo_root: Option<PathBuf>,
}

/// Error type for dashboard operations.
#[derive(Debug)]
pub enum DashboardError {
    /// Store operation failed.
    StoreError(String),
    /// Stories directory does not exist.
    StoriesNotFound { path: PathBuf },
    /// Unknown story id in selector or drilldown.
    UnknownStory { id: u32 },
    /// Cycle detected in depends_on graph.
    Cycle { edge: String },
}

impl std::fmt::Display for DashboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DashboardError::StoreError(msg) => write!(f, "store error: {msg}"),
            DashboardError::StoriesNotFound { path } => {
                write!(f, "stories directory not found: {}", path.display())
            }
            DashboardError::UnknownStory { id } => {
                write!(f, "unknown story id: {id}")
            }
            DashboardError::Cycle { edge } => {
                write!(f, "cycle detected in depends_on graph: {edge}")
            }
        }
    }
}

impl std::error::Error for DashboardError {}

/// Internal representation of a story's health status and evidence.
#[derive(Debug, Clone)]
struct StoryHealth {
    id: u32,
    title: String,
    health: Health,
    failing_tests: Vec<String>,
    uat_commit: Option<String>,
    uat_signed_at: Option<String>,
    test_run_commit: Option<String>,
    test_run_at: Option<String>,
    parse_error: Option<String>,
    stale_related_files: Vec<String>,
    not_healthy_reason: Vec<String>,
    depends_on: Vec<u32>,
    lvl: i32,
    immediate_downstreams: Vec<u32>,
    blocks_total: u32,
    status: Status,
    superseded_by: Option<u32>,
}

/// The five health statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Health {
    Proposed,
    UnderConstruction,
    Healthy,
    Unhealthy,
    Error,
}

/// Return type for classify_health function.
type HealthClassification = (
    Health,
    Vec<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
);

impl Health {
    fn as_str(self) -> &'static str {
        match self {
            Health::Proposed => "proposed",
            Health::UnderConstruction => "under_construction",
            Health::Healthy => "healthy",
            Health::Unhealthy => "unhealthy",
            Health::Error => "error",
        }
    }
}

/// The view type for rendering.
#[derive(Debug, Clone, Copy)]
enum ViewType {
    Frontier,
    Expand,
    All,
    Ancestors,
    Descendants,
    Subtree,
    Drilldown,
}

impl ViewType {
    fn as_str(self) -> &'static str {
        match self {
            ViewType::Frontier => "frontier",
            ViewType::Expand => "expand",
            ViewType::All => "all",
            ViewType::Ancestors => "ancestors",
            ViewType::Descendants => "descendants",
            ViewType::Subtree => "subtree",
            ViewType::Drilldown => "drilldown",
        }
    }
}

/// Selector type for positional arguments.
#[derive(Debug, Clone)]
enum Selector {
    Drilldown(u32),
    Ancestors(u32),
    Descendants(u32),
    Subtree(u32),
}

impl Dashboard {
    /// Construct a new Dashboard.
    ///
    /// # Arguments
    /// - `store`: the evidence store (test_runs, uat_signings)
    /// - `stories_dir`: the root directory containing story YAML files
    /// - `head_sha`: the current git HEAD SHA (40-char hex)
    pub fn new(store: Arc<dyn Store>, stories_dir: PathBuf, head_sha: String) -> Self {
        Self {
            store,
            stories_dir,
            head_sha,
            repo_root: None,
        }
    }

    /// Construct a new Dashboard with repo-aware file-intersection checking.
    ///
    /// # Arguments
    /// - `store`: the evidence store (test_runs, uat_signings)
    /// - `stories_dir`: the root directory containing story YAML files
    /// - `repo_root`: the root directory of the git repository
    ///
    /// When `repo_root` is provided, the classifier uses file-intersection
    /// semantics for stories with `related_files` declared. Without it,
    /// falls back to the legacy strict HEAD-equality rule.
    pub fn with_repo(store: Arc<dyn Store>, stories_dir: PathBuf, repo_root: PathBuf) -> Self {
        let repo =
            git2::Repository::open(&repo_root).expect("repo_root must be a valid git repository");
        let head_sha = repo
            .head()
            .expect("repo must have a HEAD")
            .peel_to_commit()
            .expect("HEAD must be a commit")
            .id()
            .to_string();

        Self {
            store,
            stories_dir,
            head_sha,
            repo_root: Some(repo_root),
        }
    }

    /// Render the dashboard as a formatted table (all stories, backward compatible with story 3).
    pub fn render_table(&self) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        Ok(self.format_table(&stories))
    }

    /// Render the dashboard as JSON (all stories, backward compatible with story 3).
    pub fn render_json(&self) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        let json = self.format_json_with_all(&stories, &stories, ViewType::All);
        Ok(json)
    }

    /// Render the dashboard as a formatted table with frontier filter (story 10).
    pub fn render_frontier_table(&self) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        let frontier = self.filter_frontier(&stories);
        Ok(self.format_dag_table(&frontier))
    }

    /// Render the dashboard as JSON with frontier filter (story 10).
    pub fn render_frontier_json(&self) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        let frontier = self.filter_frontier(&stories);
        let json = self.format_json_with_all(&frontier, &stories, ViewType::Frontier);
        Ok(json)
    }

    /// Render the dashboard as a formatted table with canopy lens (story 3 amendment).
    pub fn render_canopy_table(&self) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        Ok(self.format_dag_table(&stories))
    }

    /// Render the dashboard as JSON with canopy lens (story 3 amendment).
    pub fn render_canopy_json(&self) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        let json = self.format_json_with_all(&stories, &stories, ViewType::All);
        Ok(json)
    }

    /// Render the dashboard with --all flag (flat list, equivalent to render_table).
    pub fn render_all_table(&self) -> Result<String, DashboardError> {
        self.render_table()
    }

    /// Render the dashboard with --all flag (flat list, equivalent to render_json).
    pub fn render_all_json(&self) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        let json = self.format_json_with_all(&stories, &stories, ViewType::All);
        Ok(json)
    }

    /// Render the dashboard with --expand flag (full not-healthy subtree).
    pub fn render_expand_table(&self) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        let expanded = self.filter_expand(&stories);
        Ok(self.format_dag_table(&expanded))
    }

    /// Render the dashboard with --expand flag (full not-healthy subtree).
    pub fn render_expand_json(&self) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        let expanded = self.filter_expand(&stories);
        let json = self.format_json_with_all(&expanded, &stories, ViewType::Expand);
        Ok(json)
    }

    /// List stories matching a selector (ancestors, descendants, or subtree).
    pub fn list_selector(&self, selector: &str) -> Result<String, DashboardError> {
        let parsed = self.parse_selector(selector)?;
        let stories = self.load_and_compute_health()?;

        let result_ids = match parsed {
            Selector::Ancestors(id) => self.get_ancestors(&stories, id)?,
            Selector::Descendants(id) => self.get_descendants(&stories, id)?,
            Selector::Subtree(id) => self.get_subtree(&stories, id)?,
            Selector::Drilldown(_) => {
                // Should not reach here from list_selector; handled separately
                return Err(DashboardError::UnknownStory { id: 0 });
            }
        };

        let view_type = match parsed {
            Selector::Ancestors(_) => ViewType::Ancestors,
            Selector::Descendants(_) => ViewType::Descendants,
            Selector::Subtree(_) => ViewType::Subtree,
            Selector::Drilldown(_) => ViewType::Drilldown,
        };

        let filtered: Vec<StoryHealth> = stories
            .clone()
            .into_iter()
            .filter(|s| result_ids.contains(&s.id))
            .collect();

        let json = self.format_json_with_all(&filtered, &stories, view_type);
        Ok(json)
    }

    /// Parse a selector string into Selector enum.
    fn parse_selector(&self, selector: &str) -> Result<Selector, DashboardError> {
        if selector.contains('+') {
            if selector.starts_with('+') && selector.ends_with('+') {
                // +<id>+
                if let Some(id_str) = selector.strip_prefix('+').and_then(|s| s.strip_suffix('+')) {
                    let id: u32 = id_str
                        .parse()
                        .map_err(|_| DashboardError::UnknownStory { id: 0 })?;
                    Ok(Selector::Subtree(id))
                } else {
                    Err(DashboardError::UnknownStory { id: 0 })
                }
            } else if let Some(id_str) = selector.strip_prefix('+') {
                // +<id>
                let id: u32 = id_str
                    .parse()
                    .map_err(|_| DashboardError::UnknownStory { id: 0 })?;
                Ok(Selector::Ancestors(id))
            } else if let Some(id_str) = selector.strip_suffix('+') {
                // <id>+
                let id: u32 = id_str
                    .parse()
                    .map_err(|_| DashboardError::UnknownStory { id: 0 })?;
                Ok(Selector::Descendants(id))
            } else {
                Err(DashboardError::UnknownStory { id: 0 })
            }
        } else {
            // Bareword drilldown
            let id: u32 = selector
                .parse()
                .map_err(|_| DashboardError::UnknownStory { id: 0 })?;
            Ok(Selector::Drilldown(id))
        }
    }

    /// Topologically sort a set of story ids using Kahn's algorithm.
    /// Orders dependencies before dependents; ties broken by ascending id.
    fn topological_sort(&self, stories: &[StoryHealth], ids: Vec<u32>) -> Vec<u32> {
        if ids.is_empty() {
            return vec![];
        }

        // Build dependency graph for the selected ids only
        let mut in_degree: HashMap<u32, usize> = HashMap::new();
        let mut graph: HashMap<u32, Vec<u32>> = HashMap::new();

        for &id in &ids {
            in_degree.insert(id, 0);
            graph.insert(id, vec![]);
        }

        // Count in-degrees and build adjacency list within the subset
        for &id in &ids {
            if let Some(story) = stories.iter().find(|s| s.id == id) {
                for &dep_id in &story.depends_on {
                    if ids.contains(&dep_id) {
                        // dep_id is a dependency of id, so add edge dep_id -> id
                        graph.entry(dep_id).or_insert_with(Vec::new).push(id);
                        *in_degree.get_mut(&id).unwrap() += 1;
                    }
                }
            }
        }

        // Kahn's algorithm: collect nodes with in_degree 0
        let mut queue: Vec<u32> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(&id, _)| id)
            .collect();
        queue.sort(); // Sort for determinism

        let mut result = vec![];
        while !queue.is_empty() {
            queue.sort(); // Keep sorted for stable tie-breaking
            let node = queue.remove(0);
            result.push(node);
            if let Some(neighbors) = graph.get(&node) {
                for &neighbor in neighbors {
                    let new_degree = in_degree[&neighbor] - 1;
                    in_degree.insert(neighbor, new_degree);
                    if new_degree == 0 {
                        queue.push(neighbor);
                    }
                }
            }
        }

        result
    }

    /// Get transitive ancestors of a story.
    fn get_ancestors(
        &self,
        stories: &[StoryHealth],
        target_id: u32,
    ) -> Result<Vec<u32>, DashboardError> {
        if !stories.iter().any(|s| s.id == target_id) {
            return Err(DashboardError::UnknownStory { id: target_id });
        }

        let mut result = vec![target_id];
        let mut queue = VecDeque::new();
        queue.push_back(target_id);
        let mut visited = HashSet::new();
        visited.insert(target_id);

        while let Some(current_id) = queue.pop_front() {
            if let Some(current) = stories.iter().find(|s| s.id == current_id) {
                for &ancestor_id in &current.depends_on {
                    if !visited.contains(&ancestor_id) {
                        visited.insert(ancestor_id);
                        queue.push_back(ancestor_id);
                        result.push(ancestor_id);
                    }
                }
            }
        }

        // Topologically sort ancestors using Kahn's algorithm
        result = self.topological_sort(stories, result);

        Ok(result)
    }

    /// Get transitive descendants of a story.
    fn get_descendants(
        &self,
        stories: &[StoryHealth],
        target_id: u32,
    ) -> Result<Vec<u32>, DashboardError> {
        if !stories.iter().any(|s| s.id == target_id) {
            return Err(DashboardError::UnknownStory { id: target_id });
        }

        let mut result = vec![target_id];
        let mut queue = VecDeque::new();
        queue.push_back(target_id);
        let mut visited = HashSet::new();
        visited.insert(target_id);

        // Build reverse dependency map
        let mut dependents: HashMap<u32, Vec<u32>> = HashMap::new();
        for story in stories {
            for &dep_id in &story.depends_on {
                dependents.entry(dep_id).or_default().push(story.id);
            }
        }

        while let Some(current_id) = queue.pop_front() {
            if let Some(descendant_ids) = dependents.get(&current_id) {
                for &desc_id in descendant_ids {
                    if !visited.contains(&desc_id) {
                        visited.insert(desc_id);
                        queue.push_back(desc_id);
                        result.push(desc_id);
                    }
                }
            }
        }

        // Topologically sort descendants using Kahn's algorithm
        result = self.topological_sort(stories, result);

        Ok(result)
    }

    /// Get target plus both ancestors and descendants (subtree).
    fn get_subtree(
        &self,
        stories: &[StoryHealth],
        target_id: u32,
    ) -> Result<Vec<u32>, DashboardError> {
        if !stories.iter().any(|s| s.id == target_id) {
            return Err(DashboardError::UnknownStory { id: target_id });
        }

        let ancestors = self.get_ancestors(stories, target_id)?;
        let descendants = self.get_descendants(stories, target_id)?;

        let mut result = ancestors;
        for id in descendants {
            if !result.contains(&id) {
                result.push(id);
            }
        }

        // Final topological sort using Kahn's algorithm
        result = self.topological_sort(stories, result);

        Ok(result)
    }

    /// Return the drill-down view for a single story by id.
    pub fn drilldown(&self, story_id: u32) -> Result<String, DashboardError> {
        let stories = self.load_and_compute_health()?;
        let story = stories
            .iter()
            .find(|s| s.id == story_id)
            .ok_or(DashboardError::UnknownStory { id: story_id })?;

        Ok(self.format_drilldown(story, &stories))
    }

    /// Filter to frontier view: all stories on the active frontier, excluding retired.
    /// The frontier includes:
    /// - All healthy stories
    /// - Non-healthy stories with no non-healthy ancestors (no frontier blocking)
    /// But excludes all retired stories (which are off-tree).
    fn filter_frontier(&self, stories: &[StoryHealth]) -> Vec<StoryHealth> {
        stories
            .iter()
            .filter(|story| {
                // Exclude retired stories from frontier view
                if story.status == Status::Retired {
                    return false;
                }

                // Healthy stories are always on the frontier
                if story.health == Health::Healthy {
                    return true;
                }

                // Non-healthy stories are on frontier only if all ancestors are healthy
                for &ancestor_id in &story.depends_on {
                    if let Some(ancestor) = stories.iter().find(|s| s.id == ancestor_id) {
                        if ancestor.health != Health::Healthy {
                            return false;
                        }
                    }
                }

                true
            })
            .cloned()
            .collect()
    }

    /// Filter to expand view: all not-healthy stories (no frontier restriction).
    fn filter_expand(&self, stories: &[StoryHealth]) -> Vec<StoryHealth> {
        stories
            .iter()
            .filter(|story| story.health != Health::Healthy)
            .cloned()
            .collect()
    }

    /// Compute the era head for a story by walking the superseded_by chain
    /// to its terminus. Returns the id of the terminal story (the one with
    /// no superseded_by pointing further). For stories not retired or not
    /// superseded, returns the story's own id.
    fn compute_era_head(&self, story_id: u32, stories: &[StoryHealth]) -> u32 {
        let mut current_id = story_id;
        let mut visited = HashSet::new();

        loop {
            if !visited.insert(current_id) {
                // Cycle detected (shouldn't happen if loader validated, but defend)
                return current_id;
            }

            if let Some(story) = stories.iter().find(|s| s.id == current_id) {
                if let Some(next_id) = story.superseded_by {
                    current_id = next_id;
                } else {
                    // No successor, this is the era head
                    return current_id;
                }
            } else {
                // Story not found, return current
                return current_id;
            }
        }
    }

    /// Load all story YAML files, compute health for each, and return sorted
    /// by health (error first, then unhealthy, under_construction, proposed,
    /// healthy), with ties broken by ascending story_id.
    fn load_and_compute_health(&self) -> Result<Vec<StoryHealth>, DashboardError> {
        if !self.stories_dir.exists() {
            return Err(DashboardError::StoriesNotFound {
                path: self.stories_dir.clone(),
            });
        }

        let mut stories = Vec::new();
        let mut depends_map: HashMap<u32, Vec<u32>> = HashMap::new();

        // Read all .yml files from the stories directory.
        let entries = std::fs::read_dir(&self.stories_dir)
            .map_err(|e| DashboardError::StoreError(e.to_string()))?;

        for entry in entries {
            let entry = entry.map_err(|e| DashboardError::StoreError(e.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yml") {
                let health = self.compute_health_for_file(&path);
                depends_map.insert(health.id, health.depends_on.clone());
                stories.push(health);
            }
        }

        // Check for cycles in depends_on
        self.detect_cycles(&depends_map)?;

        // Compute lvl and immediate downstreams for each story
        let mut downstreams_map: HashMap<u32, Vec<u32>> = HashMap::new();
        for (id, deps) in &depends_map {
            for &dep_id in deps {
                downstreams_map.entry(dep_id).or_default().push(*id);
            }
        }

        let lvl_map = self.compute_lvl(&depends_map, &downstreams_map);

        // Compute ancestor offenders for each story (story 13)
        let ancestor_offenders_map = self.compute_ancestor_offenders(&stories, &depends_map);

        for story in &mut stories {
            story.lvl = *lvl_map.get(&story.id).unwrap_or(&0);
            story.immediate_downstreams = downstreams_map.get(&story.id).unwrap_or(&vec![]).clone();
            story.immediate_downstreams.sort();

            // Compute blocks_total (transitive descendant count)
            story.blocks_total = self.count_transitive_descendants(&downstreams_map, story.id);

            // Story 13: apply ancestor inheritance rule
            // If a story's own classification is healthy but has a transitive offender,
            // flip it to unhealthy
            if story.health == Health::Healthy && ancestor_offenders_map.contains_key(&story.id) {
                // This story has an ancestor that's not healthy, so it becomes unhealthy
                story.health = Health::Unhealthy;
            }

            // Compute not_healthy_reason based on ancestor offenders
            if story.health == Health::Unhealthy {
                let mut reasons = Vec::new();

                // Add own_tests if test_runs.verdict == fail (check failing_tests length)
                // We need to distinguish between "actual failing tests" and "error strings"
                let has_own_tests_fail = !story.failing_tests.is_empty()
                    && story
                        .failing_tests
                        .iter()
                        .any(|t| !t.starts_with("schema") && t != "status-evidence mismatch");

                if has_own_tests_fail {
                    reasons.push("own_tests".to_string());
                }

                // Add own_files if stale_related_files is non-empty
                if !story.stale_related_files.is_empty() {
                    reasons.push("own_files".to_string());
                }

                // Add ancestor offenders (already in ascending order from compute_ancestor_offenders)
                if let Some(offenders) = ancestor_offenders_map.get(&story.id) {
                    for &offender_id in offenders {
                        reasons.push(format!("ancestor:{offender_id}"));
                    }
                }

                story.not_healthy_reason = reasons;
            }
        }

        // Sort by lvl ascending (most-negative first), then by id ascending
        stories.sort_by(|a, b| match a.lvl.cmp(&b.lvl) {
            Ordering::Equal => a.id.cmp(&b.id),
            other => other,
        });

        Ok(stories)
    }

    /// Detect cycles in the depends_on graph.
    fn detect_cycles(&self, depends_map: &HashMap<u32, Vec<u32>>) -> Result<(), DashboardError> {
        // Simple DFS-based cycle detection
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for &id in depends_map.keys() {
            if !visited.contains(&id)
                && self.has_cycle_dfs(id, depends_map, &mut visited, &mut rec_stack)?
            {
                return Err(DashboardError::Cycle {
                    edge: format!("cycle involving story {id}"),
                });
            }
        }

        Ok(())
    }

    /// DFS helper for cycle detection.
    fn has_cycle_dfs(
        &self,
        node: u32,
        depends_map: &HashMap<u32, Vec<u32>>,
        visited: &mut HashSet<u32>,
        rec_stack: &mut HashSet<u32>,
    ) -> Result<bool, DashboardError> {
        visited.insert(node);
        rec_stack.insert(node);

        if let Some(deps) = depends_map.get(&node) {
            for &dep in deps {
                if !visited.contains(&dep) {
                    if self.has_cycle_dfs(dep, depends_map, visited, rec_stack)? {
                        return Ok(true);
                    }
                } else if rec_stack.contains(&dep) {
                    return Ok(true);
                }
            }
        }

        rec_stack.remove(&node);
        Ok(false)
    }

    /// Compute lvl for all stories: longest path from node to any leaf (negated).
    fn compute_lvl(
        &self,
        depends_map: &HashMap<u32, Vec<u32>>,
        downstreams_map: &HashMap<u32, Vec<u32>>,
    ) -> HashMap<u32, i32> {
        let mut lvl_cache = HashMap::new();

        // Get all story ids
        let mut all_ids: HashSet<u32> = depends_map.keys().copied().collect();
        for downstreams in downstreams_map.values() {
            all_ids.extend(downstreams);
        }

        // Compute lvl for each id
        for &id in &all_ids {
            self.compute_lvl_recursive(id, depends_map, downstreams_map, &mut lvl_cache);
        }

        lvl_cache
    }

    /// Recursive helper for lvl computation.
    fn compute_lvl_recursive(
        &self,
        id: u32,
        depends_map: &HashMap<u32, Vec<u32>>,
        downstreams_map: &HashMap<u32, Vec<u32>>,
        cache: &mut HashMap<u32, i32>,
    ) -> i32 {
        if let Some(&cached) = cache.get(&id) {
            return cached;
        }

        // If no downstreams, it's a leaf
        if !downstreams_map.contains_key(&id) {
            cache.insert(id, 0);
            return 0;
        }

        // Find longest path among immediate downstreams.
        // Compute the minimum (most-negative) lvl among descendants,
        // which represents the longest path to any leaf.
        let min_descendant_lvl = downstreams_map
            .get(&id)
            .map(|downstreams| {
                downstreams
                    .iter()
                    .map(|&desc_id| {
                        self.compute_lvl_recursive(desc_id, depends_map, downstreams_map, cache)
                    })
                    .min()
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        // lvl = -1 - min_descendant_lvl
        // e.g., if min_descendant_lvl is -3 (deepest child), lvl is -1 - (-3) = 2... wait that's still wrong
        // Actually we want: if min_descendant_lvl is -3, we should get -4
        // So the formula is: lvl = min_descendant_lvl - 1
        let lvl = min_descendant_lvl - 1;
        cache.insert(id, lvl);
        lvl
    }

    /// Count transitive descendants of a story.
    fn count_transitive_descendants(
        &self,
        downstreams_map: &HashMap<u32, Vec<u32>>,
        id: u32,
    ) -> u32 {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(id);
        visited.insert(id);

        while let Some(current) = queue.pop_front() {
            if let Some(downstreams) = downstreams_map.get(&current) {
                for &desc_id in downstreams {
                    if !visited.contains(&desc_id) {
                        visited.insert(desc_id);
                        queue.push_back(desc_id);
                    }
                }
            }
        }

        // Don't count the id itself
        (visited.len() - 1) as u32
    }

    /// Compute which direct ancestors are offending (not-healthy) for each story.
    /// Returns a map from story id to list of offending ancestor ids (sorted ascending).
    fn compute_ancestor_offenders(
        &self,
        stories: &[StoryHealth],
        depends_map: &HashMap<u32, Vec<u32>>,
    ) -> HashMap<u32, Vec<u32>> {
        let mut result: HashMap<u32, Vec<u32>> = HashMap::new();

        // For each story, find its direct offending ancestors
        for story in stories {
            let mut offenders = Vec::new();

            // Check each direct ancestor
            for &ancestor_id in &story.depends_on {
                // Find the ancestor's classification
                if let Some(ancestor) = stories.iter().find(|s| s.id == ancestor_id) {
                    // An ancestor is offending if it's not healthy
                    if ancestor.health != Health::Healthy {
                        offenders.push(ancestor_id);
                    }
                }
            }

            // Check transitive ancestors
            let has_transitive_offender =
                self.has_transitive_offender(story.id, stories, depends_map);

            if !offenders.is_empty() || has_transitive_offender {
                // Sort by ascending id for determinism
                offenders.sort();
                result.insert(story.id, offenders);
            }
        }

        result
    }

    /// Check if there's any transitive (non-direct) ancestor that's not healthy.
    /// This is needed to classify the story as unhealthy even if direct ancestors are healthy.
    fn has_transitive_offender(
        &self,
        story_id: u32,
        stories: &[StoryHealth],
        depends_map: &HashMap<u32, Vec<u32>>,
    ) -> bool {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(story_id);
        visited.insert(story_id);

        while let Some(current_id) = queue.pop_front() {
            if let Some(deps) = depends_map.get(&current_id) {
                for &ancestor_id in deps {
                    if !visited.contains(&ancestor_id) {
                        visited.insert(ancestor_id);

                        // Check if this ancestor is offending
                        if let Some(ancestor) = stories.iter().find(|s| s.id == ancestor_id) {
                            if ancestor.health != Health::Healthy {
                                // Skip direct ancestors (they're already in depends_map)
                                // Actually, we need to check ALL transitive ancestors
                                return true;
                            }
                        }

                        // Continue walking up
                        queue.push_back(ancestor_id);
                    }
                }
            }
        }

        false
    }

    /// Compute the health status for a single story file.
    fn compute_health_for_file(&self, path: &std::path::Path) -> StoryHealth {
        // Extract the id from the filename (e.g. "123.yml" -> 123).
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        // Try to load the story YAML.
        match agentic_story::Story::load(path) {
            Ok(story) => {
                let title = story.title.clone();
                let status = story.status;
                let related_files = story.related_files.clone();
                let depends_on = story.depends_on.clone();
                let superseded_by = story.superseded_by;

                // Load evidence from the store.
                let test_run = self.get_test_run(id);
                let uat_signings = self.get_uat_signings(id);

                // Compute health classification.
                let (
                    health,
                    failing_tests,
                    uat_commit,
                    uat_signed_at,
                    test_run_commit,
                    test_run_at,
                    stale_related_files,
                    not_healthy_reason,
                ) = self.classify_health(status, &test_run, &uat_signings, &related_files);

                StoryHealth {
                    id,
                    title,
                    health,
                    failing_tests,
                    uat_commit,
                    uat_signed_at,
                    test_run_commit,
                    test_run_at,
                    parse_error: None,
                    stale_related_files,
                    not_healthy_reason,
                    depends_on,
                    lvl: 0,
                    immediate_downstreams: vec![],
                    blocks_total: 0,
                    status,
                    superseded_by,
                }
            }
            Err(e) => {
                // Parse error: render as error row.
                let error_reason = match &e {
                    StoryError::YamlParse { .. } => "yaml parse",
                    StoryError::SchemaViolation { field, .. } => {
                        if field.contains("status") {
                            "schema: status"
                        } else {
                            "schema"
                        }
                    }
                    StoryError::UnknownStatus { .. } => "schema: status",
                    _ => "error",
                };

                StoryHealth {
                    id,
                    title: "<parse error>".to_string(),
                    health: Health::Error,
                    failing_tests: vec![error_reason.to_string()],
                    uat_commit: None,
                    uat_signed_at: None,
                    test_run_commit: None,
                    test_run_at: None,
                    parse_error: Some(e.to_string()),
                    stale_related_files: vec![],
                    not_healthy_reason: vec![],
                    depends_on: vec![],
                    lvl: 0,
                    immediate_downstreams: vec![],
                    blocks_total: 0,
                    status: Status::Proposed,
                    superseded_by: None,
                }
            }
        }
    }

    /// Retrieve the latest test_run row for a story from the store.
    fn get_test_run(&self, story_id: u32) -> Option<Value> {
        self.store
            .get("test_runs", &story_id.to_string())
            .ok()
            .flatten()
    }

    /// Retrieve all uat_signings rows for a story from the store.
    fn get_uat_signings(&self, story_id: u32) -> Vec<Value> {
        self.store
            .query("uat_signings", &|row| {
                row.get("story_id")
                    .and_then(|v| v.as_u64())
                    .map(|id| id == story_id as u64)
                    .unwrap_or(false)
            })
            .unwrap_or_default()
    }

    /// Compute the diff between two commits and return the list of changed
    /// file paths relative to the repo root.
    fn compute_git_diff(&self, from_commit: &str, to_commit: &str) -> Result<Vec<String>, String> {
        let repo_root = self
            .repo_root
            .as_ref()
            .ok_or_else(|| "repo_root not available for git diff computation".to_string())?;

        let repo =
            git2::Repository::open(repo_root).map_err(|e| format!("failed to open repo: {e}"))?;

        // Parse the OID strings
        let from_oid = git2::Oid::from_str(from_commit)
            .map_err(|e| format!("invalid from commit OID: {e}"))?;
        let to_oid =
            git2::Oid::from_str(to_commit).map_err(|e| format!("invalid to commit OID: {e}"))?;

        // Get the trees
        let from_tree = repo
            .find_tree(
                repo.find_commit(from_oid)
                    .map_err(|e| format!("failed to find from commit: {e}"))?
                    .tree_id(),
            )
            .map_err(|e| format!("failed to find from tree: {e}"))?;

        let to_tree = repo
            .find_tree(
                repo.find_commit(to_oid)
                    .map_err(|e| format!("failed to find to commit: {e}"))?
                    .tree_id(),
            )
            .map_err(|e| format!("failed to find to tree: {e}"))?;

        // Compute the diff
        let diff = repo
            .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), None)
            .map_err(|e| format!("failed to compute diff: {e}"))?;

        // Extract changed file paths
        let mut changed_files = Vec::new();
        diff.foreach(
            &mut |delta, _progress| {
                if let Some(path) = delta.new_file().path() {
                    if let Some(path_str) = path.to_str() {
                        changed_files.push(path_str.to_string());
                    }
                }
                true
            },
            None,
            None,
            None,
        )
        .map_err(|e| format!("failed to iterate diff: {e}"))?;

        Ok(changed_files)
    }

    /// Check if any of the glob patterns in related_files match any of the
    /// changed files. Returns a list of matched paths if there is an
    /// intersection, empty vec otherwise.
    fn check_related_files_intersection(
        &self,
        related_files: &[String],
        changed_files: &[String],
    ) -> Vec<String> {
        if related_files.is_empty() {
            return vec![];
        }

        // Build a globset from related_files patterns
        let mut set_builder = globset::GlobSetBuilder::new();

        for pattern in related_files {
            if let Ok(glob) = globset::Glob::new(pattern) {
                set_builder.add(glob);
            }
        }

        let globset = set_builder.build().unwrap_or_else(|_| {
            // If globset construction fails, fall back to empty set (no matches)
            globset::GlobSetBuilder::new()
                .build()
                .expect("empty globset must build")
        });

        // Find all changed files that match any pattern
        let mut matched = Vec::new();
        for changed_file in changed_files {
            if globset.is_match(changed_file) {
                matched.push(changed_file.clone());
            }
        }

        matched
    }

    /// Classify a story's health based on YAML status and evidence.
    fn classify_health(
        &self,
        yaml_status: Status,
        test_run: &Option<Value>,
        uat_signings: &[Value],
        related_files: &[String],
    ) -> HealthClassification {
        // Extract the latest UAT signing if any.
        let latest_uat = uat_signings.last();
        let latest_uat_pass = uat_signings.iter().rev().find(|row| {
            row.get("verdict")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase() == "pass")
                .unwrap_or(false)
        });

        // Rule 1: proposed ⇔ YAML says proposed (YAML wins).
        if yaml_status == Status::Proposed {
            return (
                Health::Proposed,
                vec![],
                None,
                None,
                None,
                None,
                vec![],
                vec![],
            );
        }

        // Rule 2: under_construction ⇔ YAML says under_construction AND no
        // historical UAT pass.
        if yaml_status == Status::UnderConstruction && latest_uat_pass.is_none() {
            let failing_tests = test_run
                .as_ref()
                .and_then(|row| row.get("verdict"))
                .and_then(|v| v.as_str())
                .filter(|s| s.to_lowercase() == "fail")
                .and(test_run.as_ref())
                .and_then(|row| row.get("failing_tests"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            return (
                Health::UnderConstruction,
                failing_tests,
                None,
                None,
                None,
                None,
                vec![],
                vec![],
            );
        }

        // Rule 3: healthy ⇔ latest UAT pass exists AND latest test_run pass
        // AND healthy check passes (based on repo_root availability).
        // - If repo_root: check for related_files file intersection (story 9).
        // - If no repo_root: check for strict HEAD equality (legacy story 3).
        if let Some(uat_row) = latest_uat_pass {
            let uat_commit = uat_row
                .get("commit")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let uat_signed_at = uat_row
                .get("signed_at")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let test_run_pass = test_run
                .as_ref()
                .and_then(|row| row.get("verdict"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase() == "pass")
                .unwrap_or(true); // absence is not failure

            let test_run_commit = test_run
                .as_ref()
                .and_then(|row| row.get("commit"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let test_run_at = test_run
                .as_ref()
                .and_then(|row| row.get("ran_at"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if test_run_pass {
                // Determine if the story is healthy based on repo availability.
                let is_healthy = if self.repo_root.is_some() {
                    // Story 9 logic: if related_files is non-empty, check for
                    // intersection. If empty, be permissive (don't apply strict
                    // equality rule).
                    if !related_files.is_empty() {
                        if let Some(uat_sha) = uat_commit.as_deref() {
                            match self.compute_git_diff(uat_sha, &self.head_sha) {
                                Ok(changed_files) => {
                                    let stale_files = self.check_related_files_intersection(
                                        related_files,
                                        &changed_files,
                                    );
                                    stale_files.is_empty()
                                }
                                Err(_) => {
                                    // If diff computation fails, be permissive
                                    true
                                }
                            }
                        } else {
                            false
                        }
                    } else {
                        // No repo_root, empty related_files → permissive
                        true
                    }
                } else {
                    // Legacy (story 3): no repo_root → strict HEAD equality
                    uat_commit.as_deref() == Some(self.head_sha.as_str())
                };

                if is_healthy {
                    return (
                        Health::Healthy,
                        vec![],
                        uat_commit,
                        uat_signed_at,
                        test_run_commit,
                        test_run_at,
                        vec![],
                        vec![],
                    );
                }
            }
        }

        // Rule 4: unhealthy ⇔ any historical UAT pass AND
        // (latest test_run fail OR (repo-aware: related_files intersect C0..HEAD)
        // OR (legacy: latest UAT commit != HEAD AND no repo_root)).
        if latest_uat_pass.is_some() {
            let test_run_fail = test_run
                .as_ref()
                .and_then(|row| row.get("verdict"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase() == "fail")
                .unwrap_or(false);

            let uat_commit = latest_uat
                .and_then(|row| row.get("commit"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut is_unhealthy = test_run_fail;
            let mut stale_files = vec![];

            // Always check for file staleness (needed for not_healthy_reason even if tests fail)
            if !related_files.is_empty() && self.repo_root.is_some() {
                if let Some(uat_sha) = uat_commit.as_deref() {
                    match self.compute_git_diff(uat_sha, &self.head_sha) {
                        Ok(changed_files) => {
                            stale_files = self
                                .check_related_files_intersection(related_files, &changed_files);
                            // Only set is_unhealthy from file staleness if tests didn't already fail
                            if !is_unhealthy {
                                is_unhealthy = !stale_files.is_empty();
                            }
                        }
                        Err(_) => {
                            // If diff fails, be permissive
                            if !is_unhealthy {
                                is_unhealthy = false;
                            }
                        }
                    }
                }
            } else if !is_unhealthy {
                // Legacy: no repo_root or empty related_files means strict
                // equality check (UAT commit must equal HEAD).
                let uat_commit_not_head = uat_commit.as_deref() != Some(self.head_sha.as_str());
                is_unhealthy = uat_commit_not_head;
            }

            if is_unhealthy {
                let failing_tests = test_run
                    .as_ref()
                    .and_then(|row| row.get("failing_tests"))
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let uat_signed_at = latest_uat
                    .and_then(|row| row.get("signed_at"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let test_run_commit = test_run
                    .as_ref()
                    .and_then(|row| row.get("commit"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let test_run_at = test_run
                    .as_ref()
                    .and_then(|row| row.get("ran_at"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                return (
                    Health::Unhealthy,
                    failing_tests,
                    uat_commit,
                    uat_signed_at,
                    test_run_commit,
                    test_run_at,
                    stale_files,
                    vec![],
                );
            }
        }

        // Rule 5: error — status-evidence mismatch.
        // This covers cases like YAML says healthy but no UAT signing exists.
        (
            Health::Error,
            vec!["status-evidence mismatch".to_string()],
            None,
            None,
            None,
            None,
            vec![],
            vec![],
        )
    }

    /// Format stories as a table.
    fn format_table(&self, stories: &[StoryHealth]) -> String {
        let mut output = String::new();

        // Header (story 3 backward-compatible format)
        output.push_str("ID | Title | Health | Failing tests | Healthy at\n");
        output.push_str("---|-------|--------|---------------|----------\n");

        // Rows
        for story in stories {
            let id = format!("{}", story.id);
            let title = truncate_title(&story.title);
            let health = story.health.as_str();

            let failing_tests =
                if story.health == Health::Proposed || story.health == Health::Healthy {
                    String::new()
                } else {
                    story.failing_tests.join(", ")
                };

            let healthy_at = if story.health == Health::Healthy {
                let short_sha = story.uat_commit.as_ref().map(|s| &s[..7]).unwrap_or("");
                let relative_age = story
                    .uat_signed_at
                    .as_ref()
                    .and_then(|ts| format_relative_age(ts))
                    .unwrap_or_else(|| "?".to_string());
                format!("{} {}", short_sha, relative_age)
            } else {
                String::new()
            };

            output.push_str(&format!(
                "{} | {} | {} | {} | {}\n",
                id, title, health, failing_tests, healthy_at
            ));
        }

        output
    }

    /// Format stories as a table with DAG columns (story 10 frontier/expand view).
    fn format_dag_table(&self, stories: &[StoryHealth]) -> String {
        let mut output = String::new();

        // Header (story 10 DAG-aware format)
        output.push_str("ID | Title | Health | lvl | ↑ | ↓\n");
        output.push_str("---|-------|--------|-----|---|--\n");

        // Rows
        for story in stories {
            let id = format!("{}", story.id);
            let title = truncate_title(&story.title);
            let health = story.health.as_str();
            let lvl = format!("{}", story.lvl);

            // Upstream (depends_on)
            let upstream = if story.depends_on.is_empty() {
                String::new()
            } else {
                story
                    .depends_on
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            // Downstream (immediate + blocks_total)
            let downstream = if story.immediate_downstreams.is_empty() {
                String::new()
            } else {
                let ids = story
                    .immediate_downstreams
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} (blocks {})", ids, story.blocks_total)
            };

            output.push_str(&format!(
                "{} | {} | {} | {} | {} | {}\n",
                id, title, health, lvl, upstream, downstream
            ));
        }

        output
    }

    /// Format stories as JSON, with access to all stories for computing era_head_id.
    fn format_json_with_all(
        &self,
        stories: &[StoryHealth],
        all_stories: &[StoryHealth],
        view: ViewType,
    ) -> String {
        let mut story_objects = Vec::new();

        for story in stories {
            let era_head_id = self.compute_era_head(story.id, all_stories);
            let mut obj = json!({
                "id": story.id,
                "title": story.title,
                "health": story.health.as_str(),
                "era_head_id": era_head_id,
                "lvl": story.lvl,
                "upstream": story.depends_on,
                "downstream": story.immediate_downstreams,
                "blocks_total": story.blocks_total,
                "failing_tests": story.failing_tests,
                "uat_commit": story.uat_commit,
                "uat_signed_at": story.uat_signed_at,
                "test_run_commit": story.test_run_commit,
                "test_run_at": story.test_run_at,
            });

            // Include stale_related_files only if non-empty
            if !story.stale_related_files.is_empty() {
                obj["stale_related_files"] = json!(story.stale_related_files.clone());
            }

            // Include not_healthy_reason only if non-empty (story 13)
            if !story.not_healthy_reason.is_empty() {
                obj["not_healthy_reason"] = json!(story.not_healthy_reason.clone());
            }

            story_objects.push(obj);
        }

        // Compute summary counts.
        let mut counts = std::collections::HashMap::new();
        counts.insert("healthy", 0);
        counts.insert("unhealthy", 0);
        counts.insert("under_construction", 0);
        counts.insert("proposed", 0);
        counts.insert("error", 0);

        for story in stories {
            let key = story.health.as_str();
            *counts.get_mut(key).unwrap_or(&mut 0) += 1;
        }

        let summary = json!({
            "healthy": counts.get("healthy").unwrap_or(&0),
            "unhealthy": counts.get("unhealthy").unwrap_or(&0),
            "under_construction": counts.get("under_construction").unwrap_or(&0),
            "proposed": counts.get("proposed").unwrap_or(&0),
            "error": counts.get("error").unwrap_or(&0),
            "total": stories.len(),
        });

        let result = json!({
            "stories": story_objects,
            "summary": summary,
            "view": view.as_str(),
        });

        result.to_string()
    }

    /// Format the drill-down view for a single story.
    fn format_drilldown(&self, target: &StoryHealth, all_stories: &[StoryHealth]) -> String {
        let mut output = String::new();

        output.push_str(&format!("Story ID: {}\n", target.id));
        output.push_str(&format!("Title: {}\n", target.title));
        output.push_str(&format!("Health: {}\n", target.health.as_str()));

        if !target.failing_tests.is_empty() {
            output.push_str("Failing tests:\n");
            for test in &target.failing_tests {
                output.push_str(&format!("  - {}\n", test));
            }
        }

        if let Some(ref uat_commit) = target.uat_commit {
            output.push_str(&format!("Latest UAT commit: {}\n", uat_commit));
        }

        if let Some(ref uat_signed_at) = target.uat_signed_at {
            output.push_str(&format!("Latest UAT signed at: {}\n", uat_signed_at));
        }

        // Offending ancestors section (story 13)
        let offending_ancestor_ids: Vec<u32> = target
            .not_healthy_reason
            .iter()
            .filter_map(|reason| {
                if let Some(id_str) = reason.strip_prefix("ancestor:") {
                    id_str.parse::<u32>().ok()
                } else {
                    None
                }
            })
            .collect();

        if !offending_ancestor_ids.is_empty() {
            let offenders_str = offending_ancestor_ids
                .iter()
                .map(|&id| {
                    let ancestor = all_stories.iter().find(|s| s.id == id);
                    if let Some(anc) = ancestor {
                        format!("{} ({})", id, anc.health.as_str())
                    } else {
                        format!("{}", id)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!("Offending ancestors: {}\n", offenders_str));
        }

        // Ancestors section
        output.push_str("\nAncestors:\n");
        let ancestors = self
            .get_ancestors(all_stories, target.id)
            .unwrap_or_default();
        let ancestors: Vec<_> = ancestors
            .into_iter()
            .filter(|&id| id != target.id)
            .collect();
        if ancestors.is_empty() {
            output.push_str("  (none)\n");
        } else {
            for ancestor_id in ancestors {
                if let Some(ancestor) = all_stories.iter().find(|s| s.id == ancestor_id) {
                    output.push_str(&format!(
                        "  {} - {} ({})\n",
                        ancestor.id,
                        ancestor.title,
                        ancestor.health.as_str()
                    ));
                }
            }
        }

        // Descendants section
        output.push_str("\nDescendants:\n");
        let descendants = self
            .get_descendants(all_stories, target.id)
            .unwrap_or_default();
        let descendants: Vec<_> = descendants
            .into_iter()
            .filter(|&id| id != target.id)
            .collect();
        if descendants.is_empty() {
            output.push_str("  (none)\n");
        } else {
            for descendant_id in descendants {
                if let Some(descendant) = all_stories.iter().find(|s| s.id == descendant_id) {
                    output.push_str(&format!(
                        "  {} - {} ({})\n",
                        descendant.id,
                        descendant.title,
                        descendant.health.as_str()
                    ));
                }
            }
        }

        output
    }
}

/// Sort priority for health statuses: lower value comes first.
fn health_sort_priority(health: Health) -> i32 {
    match health {
        Health::Error => 0,
        Health::Unhealthy => 1,
        Health::UnderConstruction => 2,
        Health::Proposed => 3,
        Health::Healthy => 4,
    }
}

/// Truncate a title to ~35 visible characters with a single U+2026 ellipsis.
fn truncate_title(title: &str) -> String {
    const MAX_LEN: usize = 35;
    let char_count = title.chars().count();
    if char_count <= MAX_LEN {
        title.to_string()
    } else {
        // Take the first MAX_LEN characters (safe for Unicode) and append the ellipsis.
        let truncated: String = title.chars().take(MAX_LEN).collect();
        format!("{}…", truncated)
    }
}

/// Format a timestamp (RFC3339) as a relative age string.
/// Returns strings like "just now", "5m ago", "3h ago", "2d ago".
fn format_relative_age(timestamp: &str) -> Option<String> {
    // Parse the timestamp.
    let dt = DateTime::parse_from_rfc3339(timestamp).ok()?;
    let then = dt.with_timezone(&Utc);
    let now = Utc::now();

    let duration = now.signed_duration_since(then);

    if duration.num_seconds() < 1 {
        return Some("just now".to_string());
    }

    if duration.num_seconds() < 60 {
        return Some(format!("{}s ago", duration.num_seconds()));
    }

    if duration.num_minutes() < 60 {
        return Some(format!("{}m ago", duration.num_minutes()));
    }

    if duration.num_hours() < 24 {
        return Some(format!("{}h ago", duration.num_hours()));
    }

    Some(format!("{}d ago", duration.num_days()))
}

pub mod audit {
    #[derive(Debug, Clone)]
    pub struct AuditReport {
        pub implementation_without_flip: Vec<AuditEntry>,
        pub promotion_ready: Vec<AuditEntry>,
        pub test_builder_not_started: Vec<AuditEntry>,
        pub healthy_with_failing_test: Vec<AuditEntry>,
    }

    #[derive(Debug, Clone)]
    pub struct AuditEntry {
        pub id: u32,
        pub passing_tests: Vec<String>,
        pub failing_tests: Vec<String>,
    }

    impl AuditReport {
        pub fn is_empty(&self) -> bool {
            self.implementation_without_flip.is_empty()
                && self.promotion_ready.is_empty()
                && self.test_builder_not_started.is_empty()
                && self.healthy_with_failing_test.is_empty()
        }
    }

    impl std::fmt::Display for AuditReport {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            if self.is_empty() {
                writeln!(f, "No drift detected")?;
                return Ok(());
            }
            Ok(())
        }
    }

    pub fn run_audit(_sd: &std::path::Path, _s: std::sync::Arc<dyn crate::Store>, _sha: String) -> Result<AuditReport, crate::DashboardError> {
        Err(crate::DashboardError::StoreError("not implemented".into()))
    }
}
