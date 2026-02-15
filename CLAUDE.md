# signalFlow — Operating Manual for Autonomous Development

signalFlow is a desktop-first radio automation system with a Rust core and Tauri/React GUI.

## Current Architecture (authoritative)

- **Core logic:** `src/` Rust library modules
- **Desktop shell:** `src-tauri/` command bridge + app wiring
- **GUI:** `gui/` React + TypeScript + Vite
- **Documentation memory:** `skills/` directory

> If docs and code disagree, code/tests are the source of truth.

---

## Autonomy Contract

When the user asks for implementation work, operate with a **high-autonomy default**:

1. Make reasonable assumptions and proceed without waiting for confirmation.
2. Prefer shipping a complete, tested slice over partial scaffolding.
3. If tradeoffs exist, pick the option that improves reliability, maintainability, and UX clarity.
4. Batch related low-risk improvements when they directly support the requested outcome.
5. Surface decisions in the final summary with rationale.

Only pause for clarification when a choice would likely break user intent or cause data loss.

---

## Execution Loop

### Trigger words
Treat **"continue"**, **"next"**, **"keep going"**, or equivalent as:
- pick the first unchecked item in `skills/todo.md`
- execute one full cycle

### Cycle
1. **SYNC**
   - Re-read `CLAUDE.md`, `skills/todo.md`, and relevant skill docs.
   - Inspect code paths likely impacted.
2. **PLAN**
   - Mark current item `- [~]` in `skills/todo.md`.
   - Define implementation + validation steps.
3. **BUILD**
   - Implement core logic first, then integration (Tauri/UI).
   - Keep modules cohesive; avoid hidden coupling.
4. **TEST**
   - Run targeted tests first, then broader checks.
   - Add tests when behavior changes.
5. **LOG**
   - Update `skills/changelog.md` and relevant `skills/*.md` docs.
   - Mark `skills/todo.md` item complete only when docs match code.
6. **COMMIT & PUSH**
   - Commit once checks pass and workspace is clean.
   - Push to remote after committing.

---

## Quality Gates (must pass before completion)

- Build/check passes for touched stacks (Rust and/or GUI).
- New/changed behavior has automated coverage where practical.
- No stale TODOs for work completed in this cycle.
- User-facing changes include concise usage notes.

---

## UI/UX Standards (for all present and future GUI work)

1. **Dense but readable:** optimize for operational workflows (radio operators), not marketing layouts.
2. **Predictable interactions:** every row/action supports keyboard + clear focus/selection state.
3. **Progressive disclosure:** advanced settings in drawers/modals; common actions always visible.
4. **Feedback immediacy:** every command returns visible state change, toast, log line, or disabled/loading state.
5. **Error ergonomics:** messages should state what failed, why, and the next recovery step.
6. **A11y baseline:** semantic controls, labels, contrast-safe colors, keyboard navigation.
7. **Visual consistency:** one spacing scale, one typography scale, one component style language.

When uncertain, prioritize clarity and speed-of-operation over decorative design.

---

## Reliability & “Learn from mistakes” Protocol

After any failure (test, runtime, tooling, or workflow):

1. Record root cause in relevant `skills/*.md` doc (or `skills/changelog.md` if no better place).
2. Add a concrete prevention rule/checklist item.
3. Apply the prevention in the same session when possible.

Do not repeat previously documented mistakes.

---

## Roadmap & Memory Commands

### "remember" / "note this"
- Add durable guidance to this file or a relevant `skills/*.md` doc.

### "add to roadmap" / "future feature"
- Update `skills/roadmap.md` + `skills/todo.md` only.
- Do **not** implement code for roadmap-only requests.

---

## Directory Map

- `src/` — Rust core modules and business logic
- `src-tauri/` — Tauri runtime and IPC command wiring
- `gui/` — React/TypeScript interface
- `skills/` — roadmap, todo, changelog, domain design docs
- `tests/` — integration/system tests
