//! Durable embedded [`Store`] implementation backed by the `surrealkv`
//! engine.
//!
//! See the story 5 guidance in `stories/5.yml` and ADR-0002 for the
//! contract this module delivers: trait parity with [`crate::MemStore`],
//! on-disk durability across a drop-and-reopen, standalone drivability
//! (no orchestrator / runtime dependency), and typed failure on a bad
//! root path.
//!
//! # Why `surrealkv` directly and not the full `surrealdb` crate
//!
//! ADR-0002 calls for "SurrealDB embedded" as the durable persistence
//! layer. The `surrealdb` crate's `kv-surrealkv` feature wraps exactly
//! the `surrealkv` engine this module links against — so on-disk format
//! compatibility with SurrealDB's embedded mode is preserved, and a
//! future migration "up" to the full `surrealdb` crate (when a consumer
//! needs SurrealQL or live queries) is a local swap inside this file.
//! See the workspace `Cargo.toml` for the compile-time budget rationale.
//!
//! # Data model
//!
//! `surrealkv` is a versioned byte-to-byte key-value store; the `Store`
//! trait is keyed by `(table, key)` with `serde_json::Value` payloads.
//! We map between them with the following namespacing scheme:
//!
//! - `upsert(tbl, key, doc)` writes to the surrealkv key `U/<tbl>/<key>`
//!   with `doc` serialised as JSON bytes. Re-upserting the same
//!   `(tbl, key)` replaces the value — surrealkv's `set` is upsert-like
//!   when it overwrites the previous version's visible row.
//! - `append(tbl, doc)` writes to `A/<tbl>/<seq>` where `<seq>` is a
//!   per-table monotonic counter padded so lexicographic order equals
//!   numeric order. The counter is persisted at `M/<tbl>` so insertion
//!   order survives a reopen.
//! - `get(tbl, key)` reads `U/<tbl>/<key>`, returning `Ok(None)` when
//!   the key is absent (typed absence, per the story-4 contract).
//! - `query(tbl, filter)` iterates the `A/<tbl>/` and `U/<tbl>/` prefixes
//!   and applies `filter` to each decoded `serde_json::Value`.
//!
//! We never mix the `U/` and `A/` prefixes for the same logical table,
//! so the existing story-4 "a table is either keyed or appended, not
//! both" convention is preserved without extra bookkeeping.

use std::path::Path;
use std::sync::Mutex;

use serde_json::Value;
use surrealkv::{LSMIterator, Mode, Tree, TreeBuilder};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::Mutex as AsyncMutex;

use crate::{Store, StoreError};

/// Durable [`Store`] implementation backed by the `surrealkv` embedded
/// engine. Constructed via [`SurrealStore::open`]. See module docs for
/// data-model and engine-choice rationale.
pub struct SurrealStore {
    /// Single-thread tokio runtime owned by the store so sync trait
    /// methods can `block_on` surrealkv's async `Transaction::commit`.
    runtime: Runtime,
    /// The surrealkv LSM handle. Wrapped in a [`Mutex`] so `&self`
    /// methods on [`Store`] can drive short-lived transactions without
    /// lifetime headaches. Used for the synchronous part of every
    /// trait method (the parts that don't `.await`).
    tree: Mutex<Tree>,
    /// Async mutex serialising the write path. Held across the
    /// transaction's async `commit`, which is what gives concurrent
    /// appenders a single well-ordered view of the per-table monotonic
    /// seq counter (see [`Store::append`]). Using `tokio::sync::Mutex`
    /// (rather than `std::sync::Mutex`) means holding this across an
    /// `.await` is both clippy-clean and deadlock-free on a
    /// multi-threaded runtime — neither of which is a hypothetical on
    /// this codebase: the trait-parity test for threaded access drives
    /// exactly this path from two threads at once.
    write: AsyncMutex<()>,
}

/// Hand-rolled `Debug` that prints only the opaque shape — the
/// underlying `Tree` and tokio `Runtime` are not themselves `Debug`,
/// and their internals would be noise in test panic output anyway.
/// We need `Debug` on the type because tests call `Result::expect_err`
/// (which prints the Ok value on failure, and therefore requires
/// `Debug` on it).
impl std::fmt::Debug for SurrealStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurrealStore").finish_non_exhaustive()
    }
}

/// Prefix tag for rows written via [`Store::upsert`]. One byte so
/// lexicographic order is easy to reason about.
const PREFIX_UPSERT: u8 = b'U';
/// Prefix tag for rows written via [`Store::append`].
const PREFIX_APPEND: u8 = b'A';
/// Prefix tag for the per-table monotonic append counter. Stored as an
/// 8-byte big-endian number so a single `get`+1 yields the next seq.
const PREFIX_APPEND_META: u8 = b'M';
/// Byte that terminates each namespace component inside composite keys.
/// Chosen as `0x00` because it does not appear in valid UTF-8 (so the
/// split is unambiguous for any string table/key).
const SEP: u8 = 0x00;

impl SurrealStore {
    /// Open a SurrealStore rooted at `root`. `root` is the directory
    /// surrealkv uses for its on-disk state; it is created if missing
    /// and its parent exists. Returns [`StoreError::Open`] if the path
    /// is not a usable data directory (an existing file, or a
    /// nonexistent parent).
    ///
    /// See story 5's `surrealstore_malformed_root_is_typed_error.rs`
    /// for the exact failure shapes pinned by tests.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        validate_root(&root)?;

        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| StoreError::Open {
                path: root.clone(),
                source: Box::new(e),
            })?;

        // surrealkv spawns tokio background tasks (memtable flush +
        // level compaction) during `TreeBuilder::build`, so the builder
        // must be called from within a runtime context. We also need
        // that context for every subsequent `set` / `get` / `range`,
        // because those pump into those same background tasks. Bind the
        // handle into a guard that lives as long as the tree.
        let _guard = runtime.enter();
        let tree = TreeBuilder::new()
            .with_path(root.clone())
            .build()
            .map_err(|e| StoreError::Open {
                path: root.clone(),
                source: Box::new(e),
            })?;
        drop(_guard);

        Ok(Self {
            runtime,
            tree: Mutex::new(tree),
            write: AsyncMutex::new(()),
        })
    }

    /// Acquire the tree mutex, mapping lock-poisoning to a [`Backend`]
    /// error rather than panicking.
    ///
    /// [`Backend`]: StoreError::Backend
    fn lock_tree(&self) -> Result<std::sync::MutexGuard<'_, Tree>, StoreError> {
        self.tree
            .lock()
            .map_err(|e| StoreError::Backend(format!("tree mutex poisoned: {e}")))
    }
}

impl Store for SurrealStore {
    fn upsert(&self, table: &str, key: &str, doc: Value) -> Result<(), StoreError> {
        let key_bytes = encode_key(PREFIX_UPSERT, table, key.as_bytes());
        let value_bytes = serde_json::to_vec(&doc)
            .map_err(|e| StoreError::Backend(format!("serialize doc: {e}")))?;

        // Serialise the entire write path — including the async commit
        // — via the async `write` mutex. We acquire it inside
        // `block_on` so the runtime is entered for both the synchronous
        // `begin` / `set` calls (which spawn background tasks
        // internally) and the async commit. See [`SurrealStore::write`]
        // for why this is an async mutex rather than a std one.
        self.runtime.block_on(async {
            let _write_guard = self.write.lock().await;
            let mut txn = {
                let tree = self.lock_tree()?;
                let mut txn = tree
                    .begin_with_mode(Mode::ReadWrite)
                    .map_err(|e| StoreError::Backend(format!("begin upsert txn: {e}")))?;
                txn.set(key_bytes, value_bytes)
                    .map_err(|e| StoreError::Backend(format!("set upsert row: {e}")))?;
                txn
            }; // std tree-mutex released here; async write-mutex still held.
            txn.commit()
                .await
                .map_err(|e| StoreError::Backend(format!("commit upsert: {e}")))
        })
    }

    fn append(&self, table: &str, doc: Value) -> Result<(), StoreError> {
        let row_value = serde_json::to_vec(&doc)
            .map_err(|e| StoreError::Backend(format!("serialize append doc: {e}")))?;
        let meta_key = encode_meta_key(table);

        self.runtime.block_on(async {
            // Hold the write lock from the read-of-seq through to the
            // commit. This serialises concurrent appenders so each one
            // observes the fully-committed counter from the previous
            // and produces a distinct row-key. Without this, two
            // appenders racing both read seq=N and both write to
            // `A/<tbl>/N`, silently losing one row.
            let _write_guard = self.write.lock().await;
            let mut txn = {
                let tree = self.lock_tree()?;
                let mut txn = tree
                    .begin_with_mode(Mode::ReadWrite)
                    .map_err(|e| StoreError::Backend(format!("begin append txn: {e}")))?;

                let current: u64 = match txn
                    .get(meta_key.as_slice())
                    .map_err(|e| StoreError::Backend(format!("read append seq: {e}")))?
                {
                    None => 0,
                    Some(bytes) => {
                        let arr: [u8; 8] = bytes.as_slice().try_into().map_err(|_| {
                            StoreError::Backend("append seq is not 8 bytes".to_string())
                        })?;
                        u64::from_be_bytes(arr)
                    }
                };
                let next = current
                    .checked_add(1)
                    .ok_or_else(|| StoreError::Backend("append seq overflow".to_string()))?;

                let row_key = encode_key(PREFIX_APPEND, table, &current.to_be_bytes());
                txn.set(row_key, row_value)
                    .map_err(|e| StoreError::Backend(format!("set append row: {e}")))?;
                txn.set(meta_key, next.to_be_bytes().to_vec())
                    .map_err(|e| StoreError::Backend(format!("update append seq: {e}")))?;
                txn
            }; // std tree-mutex released here; async write-mutex still held.

            txn.commit()
                .await
                .map_err(|e| StoreError::Backend(format!("commit append: {e}")))
        })
    }

    fn get(&self, table: &str, key: &str) -> Result<Option<Value>, StoreError> {
        let key_bytes = encode_key(PREFIX_UPSERT, table, key.as_bytes());
        // Read-only snapshot: no commit, so no await. The runtime
        // context is still required because surrealkv's internals may
        // notify background tasks during `get`; we enter it purely to
        // provide that context.
        let _guard = self.runtime.enter();
        let tree = self.lock_tree()?;
        let txn = tree
            .begin_with_mode(Mode::ReadOnly)
            .map_err(|e| StoreError::Backend(format!("begin get txn: {e}")))?;
        let got = txn
            .get(key_bytes.as_slice())
            .map_err(|e| StoreError::Backend(format!("get: {e}")))?;
        drop(txn);
        drop(tree);
        match got {
            None => Ok(None),
            Some(bytes) => {
                let v: Value = serde_json::from_slice(&bytes)
                    .map_err(|e| StoreError::Backend(format!("deserialize get row: {e}")))?;
                Ok(Some(v))
            }
        }
    }

    fn query(
        &self,
        table: &str,
        filter: &dyn Fn(&Value) -> bool,
    ) -> Result<Vec<Value>, StoreError> {
        let _guard = self.runtime.enter();
        let tree = self.lock_tree()?;
        let txn = tree
            .begin_with_mode(Mode::ReadOnly)
            .map_err(|e| StoreError::Backend(format!("begin query txn: {e}")))?;

        let mut out: Vec<Value> = Vec::new();
        // Read both append-shaped and upsert-shaped rows for the
        // table. The story-4 contract says a table is either keyed or
        // appended, never both — so in practice at most one of these
        // ranges is non-empty for any given table. Scanning both is
        // cheap when one is empty and keeps the code model-agnostic.
        for prefix in [PREFIX_APPEND, PREFIX_UPSERT] {
            let (start, end) = range_bounds(prefix, table);
            let mut iter = txn
                .range(start.as_slice(), end.as_slice())
                .map_err(|e| StoreError::Backend(format!("range query: {e}")))?;
            // surrealkv's `LSMIterator` is a cursor-style iterator:
            // `seek_first` to position, then loop on `valid()` /
            // `next()`. It deliberately does NOT implement
            // `std::iter::Iterator` because the cursor owns the
            // current row and can only hand out borrows of it.
            let mut valid = iter
                .seek_first()
                .map_err(|e| StoreError::Backend(format!("query seek_first: {e}")))?;
            while valid {
                let v_bytes = iter
                    .value()
                    .map_err(|e| StoreError::Backend(format!("query iter value: {e}")))?;
                let doc: Value = serde_json::from_slice(&v_bytes)
                    .map_err(|e| StoreError::Backend(format!("deserialize query row: {e}")))?;
                if filter(&doc) {
                    out.push(doc);
                }
                valid = iter
                    .next()
                    .map_err(|e| StoreError::Backend(format!("query iter next: {e}")))?;
            }
        }
        drop(txn);
        drop(tree);
        Ok(out)
    }
}

/// Compose the surrealkv byte key for a document at `(prefix, table, tail)`.
/// The wire format is `<prefix> <SEP> <table-bytes> <SEP> <tail-bytes>`,
/// which gives us:
///   - Cheap prefix scans for "all rows of table X under prefix P"
///     (see [`range_bounds`]).
///   - Lexicographic equality with a later lookup for the same inputs.
fn encode_key(prefix: u8, table: &str, tail: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + table.len() + 1 + tail.len());
    out.push(prefix);
    out.push(SEP);
    out.extend_from_slice(table.as_bytes());
    out.push(SEP);
    out.extend_from_slice(tail);
    out
}

/// Compose the surrealkv byte key holding the monotonic append counter
/// for `table`: `M <SEP> <table-bytes>`.
fn encode_meta_key(table: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + table.len());
    out.push(PREFIX_APPEND_META);
    out.push(SEP);
    out.extend_from_slice(table.as_bytes());
    out
}

/// Compute `[start, end)` byte bounds spanning every document written
/// under `(prefix, table)`. `end` is built by appending `0xFF` to the
/// scan prefix, which is strictly greater than any real tail (every
/// real tail byte is `< 0xFF` because our tails are either UTF-8 bytes
/// or big-endian u64s whose high byte is 0 for any reasonable
/// counter value).
fn range_bounds(prefix: u8, table: &str) -> (Vec<u8>, Vec<u8>) {
    let mut start = Vec::with_capacity(2 + table.len() + 1);
    start.push(prefix);
    start.push(SEP);
    start.extend_from_slice(table.as_bytes());
    start.push(SEP);

    let mut end = start.clone();
    // Replace the trailing SEP (0x00) with 0xFF to get a byte-string
    // that is strictly greater than every key sharing the scan prefix.
    let last = end.len() - 1;
    end[last] = 0xFF;
    (start, end)
}

/// Pre-validate a candidate root path. surrealkv creates missing
/// directories on its own, but its own error shape for "path is a
/// file" or "parent is missing" is not a stable public contract we can
/// rely on across versions. Doing this check ourselves keeps the typed
/// [`StoreError::Open`] shape the story-5 test pins.
fn validate_root(path: &Path) -> Result<(), StoreError> {
    // If the path already exists, it must be a directory — a file at
    // the target path is the "pointed at the wrong thing" deployment
    // error the story calls out.
    if path.exists() {
        if !path.is_dir() {
            return Err(StoreError::Open {
                path: path.to_path_buf(),
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "path exists but is not a directory",
                )),
            });
        }
        return Ok(());
    }
    // Path doesn't exist yet. Its parent must exist and be a directory
    // so surrealkv has somewhere to create the data dir.
    match path.parent() {
        None => Err(StoreError::Open {
            path: path.to_path_buf(),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "path has no parent directory",
            )),
        }),
        Some(parent) => {
            if !parent.exists() {
                return Err(StoreError::Open {
                    path: path.to_path_buf(),
                    source: Box::new(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("parent directory {} does not exist", parent.display()),
                    )),
                });
            }
            if !parent.is_dir() {
                return Err(StoreError::Open {
                    path: path.to_path_buf(),
                    source: Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "parent path {} exists but is not a directory",
                            parent.display()
                        ),
                    )),
                });
            }
            Ok(())
        }
    }
}

/// Sanity: `SurrealStore` must be `Send + Sync` so it can be held
/// behind `Arc<dyn Store + Send + Sync>` — the consumer-facing shape
/// pinned by story 4's "send+sync behind Arc<dyn Store>" contract and
/// exercised by story 5's threaded parity test.
fn _assert_send_sync() {
    fn check<T: Send + Sync>() {}
    check::<SurrealStore>();
}
