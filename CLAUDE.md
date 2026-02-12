# signalFlow — Radio Automation Engine

A high-performance, Windows-native radio automation system. Rust audio core (CLI-first) that will eventually power a Tauri GUI.

## Architecture

- **Core:** Rust library crate — all logic decoupled from interface
- **CLI:** clap (Phase 1 interface)
- **GUI:** Tauri + React/TypeScript (Phase 2, not started)
- **Audio:** rodio (playback, crossfading, sink management)
- **Metadata:** lofty (duration, artist, title)
- **Storage:** serde_json (playlists, schedule, config)
- **OS:** Windows (WASAPI via cpal/rodio)

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
- Scan `src/` to understand current code state

### 2. PLAN
- Select the ONE item identified in SYNC
- Mark it `in-progress` in `skills/todo.md` (change `- [ ]` to `- [~]`)
- If the feature involves complex logic, create or update a `skills/` design doc before coding
- State the plan briefly in chat

### 3. BUILD (CLI First)
- Implement the logic in the core library (`src/`)
- Expose it via a CLI command in `src/main.rs`
- **Rule:** Do not touch Tauri/UI code until the CLI command works

### 4. TEST
- Write Rust tests (`#[cfg(test)]`) for the new logic
- Run `cargo test` — all tests must pass
- Run `cargo check` — no warnings

### 5. LOG
- Update `skills/changelog.md` with what was built
- Update all `skills/*.md` design docs that reference the feature (see "Keep skills docs current" rule)
- Update `skills/todo.md` — mark the item `- [x]` (complete) only after docs are accurate

### 6. COMMIT
- If all tests pass and `cargo check` has no warnings, commit the changes
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

- **"Run" means launch the app:** When the user says "run", they mean launch the Tauri dev app (see Memory for launch steps), NOT the continue/workloop cycle
- **Learn from mistakes:** When something fails (running/stopping the app, builds, tests, anything), record what actually worked in Memory so the same mistake isn't repeated. This applies to everything — not just the app lifecycle
- **Best practices override convenience:** The project structure keeps CLI and GUI separate for easier LLM debugging, but always override for best practices (e.g., storing runtime data in OS app data dir instead of source tree)
- **CLI-first:** If it doesn't work in the CLI, it doesn't exist
- **One feature per cycle:** Don't bundle unrelated changes
- **Library/binary split:** Logic in `src/lib.rs` + modules, CLI wiring in `src/main.rs`
- **Test everything:** Every module gets `#[cfg(test)]` unit tests. Write lots of tests — err on the side of more coverage, not less
- **Test GUI too:** GUI/Tauri code also needs tests (component tests, integration tests). Don't skip testing just because it's frontend
- **Design before complexity:** Multi-step features get a `skills/*.md` doc first
- **Full roadmap:** See `skills/roadmap.md` for the complete feature spec
- **Phase-sized features get their own phase:** If a todo item is complex enough to warrant multiple sub-steps (e.g., has its own spec doc or multiple components), break it out into its own phase with individual checkboxes for each step
- **Todo ordering:** When adding new todo items, always insert them before the last far-off phase (Phase H: Future / Long-Term)
- **Keep skills docs current (hard gate):** Do NOT mark a todo item `[x]` until its design docs are accurate. During the LOG step, update all `skills/*.md` files that reference the completed feature:
  - Add status markers (DONE) to completed sections
  - Fix data model descriptions to match actual struct fields
  - Update test counts
  - Correct CLI syntax to match actual clap definitions
  - Remove or fix any "not built" claims that are now false
  - If unsure, re-read the relevant `src/` files before updating docs

## Directory Map

```
src/           — Rust source (library + binary)
skills/        — Living documentation
  todo.md      — The strike-list. Single source of truth for "what's next"
  changelog.md — History of completed work
  roadmap.md   — Full feature specifications
  *.md         — Design docs for specific domains
tests/         — Integration tests
```
