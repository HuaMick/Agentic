# agentic-ci-record

Per-story test-run bookkeeping. Per story 2 ([stories/2.yml](../../stories/2.yml)) — shipped and healthy.

Upserts one row per story into the `test_runs` table via `agentic-store` on
every CI run. Records `verdict`, `commit`, `ran_at`, and `failing_tests[]`
(basenames only). No history; the dashboard only needs the latest result,
and CI runs produce a new record every time.

Allowed runtime dependencies (per the
[standalone-resilient-library pattern](../../patterns/standalone-resilient-library.yml)):
`agentic-store`, `git2`.

The CLI shim that invokes this library on each CI run lives in
[agentic-cli](../agentic-cli/) as `agentic ci record <id>`.
