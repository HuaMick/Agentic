//! # agentic-store
//!
//! Persistence abstraction for the Agentic workspace.
//!
//! This crate defines the [`Store`] trait — the single point every runtime
//! crate writes through — and the in-memory [`MemStore`] implementation
//! used by tests and fast-path consumers.
//!
//! ## Story 4 behavioural contract
//!
//! Story 4 pins the following observable behaviours on [`Store`]; all of
//! them are exercised as integration tests under `tests/` in this crate,
//! and story 5's `SurrealStore` impl must satisfy the same tests:
//!
//! 1. **Upsert-by-key replaces.** Writing twice to the same `(table, key)`
//!    leaves exactly one row, equal to the second write.
//! 2. **Append-to-collection preserves.** Appending N times to a table
//!    yields N distinct rows in insertion order; later writes do not
//!    mutate earlier ones.
//! 3. **Typed absence on `get`.** A `get` for a `(table, key)` that was
//!    never written returns `Ok(None)` — never an error, never a panic.
//!    Missing tables and missing keys are indistinguishable at the trait
//!    level.
//! 4. **Empty query is not an error.** A filter matching zero rows (or a
//!    query against an unknown table) returns `Ok(vec![])`, not an error.
//! 5. **Send + Sync behind `Arc<dyn Store>`.** The trait object form is
//!    the canonical way consumers hold a store, and it must be shareable
//!    across threads.
//!
//! ## Why schemaless `serde_json::Value`
//!
//! ADR-0002 makes the project schemaless-by-default. Modelling documents
//! as [`serde_json::Value`] keeps the trait object-safe (no type
//! parameters on methods), matches the on-disk shape we expect from the
//! eventual SurrealDB backend, and lets consumers layer their own
//! typed serde helpers without the trait taking a position on them.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value;

mod surrealstore;
pub use surrealstore::SurrealStore;

/// Errors returned by [`Store`] operations.
///
/// Story 4 pinned the minimum: one "backend is broken" catch-all so
/// consumers can distinguish store failures from "not found" (which is
/// typed absence via `Ok(None)`, not an error).
///
/// Story 5 adds [`StoreError::Open`] for the `SurrealStore::open` path,
/// so deployment-time configuration mistakes (wrong path in an env var,
/// pointing the store at a file instead of a directory) surface as a
/// typed, pattern-matchable error at startup rather than as a panic
/// halfway through the first write. The variant carries the offending
/// path plus the underlying cause so logs name both without a consumer
/// having to stringly-match.
#[derive(Debug)]
#[non_exhaustive]
pub enum StoreError {
    /// The backend's internal state became unreachable. In the in-memory
    /// implementation this only happens if a previous operation panicked
    /// while holding the internal lock (poisoning the mutex). In the
    /// SurrealDB implementation this wraps a SurrealDB runtime error.
    Backend(String),

    /// Opening the store failed. Carries the path the caller supplied so
    /// a one-line "could not open store at <path>: <cause>" log is
    /// trivial to produce, and the underlying cause so callers who want
    /// deeper context can downcast it. See story 5's
    /// `surrealstore_malformed_root_is_typed_error.rs` for the exact
    /// failure shapes this variant covers.
    Open {
        /// The path the caller passed to `open`.
        path: PathBuf,
        /// The underlying cause. Typically an `std::io::Error` (path
        /// does not exist, is a file, is not writable) or a
        /// `surrealdb::Error` (engine-level failure).
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Backend(msg) => write!(f, "store backend error: {msg}"),
            StoreError::Open { path, source } => {
                write!(f, "could not open store at {}: {}", path.display(), source)
            }
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Backend(_) => None,
            StoreError::Open { source, .. } => Some(source.as_ref()),
        }
    }
}

/// The persistence abstraction every runtime crate writes through.
///
/// See the crate-level documentation for the five behavioural contracts
/// this trait pins. The trait is deliberately object-safe: consumers hold
/// it as `Arc<dyn Store>` (or `Arc<dyn Store + Send + Sync>`) so the
/// backend can be swapped without touching call sites.
pub trait Store: Send + Sync {
    /// Write a document at `(table, key)`. If a document already exists at
    /// that coordinate, it is replaced. See the upsert-by-key contract in
    /// the crate docs.
    fn upsert(&self, table: &str, key: &str, doc: Value) -> Result<(), StoreError>;

    /// Append a document to the collection at `table`. Each call yields a
    /// new row; later calls do not mutate or replace earlier ones. See the
    /// append-to-collection contract in the crate docs.
    fn append(&self, table: &str, doc: Value) -> Result<(), StoreError>;

    /// Return the document at `(table, key)`, or `None` if no document has
    /// ever been written at that coordinate. Missing tables and missing
    /// keys are indistinguishable at the trait level; both return
    /// `Ok(None)`. See the typed-absence contract in the crate docs.
    fn get(&self, table: &str, key: &str) -> Result<Option<Value>, StoreError>;

    /// Return every document in `table` for which `filter` is true, in
    /// insertion order. An empty result (no matches, or unknown table) is
    /// `Ok(vec![])` — never an error. See the query-by-filter contract in
    /// the crate docs.
    ///
    /// The filter is taken as `&dyn Fn` so [`Store`] remains object-safe;
    /// closures at the call site work as expected.
    fn query(&self, table: &str, filter: &dyn Fn(&Value) -> bool)
        -> Result<Vec<Value>, StoreError>;
}

/// The row kind stored for a given table.
///
/// A table is either a keyed map (written by [`Store::upsert`]) or an
/// append-only list (written by [`Store::append`]). Mixing the two kinds
/// on the same table is an error condition we are not asked to handle in
/// story 4; the first operation against a table effectively fixes its
/// kind. Later stories may tighten this into an explicit error; for now
/// the tests never mix, and the simpler model keeps the implementation
/// auditable.
enum Rows {
    Keyed(Vec<(String, Value)>),
    Appended(Vec<Value>),
}

impl Rows {
    fn values(&self) -> Vec<&Value> {
        match self {
            Rows::Keyed(v) => v.iter().map(|(_, doc)| doc).collect(),
            Rows::Appended(v) => v.iter().collect(),
        }
    }
}

/// In-memory [`Store`] implementation.
///
/// Writes are held in a single [`Mutex<HashMap<_, Rows>>`] — the simplest
/// structure that satisfies the five story-4 contracts. There is no
/// persistence: dropping the [`MemStore`] discards everything it held.
///
/// [`MemStore`] is `Send + Sync` and intended to be shared behind
/// `Arc<dyn Store + Send + Sync>`, which is the canonical form consumers
/// hold.
#[derive(Default)]
pub struct MemStore {
    tables: Mutex<HashMap<String, Rows>>,
}

impl MemStore {
    /// Construct an empty [`MemStore`].
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, Rows>>, StoreError> {
        self.tables
            .lock()
            .map_err(|e| StoreError::Backend(format!("tables mutex poisoned: {e}")))
    }
}

impl Store for MemStore {
    fn upsert(&self, table: &str, key: &str, doc: Value) -> Result<(), StoreError> {
        let mut tables = self.lock()?;
        let rows = tables
            .entry(table.to_string())
            .or_insert_with(|| Rows::Keyed(Vec::new()));
        match rows {
            Rows::Keyed(v) => {
                if let Some(existing) = v.iter_mut().find(|(k, _)| k == key) {
                    existing.1 = doc;
                } else {
                    v.push((key.to_string(), doc));
                }
                Ok(())
            }
            Rows::Appended(_) => Err(StoreError::Backend(format!(
                "table '{table}' was previously used with append(); cannot upsert into it"
            ))),
        }
    }

    fn append(&self, table: &str, doc: Value) -> Result<(), StoreError> {
        let mut tables = self.lock()?;
        let rows = tables
            .entry(table.to_string())
            .or_insert_with(|| Rows::Appended(Vec::new()));
        match rows {
            Rows::Appended(v) => {
                v.push(doc);
                Ok(())
            }
            Rows::Keyed(_) => Err(StoreError::Backend(format!(
                "table '{table}' was previously used with upsert(); cannot append to it"
            ))),
        }
    }

    fn get(&self, table: &str, key: &str) -> Result<Option<Value>, StoreError> {
        let tables = self.lock()?;
        // Typed absence: missing table and missing key both return Ok(None).
        let Some(rows) = tables.get(table) else {
            return Ok(None);
        };
        match rows {
            Rows::Keyed(v) => Ok(v.iter().find(|(k, _)| k == key).map(|(_, doc)| doc.clone())),
            // An appended-only table has no notion of per-key lookup; behave
            // the same as a missing key from the consumer's perspective.
            Rows::Appended(_) => Ok(None),
        }
    }

    fn query(
        &self,
        table: &str,
        filter: &dyn Fn(&Value) -> bool,
    ) -> Result<Vec<Value>, StoreError> {
        let tables = self.lock()?;
        // Empty / unknown: Ok(vec![]), never an error.
        let Some(rows) = tables.get(table) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for doc in rows.values() {
            if filter(doc) {
                out.push(doc.clone());
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }

    // Sanity check that the trait object form compiles at the unit-test
    // level as well — the full Send + Sync contract is exercised by
    // tests/memstore_trait_object_is_send_sync.rs.
    #[test]
    fn memstore_is_a_store() {
        let _: Box<dyn Store> = Box::new(MemStore::new());
    }
}
