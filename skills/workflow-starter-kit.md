# Workflow Starter Kit — High-Autonomy Bootstrap Template

> Maintenance note (2026-02-13): treat this as a living design record. Verify behavior against current code and tests before implementation decisions.

Use this to initialize new projects so future sessions produce stronger code quality, better UI decisions, and less back-and-forth.

## Goals

- Get from idea → executable spec quickly.
- Build a durable `skills/` memory system.
- Default to autonomous execution with explicit quality gates.
- Improve future UI output through clear design standards.

---

## Quick Start

1. Create a new repo/folder.
2. Copy the template below into root `CLAUDE.md`.
3. Say **"init"**.
4. Answer discovery rounds.
5. Approve spec.
6. Say **"continue"** for iterative delivery.

---

## CLAUDE.md Template

```markdown
# [PROJECT NAME] — [ONE-LINE DESCRIPTION]

## Operating Mode

- High autonomy by default: make reasonable assumptions and execute.
- Ship vertical slices (logic + interface + tests + docs), not disconnected scaffolds.
- If docs disagree with code, code/tests are the source of truth.

## Init Protocol

When the user says **"init"**, run DISCOVER → SPEC → SCAFFOLD → SEED.

### 1) DISCOVER (structured interview)

Run rounds of 3–5 questions and summarize after each round.

- Vision: problem, user, v1 success criteria.
- Platform: runtime targets, constraints, dependencies.
- Data/domain: entities, workflows, persistence.
- Interface: CLI/API/UI shape, accessibility, responsiveness.
- Risks/priorities: technical uncertainty, schedule constraints, quality bars.

Add targeted follow-ups when complexity appears. Prefer precision over speed.

### 2) SPEC

Create a concrete spec section including:

- Architecture and module boundaries.
- Data model and invariants.
- Interface contract (commands/endpoints/screens).
- Test strategy and quality gates.
- Non-goals for v1.

Wait for explicit approval before scaffolding.

### 3) SCAFFOLD

- Initialize project and dependencies.
- Create `skills/` with `todo.md`, `roadmap.md`, `changelog.md`, and domain docs.
- Add CI-friendly scripts for check/test/lint.
- Verify clean build.

### 4) SEED

Update `CLAUDE.md` with final project-specific rules:

- Execution loop
- Quality gates
- UI/UX standards (if UI exists)
- Failure learning protocol
- Directory map

Then prompt user to say **"continue"**.

## Continue Protocol

When user says **"continue"**, pick first unchecked item in `skills/todo.md` and run:

1. SYNC (read docs + code)
2. PLAN (mark `- [~]` and plan)
3. BUILD (implement complete slice)
4. TEST (automated checks)
5. LOG (update skills docs)
6. COMMIT (clean, verified commit)

## UI Quality Standard (always include for UI projects)

- Task-first layout: frequent actions visible without navigation depth.
- Strong interaction states: hover/focus/active/disabled/loading.
- Keyboard support + accessibility labels by default.
- Consistent spacing/typography/tokens.
- Clear validation and recovery-focused error messages.
- Empty/loading/error states for every data panel.

## Coding Quality Standard

- Keep domain logic isolated from transport/view layers.
- Prefer small, testable modules with explicit contracts.
- Add tests for new behavior and regressions.
- Refactor opportunistically when complexity is reduced.
- Document non-obvious decisions in `skills/*.md`.

## Roadmap Protocol

For roadmap-only requests:

- Update `skills/roadmap.md` and `skills/todo.md`.
- Do not implement code.

## Remember Protocol

For durable guidance:

- Add it to `CLAUDE.md` or the most relevant skill doc.
```

---

## Notes

- This kit intentionally optimizes for autonomous progress and maintainable output.
- Re-run init sections if project direction changes significantly.
