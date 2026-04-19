# agentic-uat

Signed UAT verdict. Per story 1 ([stories/1.yml](../../stories/1.yml)) —
shipped and healthy.

Runs a story's UAT walkthrough through a `UatExecutor` trait, refuses to
produce a verdict on a dirty git tree, and — on a Pass — writes a row to the
`uat_signings` table via `agentic-store` and promotes the story's YAML to
`status: healthy`. This is the only code path that writes `healthy` to a
story file.

Allowed runtime dependencies (per the
[standalone-resilient-library pattern](../../patterns/standalone-resilient-library.yml)):
`agentic-store`, `agentic-story`, `git2`. No orchestrator, no runtime, no
sandbox, no CLI-crate — so the gate still works when the rest of the system
is in flames.

The CLI subcommand `agentic uat <id> --verdict <pass|fail>` lives in
[agentic-cli](../agentic-cli/) and is a thin wrapper over this library.
The walkthrough-driver that decides the verdict is the `test-uat` agent
(see `agents/test/test-uat/`).
