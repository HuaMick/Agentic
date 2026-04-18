//! Story 2 acceptance test: the happy-path row shape produced by a passing
//! acceptance-test run.
//!
//! Justification (from stories/2.yml): proves the happy path — given a
//! passing acceptance-test run for story id `<n>`, `Recorder::record`
//! upserts a row to `test_runs` with `story_id=<n>`, `verdict=pass`,
//! `failing_tests=[]`, `commit` set to HEAD's SHA, and `ran_at` set to now
//! (RFC3339). Without this the dashboard cannot distinguish "tests pass"
//! from "we never ran them," which is the whole reason we are recording.
//!
//! The scaffold drives the library directly against a `MemStore` from
//! `agentic-store`, per the story's `Test file locations` guidance — no
//! real DB, no shell-out, no CLI. Red today comes from the fact that the
//! `agentic_ci_record::{Recorder, RunInput, Verdict}` surface does not
//! yet exist in `src/lib.rs`; the scaffold compiles once that surface
//! lands and then passes once the upsert semantics match.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use agentic_ci_record::{Recorder, RunInput, Verdict};
use agentic_store::{MemStore, Store};

#[test]
fn record_upserts_pass_row_with_expected_shape() {
    // Story id used for the run.  Any positive integer works; `42` is the
    // inline fixture convention in this repo's other scaffolds.
    const STORY_ID: i64 = 42;

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let recorder = Recorder::new(store.clone());

    // A passing acceptance-test run: verdict is Pass, no failing tests.
    let input = RunInput::pass(STORY_ID);

    // The recorder stamps `commit` from HEAD and `ran_at` from the wall
    // clock at record time.  The test captures "now" as a UNIX timestamp
    // before and after, and asserts the recorded ran_at falls inside that
    // window after parsing the RFC3339 string.
    let before = unix_now();
    recorder.record(input).expect("record should succeed on valid pass input");
    let after = unix_now();

    // Exactly one row lands in `test_runs` keyed by story_id.
    let row = store
        .get("test_runs", &STORY_ID.to_string())
        .expect("store get should succeed")
        .expect("recorder must have upserted a row for this story_id");

    assert_eq!(
        row.get("story_id").and_then(|v| v.as_i64()),
        Some(STORY_ID),
        "row must carry story_id={STORY_ID}; got row={row}"
    );
    assert_eq!(
        row.get("verdict").and_then(|v| v.as_str()),
        Some("pass"),
        "Pass run must record verdict=\"pass\"; got row={row}"
    );

    let failing = row
        .get("failing_tests")
        .and_then(|v| v.as_array())
        .expect("failing_tests must be an array");
    assert!(
        failing.is_empty(),
        "Pass run must record failing_tests=[]; got {failing:?}"
    );

    let commit = row
        .get("commit")
        .and_then(|v| v.as_str())
        .expect("row must carry a string `commit` field");
    // A full git SHA is 40 lowercase hex characters; the row contract in
    // the story's guidance says "full git SHA of HEAD".
    assert_eq!(
        commit.len(),
        40,
        "commit must be the full 40-char SHA; got {commit:?}"
    );
    assert!(
        commit.chars().all(|c| c.is_ascii_hexdigit()),
        "commit must be all hex; got {commit:?}"
    );

    let ran_at_str = row
        .get("ran_at")
        .and_then(|v| v.as_str())
        .expect("row must carry a string `ran_at` field");
    // Minimal RFC3339 shape check without pulling in chrono: the format
    // always has `T` between date and time and either `Z` or a timezone
    // offset at the end.  The recorder emits UTC, so `Z` is the expected
    // terminator; the length is 20 (no subseconds) or 24 (milli subsecs).
    assert!(
        ran_at_str.len() >= 20,
        "ran_at must be an RFC3339 UTC string; got {ran_at_str:?}"
    );
    assert!(
        ran_at_str.as_bytes().get(10) == Some(&b'T'),
        "ran_at must have T at index 10 per RFC3339; got {ran_at_str:?}"
    );
    assert!(
        ran_at_str.ends_with('Z'),
        "ran_at must be a UTC RFC3339 string ending in Z; got {ran_at_str:?}"
    );

    // Year / month / day / hour / minute / second parse as integers and
    // combine into a UNIX timestamp that falls inside [before, after].
    let ran_at_unix = parse_rfc3339_utc_to_unix(ran_at_str)
        .expect("ran_at must parse as RFC3339 UTC");
    assert!(
        ran_at_unix >= before && ran_at_unix <= after,
        "ran_at must fall within the record call; before={before}, ran_at={ran_at_unix}, after={after}"
    );

    // Sanity: the typed Verdict enum round-trips the pass arm.
    assert_eq!(Verdict::Pass.as_str(), "pass");
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after UNIX epoch")
        .as_secs() as i64
}

/// Parse a subset of RFC3339 sufficient for the recorder's output shape:
/// `YYYY-MM-DDTHH:MM:SS[.fff]Z`.  Deliberately hand-rolled to avoid
/// pulling in `chrono` as a dev-dependency; the recorder's implementation
/// is free to use whatever date library it likes.
fn parse_rfc3339_utc_to_unix(s: &str) -> Option<i64> {
    let s = s.strip_suffix('Z')?;
    // Allow optional fractional seconds; drop them for the epoch math.
    let s = match s.find('.') {
        Some(i) => &s[..i],
        None => s,
    };
    let (date, time) = s.split_once('T')?;
    let mut date_parts = date.splitn(3, '-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: i64 = date_parts.next()?.parse().ok()?;
    let day: i64 = date_parts.next()?.parse().ok()?;
    let mut time_parts = time.splitn(3, ':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    let second: i64 = time_parts.next()?.parse().ok()?;

    // Civil-calendar-to-UNIX-epoch math (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = month;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days_since_epoch = era * 146097 + doe - 719468;
    Some(days_since_epoch * 86400 + hour * 3600 + minute * 60 + second)
}
