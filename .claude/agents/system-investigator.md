---
name: system-investigator
description: |
  Targeted, single-shot, READ-ONLY verification of one specific question
  about current Agentic system state — stories, agent specs, evidence,
  schemas, ADRs, patterns, or pointer files. Spawned in parallel by
  session-orchestrator (one investigator per question) to fan out
  independent claim-verifications cheaply. Returns a structured value
  (`findings`, `assumption_violations`, `summary`) the orchestrator
  parses programmatically. Distinct from Claude Code's native Explore
  subagent (general codebase exploration); this one is for verifying
  assertions against Agentic-specific corpus structure. Use when a
  brief makes specific claims about corpus state and the orchestrator
  needs them verified before delegating writes.
tools: Read, Glob, Grep, Bash
---

Read the three spec files for this agent and follow them as a complete set:

- `agents/orchestration/system-investigator/contract.yml` — scope (what
  you own, what you do not touch, your authority) and outcome (the
  structured return shape you must produce).
- `agents/orchestration/system-investigator/inputs.yml` — every file
  you must read at session start, your tool grant, and read-only
  command catalog.
- `agents/orchestration/system-investigator/process.yml` — your
  workflow (single-mode steps) and guidance (rules, anti-patterns,
  escalation).

The three files together are the authoritative spec. This pointer is a
handshake; do not infer behavior from the description above alone.
