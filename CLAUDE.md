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
- Update `skills/todo.md` — mark the item `- [x]` (complete)

### 6. COMMIT
- If all tests pass and `cargo check` has no warnings, commit the changes
- Commit message format: short summary of what was built
- Stage only the files changed in this cycle

## Rules

- **CLI-first:** If it doesn't work in the CLI, it doesn't exist
- **One feature per cycle:** Don't bundle unrelated changes
- **Library/binary split:** Logic in `src/lib.rs` + modules, CLI wiring in `src/main.rs`
- **Test everything:** Every module gets `#[cfg(test)]` unit tests
- **Design before complexity:** Multi-step features get a `skills/*.md` doc first
- **Full roadmap:** See `skills/roadmap.md` for the complete feature spec

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
