# Agent Behavior Reference

This document describes how AI agents (Claude Code, Codex, etc.) should operate on this project.

The authoritative source for all agent instructions is **[`CLAUDE.md`]** at the project root.

## Key Sections in CLAUDE.md

- **Autonomy Contract** — high-autonomy default; make reasonable assumptions, ship complete slices, surface decisions in summaries.
- **Execution Loop** — trigger words (`continue`, `next`, `keep going`) drive a SYNC → PLAN → BUILD → TEST → LOG → COMMIT cycle against `skills/todo.md`.  you can do multiple tasks at once
- **Quality Gates** — build must pass, new behavior needs tests, no stale TODOs, user-facing changes get usage notes.
- **UI/UX Standards** — dense-but-readable, keyboard-first, progressive disclosure, immediate feedback, accessibility baseline.
- **Reliability Protocol** — after any failure: record root cause, add prevention rule, apply fix in the same session.
- **Roadmap & Memory Commands** — `"remember"` updates `CLAUDE.md`; `"add to roadmap"` updates `skills/roadmap.md` + `skills/todo.md` only (no code).

## Quick Rules

1. Code and tests are the source of truth — if docs disagree, follow the code.
2. Only pause for clarification when a choice would break user intent or cause data loss.
3. Do not repeat previously documented mistakes — check `skills/` docs and `memory/` before writing new code.
4. Commit only when checks pass and workspace is clean.
