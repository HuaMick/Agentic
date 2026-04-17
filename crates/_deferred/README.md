# _deferred/ — Placeholder crates

Crates we've identified as likely needed later but are **not** building yet. Each has a README describing what shape it would take and what would trigger building it.

The underscore prefix keeps these sorted below day-one crates and signals "not active" clearly.

**Rule:** nothing in `_deferred/` gets a `Cargo.toml`. If it compiles, it's not deferred — move it out.
