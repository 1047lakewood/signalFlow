# Workflow Starter Kit — Claude Code Project Bootstrap

A shareable skill for bootstrapping new projects with Claude Code. Drop this into a conversation or paste it into your first `CLAUDE.md` to kick off the **Init Protocol** — a structured questioning phase that produces a full project spec, then sets up the skills-driven workflow for ongoing development.

---

## How to Use

1. Start a new Claude Code session in an empty (or new) project directory
2. Paste the **CLAUDE.md Template** below into `CLAUDE.md` at the project root
3. Say **"init"** to begin the Init Protocol
4. Answer the questions — Claude will generate your spec, directory structure, skills files, and first todo list
5. Say **"continue"** to start building, one feature at a time

---

## CLAUDE.md Template

Copy everything below this line into your project's `CLAUDE.md`:

```markdown
# [PROJECT NAME] — [ONE-LINE DESCRIPTION]

> This file was bootstrapped with the Workflow Starter Kit.
> Say "init" to begin project setup. Say "continue" to build the next feature.

## The "Init" Protocol

When the user says **"init"**, **"bootstrap"**, **"start project"**, or similar:

Run the full DISCOVER → SPEC → SCAFFOLD → SEED cycle below. Do not skip phases. Do not write any application code until SCAFFOLD is complete.

### Phase 1: DISCOVER (The Interview)

Ask questions in **rounds**. Each round is 3–5 focused questions. Wait for answers before moving to the next round. Summarize what you've learned after each round so the user can correct misunderstandings early.

**Round 1 — Vision & Purpose**
- What does this project do in one sentence?
- Who is this for? (yourself, a team, customers, open source?)
- What's the single most important thing it needs to do on day one?
- Is there an existing tool/product this replaces or improves on?
- What does "done" look like for a v1?

**Round 2 — Technical Shape**
- What language/framework are you using (or open to)?
- What platform(s)? (web, desktop, mobile, CLI, API, embedded?)
- Any hard constraints? (must run on Windows, must be offline, must use Postgres, etc.)
- Are there external services or APIs this needs to talk to?
- Any existing code, repos, or assets to integrate?

**Round 3 — Architecture & Data**
- What are the main "things" in your system? (users, documents, tracks, orders, etc.)
- How do they relate to each other?
- Where does data live? (files, database, cloud, in-memory?)
- Does this need auth/accounts, or is it single-user/local?
- What's the expected scale? (personal tool, 10 users, 10k users?)

**Round 4 — Interface & Experience**
- How will people interact with this? (CLI, GUI, web dashboard, API-only, etc.)
- Any UI/UX preferences or inspirations? (specific apps, design systems, dark mode, etc.)
- What are the 3–5 core screens/commands/endpoints?
- Does it need real-time updates, or is request/response fine?
- Any accessibility or i18n requirements?

**Round 5 — Priorities & Risks**
- What's the riskiest or most uncertain part of this project?
- What would you cut if you had to ship in half the time?
- Are there non-negotiable quality bars? (must have tests, must be type-safe, must be fast, etc.)
- What's your dev workflow preference? (TDD, CLI-first, prototype-then-refine, etc.)
- Anything else I should know that doesn't fit the above?

**Adaptive Follow-ups:** After each round, if answers reveal complexity (e.g., "it needs to talk to 4 APIs" or "it handles payments"), ask 2–3 targeted follow-ups before moving on. Don't assume — ask.

### Phase 2: SPEC

After all DISCOVER rounds are complete:

1. Write a **Project Spec** section at the top of this file with:
   - Project name and one-line description
   - Architecture summary (language, framework, platform, key dependencies)
   - Data model overview (main entities and relationships)
   - Interface description (CLI commands, API endpoints, or UI screens)
   - Non-negotiable requirements and constraints
   - What's explicitly out of scope for v1

2. Present the spec to the user for approval. Do not proceed until they confirm.

### Phase 3: SCAFFOLD

Once the spec is approved:

1. Set up the project directory structure (language-appropriate)
2. Initialize the project (cargo init, npm init, etc.)
3. Add dependencies from the spec
4. Create the `skills/` directory with:
   - `todo.md` — phased task list derived from the spec
   - `changelog.md` — empty, ready for entries
   - `roadmap.md` — full feature spec organized by phase
5. Verify the project builds/compiles with no errors

### Phase 4: SEED

1. Update this `CLAUDE.md` file with:
   - The final project spec (replacing the placeholder at the top)
   - Architecture section with actual tech choices
   - Directory map reflecting the scaffolded structure
   - Any project-specific rules discovered during DISCOVER
2. Confirm setup is complete and tell the user to say "continue" to start building

---

## The "Continue" Protocol

When the user says **"continue"**, **"next"**, **"keep going"**, or similar:

1. Execute the Workloop below automatically
2. Pick the **first unchecked `- [ ]` item** from `skills/todo.md`
3. Run the full SYNC → PLAN → BUILD → TEST → LOG cycle for that item

This is the primary way work gets done. No ambiguity — one trigger, one item, one cycle.

## The Workloop

Every feature goes through this cycle. Do not skip steps.

### 1. SYNC
- Read this file (`CLAUDE.md`)
- Read `skills/todo.md` — identify the next unchecked item
- Read relevant `skills/*.md` files for context on that feature
- Scan source code to understand current state

### 2. PLAN
- Select the ONE item identified in SYNC
- Mark it `in-progress` in `skills/todo.md` (change `- [ ]` to `- [~]`)
- If the feature involves complex logic, create or update a `skills/` design doc before coding
- State the plan briefly in chat

### 3. BUILD
- Implement the logic in the appropriate source files
- Expose it through the project's interface layer (CLI, API, UI — whatever Phase 2 SPEC defined)
- **Rule:** Interface layer must work before moving to secondary interfaces

### 4. TEST
- Write tests for the new logic (unit tests, integration tests — whatever the project uses)
- Run the full test suite — all tests must pass
- Run the linter/type checker — no warnings

### 5. LOG
- Update `skills/changelog.md` with what was built
- Update all `skills/*.md` design docs that reference the feature
- Update `skills/todo.md` — mark the item `- [x]` (complete) only after docs are accurate

### 6. COMMIT
- If all tests pass and there are no warnings, commit the changes
- Commit message format: short summary of what was built
- Stage all changes (not just the current cycle)
- After committing, push to origin

## The "Remember" Protocol

When the user says **"remember"**, **"add this to CLAUDE.md"**, **"note that"**, or similar:

- Add the information to the appropriate section of this file (`CLAUDE.md`)
- If no section fits, add it under **Rules** or create a new section
- Confirm what was added

## The "Roadmap" Protocol

When the user says **"future feature"**, **"add to roadmap"**, **"add to todo"**, or similar:

1. Add the feature to the appropriate phase in `skills/roadmap.md` (or create a new phase if none fits)
2. Add a corresponding `- [ ]` item to `skills/todo.md` under the matching phase
3. **Do NOT implement anything** — no code changes, no build, no tests
4. Confirm what was added and where

This is planning only. Implementation happens through the "Continue" protocol.

## Rules

- **Interface-first:** If it doesn't work through the primary interface, it doesn't exist
- **One feature per cycle:** Don't bundle unrelated changes
- **Separation of concerns:** Logic in library/modules, interface wiring in the entry point
- **Test everything:** Every module gets tests appropriate to the language
- **Design before complexity:** Multi-step features get a `skills/*.md` doc first
- **Full roadmap:** See `skills/roadmap.md` for the complete feature spec
- **Keep skills docs current (hard gate):** Do NOT mark a todo item `[x]` until its design docs are accurate. During the LOG step, update all `skills/*.md` files that reference the completed feature:
  - Add status markers (DONE) to completed sections
  - Fix data model descriptions to match actual code
  - Update test counts
  - Correct interface syntax to match actual definitions
  - Remove or fix any "not built" claims that are now false
  - If unsure, re-read the relevant source files before updating docs

## Directory Map

```
src/              — Source code
skills/           — Living documentation
  todo.md         — The strike-list. Single source of truth for "what's next"
  changelog.md    — History of completed work
  roadmap.md      — Full feature specifications
  *.md            — Design docs for specific domains
tests/            — Integration tests (if applicable)
```
```

---

## Tips for Sharing

- Send your friend this file, or just the **CLAUDE.md Template** section
- Works with any language or framework — the Init Protocol adapts based on answers
- The questioning rounds prevent the "build the wrong thing" problem
- After init, the workflow is self-sustaining: just say "continue" to keep building
- The skills directory becomes the project's living memory across sessions
