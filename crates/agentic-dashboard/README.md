# agentic-dashboard

Four-status story-health dashboard. Per stories 3 + 9
([stories/3.yml](../../stories/3.yml), [stories/9.yml](../../stories/9.yml)) —
both shipped and healthy.

Reads `test_runs` and `uat_signings` from `agentic-store`, joins against
`stories/*.yml`, computes `health` per story, and renders a table with
columns `ID | Title | Health | Failing tests | Healthy at`. Sort order:
`unhealthy → under_construction → proposed → healthy` (error rows float to
the top). Staleness is scoped per-story via the `related_files` field
(story 9): a UAT-pass commit older than HEAD marks a story `unhealthy`
only when a file changed since that commit matches one of the story's
declared globs. An empty or absent `related_files` is permissive.

Pure reader — does not write to the store, does not promote any story. Fails
loudly when the store is unreachable rather than producing a degraded
reading an operator might misinterpret.

The CLI subcommand `agentic stories health` lives in
[agentic-cli](../agentic-cli/) and is a thin wrapper over this library.
