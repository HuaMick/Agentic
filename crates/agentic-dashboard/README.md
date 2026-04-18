# agentic-dashboard

Four-status story-health dashboard. Per story 3
([stories/3.yml](../../stories/3.yml)).

Reads `test_runs` and `uat_signings` from `agentic-store`, joins against
`stories/*.yml`, computes `health` per story, and renders a table with
columns `ID | Title | Health | Failing tests | Healthy at`. Sort order:
`unhealthy → under_construction → proposed → healthy` (error rows float to
the top).

Pure reader — does not write to the store, does not promote any story. Fails
loudly when the store is unreachable rather than producing a degraded
reading an operator might misinterpret.

The CLI subcommand `agentic stories health` lives in
[agentic-cli](../agentic-cli/) and is a thin wrapper over this library.
