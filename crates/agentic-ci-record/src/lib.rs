//! # agentic-ci-record
//!
//! Per-story test-run bookkeeping. After each acceptance-test run, the
//! system upserts a single row per story into the `test_runs` table via
//! the [`agentic_store::Store`] trait, so the dashboard ([`agentic-dashboard`],
//! story 3) can render which stories' tests are currently red without
//! doing "pick the latest timestamp" work at read time.
//!
//! See [`stories/2.yml`](../../stories/2.yml) for the full contract and
//! [`patterns/standalone-resilient-library.yml`](../../patterns/standalone-resilient-library.yml)
//! for the dependency-floor rationale.
//!
//! ## Row contract (from `stories/2.yml` guidance)
//!
//! Schemaless per ADR-0002. Upsert keyed by `story_id`. Fields:
//!
//! - `story_id`: integer, the primary key.
//! - `verdict`: `"pass"` | `"fail"` (lowercase string).
//! - `commit`: full 40-char git SHA of HEAD at record time.
//! - `ran_at`: RFC3339 UTC timestamp.
//! - `failing_tests`: array of strings. Empty `[]` on Pass; on Fail,
//!   basenames only (e.g. `"record_fail.rs"`) — no paths, no extensions
//!   stripped.
//!
//! ## Malformed-input contract
//!
//! [`Recorder::record_from_raw`] validates its byte input BEFORE opening
//! any write transaction. On any validation failure it returns
//! [`RecordError::MalformedInput`] naming the offending field and leaves
//! the store untouched. This is what makes "a flaky CI step that emitted
//! garbage does not corrupt a known-good row" a write-time invariant.
//!
//! ## Dependency floor
//!
//! Per the standalone-resilient-library pattern, this crate depends only
//! on [`agentic-store`], `serde_json`, and `git2`. No orchestrator, no
//! runtime, no sandbox, no CLI — those crates may call us, but we do not
//! call them. The `record_standalone_resilience` integration test enforces
//! this by linking only against the allowed set.

use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use agentic_store::{Store, StoreError};
use serde_json::{json, Value};

/// Name of the store table the recorder writes to.
///
/// Upsert-keyed by `story_id.to_string()`. The dashboard reads from the
/// same table name.
const TEST_RUNS_TABLE: &str = "test_runs";

/// The typed verdict an acceptance-test run yielded.
///
/// The on-disk representation is the lowercase string returned by
/// [`Verdict::as_str`]. The story's guidance pins those two string values;
/// see the Fail walkthrough and Pass walkthrough in `stories/2.yml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Every acceptance test of the story passed.
    Pass,
    /// At least one acceptance test of the story failed. The failing
    /// files are carried on [`RunInput::failing_tests`].
    Fail,
}

impl Verdict {
    /// The lowercase string the recorder writes for this verdict.
    ///
    /// This is a contract with the dashboard (story 3); changing the
    /// wire shape here breaks the dashboard's filter.
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Fail => "fail",
        }
    }
}

/// Input to [`Recorder::record`] — the typed shape an already-parsed
/// test-runner output takes before being upserted into `test_runs`.
///
/// Construct via [`RunInput::pass`] or [`RunInput::fail`]; the recorder
/// does the basename collapse on the failing-test paths at write time so
/// callers can pass full paths exactly as they come out of the test
/// runner's JSON.
#[derive(Debug, Clone)]
pub struct RunInput {
    story_id: i64,
    verdict: Verdict,
    /// On Pass, always empty. On Fail, the caller-supplied list of
    /// failing test files (may be full paths; the recorder collapses
    /// them to basenames at write time).
    failing_tests: Vec<String>,
}

impl RunInput {
    /// Build a Pass input for the given story id. `failing_tests` is
    /// forced empty per the row contract.
    pub fn pass(story_id: i64) -> Self {
        Self {
            story_id,
            verdict: Verdict::Pass,
            failing_tests: Vec::new(),
        }
    }

    /// Build a Fail input naming the test files that failed. Entries
    /// may be full paths as reported by the test runner; the recorder
    /// collapses each to its basename (extension preserved) at write
    /// time — this is what the scaffold `record_fail.rs` pins.
    pub fn fail(story_id: i64, failing_tests: Vec<String>) -> Self {
        Self {
            story_id,
            verdict: Verdict::Fail,
            failing_tests,
        }
    }

    /// Story id this run was for.
    pub fn story_id(&self) -> i64 {
        self.story_id
    }

    /// Verdict of this run.
    pub fn verdict(&self) -> Verdict {
        self.verdict
    }

    /// Failing test paths/basenames as supplied by the caller.
    pub fn failing_tests(&self) -> &[String] {
        &self.failing_tests
    }
}

/// Errors the recorder can surface.
///
/// [`RecordError::MalformedInput`] is deliberately the ONLY error a
/// caller can recognise by shape without downcasting: the corruption-
/// resistance contract (story 2, acceptance test
/// `record_malformed_input_preserves_row.rs`) says that variant must fire
/// before any write transaction opens, so matching on it is the
/// dashboard's guarantee that a failing CI step did not overwrite a
/// known-good row.
#[derive(Debug)]
#[non_exhaustive]
pub enum RecordError {
    /// The raw test-runner output failed validation. Validation is
    /// performed BEFORE any write transaction opens; this error
    /// guarantees the store was not touched.
    ///
    /// `field` names the offending field (`"input"` for unparseable
    /// bytes, `"verdict"` / `"failing_tests"` / `"story_id"` for specific
    /// field-level violations).
    MalformedInput {
        /// Name of the offending field, so a log line can be "malformed
        /// test-runner output: verdict missing" without a caller having
        /// to string-match the `Display` output.
        field: String,
        /// Human-readable description of what was wrong.
        reason: String,
    },
    /// The local git repository could not be discovered, or HEAD could
    /// not be resolved to a commit — so the recorder cannot stamp the
    /// `commit` field on the row.
    Git(String),
    /// The underlying [`Store`] failed during upsert.
    Store(StoreError),
    /// The system clock is before the UNIX epoch — the recorder cannot
    /// format an RFC3339 timestamp.
    Clock(String),
}

impl std::fmt::Display for RecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordError::MalformedInput { field, reason } => {
                write!(f, "malformed test-runner output: {field}: {reason}")
            }
            RecordError::Git(msg) => write!(f, "could not resolve git HEAD: {msg}"),
            RecordError::Store(err) => write!(f, "store error while recording: {err}"),
            RecordError::Clock(msg) => write!(f, "clock error while recording: {msg}"),
        }
    }
}

impl std::error::Error for RecordError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RecordError::Store(err) => Some(err),
            _ => None,
        }
    }
}

impl From<StoreError> for RecordError {
    fn from(err: StoreError) -> Self {
        RecordError::Store(err)
    }
}

/// Upserts per-story acceptance-test run rows into the `test_runs` table
/// on each CI run.
///
/// Construct with [`Recorder::new`] against any [`Store`]; the recorder
/// holds only the store handle and a git-repo discovery strategy. It is
/// cheap to clone (just an `Arc` bump) and safe to share across threads.
pub struct Recorder {
    store: Arc<dyn Store>,
}

impl Recorder {
    /// Wrap a [`Store`] handle in a recorder. The recorder itself holds
    /// no other state; commit and timestamp are resolved at
    /// [`Recorder::record`] / [`Recorder::record_from_raw`] time so a
    /// long-lived recorder instance is safe.
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    /// Upsert a `test_runs` row built from an already-validated
    /// [`RunInput`].
    ///
    /// This is the entry point the CI hook, a future pre-commit hook,
    /// the orchestrator, and the `record_standalone_resilience` test all
    /// share — hence the "same row shape as the CLI / CI hook path"
    /// clause of the standalone-resilience justification.
    pub fn record(&self, input: RunInput) -> Result<(), RecordError> {
        self.record_inner(input)
    }

    /// Validate a raw test-runner payload (typically JSON bytes captured
    /// off the runner's stdout) for the given `story_id` and upsert the
    /// resulting row.
    ///
    /// Validation runs BEFORE any store write; malformed bytes return
    /// [`RecordError::MalformedInput`] and the store is untouched. The
    /// exact JSON shape this currently accepts is:
    ///
    /// ```json
    /// {"verdict": "pass" | "fail",
    ///  "failing_tests": ["<path>", ...]}
    /// ```
    ///
    /// A Pass payload may omit `failing_tests` (empty is implied) but
    /// must not supply a non-empty array; that mismatch is a
    /// [`RecordError::MalformedInput`]. A Fail payload must supply at
    /// least one failing test entry and every entry must be non-empty
    /// after trim (the story's guidance: "every reported failure has a
    /// non-empty file path").
    ///
    /// Empty bytes are the canonical malformed shape and are rejected
    /// with `field = "input"` — that is the exact case
    /// `record_malformed_input_preserves_row.rs` pins.
    pub fn record_from_raw(&self, story_id: i64, raw: &[u8]) -> Result<(), RecordError> {
        let input = parse_raw_input(story_id, raw)?;
        self.record_inner(input)
    }

    fn record_inner(&self, input: RunInput) -> Result<(), RecordError> {
        let commit = resolve_head_sha()?;
        let ran_at = rfc3339_utc_now()?;

        // Collapse failing-test paths to basenames at write time; this
        // is the contract pinned by `record_fail.rs`.
        let failing_tests: Vec<Value> = input
            .failing_tests
            .iter()
            .map(|p| Value::String(basename_of(p)))
            .collect();

        let key = input.story_id.to_string();
        let doc = json!({
            "story_id": input.story_id,
            "verdict": input.verdict.as_str(),
            "commit": commit,
            "ran_at": ran_at,
            "failing_tests": failing_tests,
        });

        self.store.upsert(TEST_RUNS_TABLE, &key, doc)?;
        Ok(())
    }
}

/// Parse and validate a raw test-runner payload into a [`RunInput`].
///
/// Separated from [`Recorder::record_from_raw`] for unit-test hygiene
/// and because the failure shape ([`RecordError::MalformedInput`]) is
/// purely a function of the bytes — no store, no git, no clock.
fn parse_raw_input(story_id: i64, raw: &[u8]) -> Result<RunInput, RecordError> {
    if raw.is_empty() {
        return Err(RecordError::MalformedInput {
            field: "input".to_string(),
            reason: "raw input is empty".to_string(),
        });
    }

    let value: Value = serde_json::from_slice(raw).map_err(|e| RecordError::MalformedInput {
        field: "input".to_string(),
        reason: format!("not valid JSON: {e}"),
    })?;

    let obj = value
        .as_object()
        .ok_or_else(|| RecordError::MalformedInput {
            field: "input".to_string(),
            reason: "expected a JSON object at the top level".to_string(),
        })?;

    let verdict_str =
        obj.get("verdict")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RecordError::MalformedInput {
                field: "verdict".to_string(),
                reason: "missing or not a string".to_string(),
            })?;

    let verdict = match verdict_str {
        "pass" => Verdict::Pass,
        "fail" => Verdict::Fail,
        other => {
            return Err(RecordError::MalformedInput {
                field: "verdict".to_string(),
                reason: format!("must be \"pass\" or \"fail\", got {other:?}"),
            });
        }
    };

    let failing_tests = match obj.get("failing_tests") {
        None => Vec::new(),
        Some(Value::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                let s = item.as_str().ok_or_else(|| RecordError::MalformedInput {
                    field: "failing_tests".to_string(),
                    reason: format!("entry {i} is not a string"),
                })?;
                if s.trim().is_empty() {
                    return Err(RecordError::MalformedInput {
                        field: "failing_tests".to_string(),
                        reason: format!("entry {i} is empty"),
                    });
                }
                out.push(s.to_string());
            }
            out
        }
        Some(_) => {
            return Err(RecordError::MalformedInput {
                field: "failing_tests".to_string(),
                reason: "expected an array of strings".to_string(),
            });
        }
    };

    match verdict {
        Verdict::Pass => {
            if !failing_tests.is_empty() {
                return Err(RecordError::MalformedInput {
                    field: "failing_tests".to_string(),
                    reason: "must be empty on a pass verdict".to_string(),
                });
            }
            Ok(RunInput::pass(story_id))
        }
        Verdict::Fail => {
            if failing_tests.is_empty() {
                return Err(RecordError::MalformedInput {
                    field: "failing_tests".to_string(),
                    reason: "must be non-empty on a fail verdict".to_string(),
                });
            }
            Ok(RunInput::fail(story_id, failing_tests))
        }
    }
}

/// Discover the git repository at or above the current working directory
/// and return the full 40-char hex SHA of its HEAD commit.
///
/// Uses `git2::Repository::discover(".")` so the recorder runs from any
/// subdirectory of the repo. Returns [`RecordError::Git`] on failure
/// rather than bubbling `git2::Error` out, to keep the public error type
/// free of third-party crates.
fn resolve_head_sha() -> Result<String, RecordError> {
    let repo = git2::Repository::discover(".")
        .map_err(|e| RecordError::Git(format!("could not discover repo: {e}")))?;
    let head = repo
        .head()
        .map_err(|e| RecordError::Git(format!("could not resolve HEAD: {e}")))?;
    let oid = head
        .target()
        .ok_or_else(|| RecordError::Git("HEAD is not a direct reference".to_string()))?;
    Ok(oid.to_string())
}

/// Format `SystemTime::now()` as an RFC3339 UTC string
/// (`YYYY-MM-DDTHH:MM:SSZ`, length 20, ends with `Z`).
///
/// Hand-rolled rather than pulled from `chrono` because the row
/// contract only needs seconds-resolution UTC, and the scaffold's
/// inline parser validates exactly this shape (see `record_pass.rs`).
/// Adding `chrono` would be a speculative dependency.
fn rfc3339_utc_now() -> Result<String, RecordError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| RecordError::Clock(format!("system clock before UNIX epoch: {e}")))?;
    let secs = now.as_secs() as i64;
    Ok(format_unix_to_rfc3339(secs))
}

/// Convert seconds-since-UNIX-epoch to `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Implemented from Howard Hinnant's `civil_from_days` algorithm — the
/// same math the scaffold's `parse_rfc3339_utc_to_unix` inverts. Keeping
/// the forward and inverse in lock-step guarantees the round-trip the
/// `record_pass.rs` test relies on.
fn format_unix_to_rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(86400);
    let secs_of_day = secs.rem_euclid(86400);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;

    // Howard Hinnant's days_from_civil, inverted: given days since
    // 1970-01-01, recover (year, month, day).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if month <= 2 { y + 1 } else { y };

    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z",
        year = year,
        month = month,
        day = day,
        hour = hour,
        minute = minute,
        second = second,
    )
}

/// Collapse a path-like string to just its file-name component, with
/// the extension preserved.
///
/// The story's row contract: `failing_tests` entries are basenames, not
/// paths; e.g. `"crates/agentic-ci-record/tests/record_fail.rs"` becomes
/// `"record_fail.rs"`. If the input has no path separators at all it is
/// returned unchanged.
fn basename_of(path: &str) -> String {
    // Prefer std::path::Path::file_name for portability; fall back to
    // the raw string if that returns None (e.g. a path ending in `/`).
    Path::new(path)
        .file_name()
        .and_then(|os| os.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_as_str_round_trips() {
        assert_eq!(Verdict::Pass.as_str(), "pass");
        assert_eq!(Verdict::Fail.as_str(), "fail");
    }

    #[test]
    fn basename_collapses_unix_paths() {
        assert_eq!(
            basename_of("crates/agentic-ci-record/tests/record_fail.rs"),
            "record_fail.rs"
        );
    }

    #[test]
    fn basename_passes_through_bare_filename() {
        assert_eq!(basename_of("record_fail.rs"), "record_fail.rs");
    }

    #[test]
    fn rfc3339_shape_matches_scaffold_expectations() {
        // 2026-04-18T21:52:21Z → the red-state evidence timestamp for
        // story 2, a convenient real value to pin the format against.
        // 2026-04-18 is 20561 days after 1970-01-01; 21:52:21 = 78741s.
        let secs = 20_561 * 86_400 + 78_741;
        let s = format_unix_to_rfc3339(secs);
        assert_eq!(s, "2026-04-18T21:52:21Z");
        assert_eq!(s.len(), 20);
        assert_eq!(s.as_bytes()[10], b'T');
        assert!(s.ends_with('Z'));
    }

    #[test]
    fn parse_raw_rejects_empty_bytes() {
        let err = parse_raw_input(1, &[]).unwrap_err();
        match err {
            RecordError::MalformedInput { field, .. } => assert_eq!(field, "input"),
            other => panic!("expected MalformedInput, got {other:?}"),
        }
    }

    #[test]
    fn parse_raw_accepts_pass_shape() {
        let raw = br#"{"verdict":"pass"}"#;
        let input = parse_raw_input(42, raw).expect("pass shape must parse");
        assert_eq!(input.story_id(), 42);
        assert_eq!(input.verdict(), Verdict::Pass);
        assert!(input.failing_tests().is_empty());
    }

    #[test]
    fn parse_raw_rejects_fail_without_failing_tests() {
        let raw = br#"{"verdict":"fail"}"#;
        let err = parse_raw_input(42, raw).unwrap_err();
        match err {
            RecordError::MalformedInput { field, .. } => assert_eq!(field, "failing_tests"),
            other => panic!("expected MalformedInput, got {other:?}"),
        }
    }
}
