# signalFlow — Todo

## Phase A: Core Audio Engine

- [x] Multi-instance playback — Support multiple playlists in memory
- [x] Active context switching — Playing a track switches the active playlist context
- [x] Transport controls — Play, Stop, Skip Next, Seek
- [x] Crossfading — Configurable fade duration between tracks
- [x] Silence detection — Auto-skip when signal drops below threshold for X seconds

## Phase B: Playlist Management

- [x] Playlist CRUD — Remove, Reorder, Copy, Paste tracks within/between playlists
- [x] Metadata enhancement — Calculated vs Played duration, filename fallback improvements
- [x] Auto-Intro system — Check intros folder, play intro before matching artist tracks
- [x] Auto-Intro dot indicator — `has_intro: bool` data flag implemented; GUI dot indicator deferred to Phase E
- [x] Recurring intro overlay — Re-play artist intro every 15 min as overlay (duck volume), timer resets per track, no retroactive playback

## Phase C: Scheduler

- [x] Scheduler data model — Scheduled events with time, mode, file path, priority
- [x] Overlay mode — Play sound on top of current audio
- [x] Stop mode — Kill current audio, play scheduled item
- [x] Insert mode — Queue scheduled item as next track
- [x] Conflict resolution — Define behavior when manual play conflicts with schedule

## Phase D: Data & Integration

- [x] Track metadata editing — Edit artist, title, etc. and persist to file tags
- [x] Now-Playing XML export — Output XML with current/next track info and playback state

## Phase E: GUI (Tauri)

- [x] Tauri project scaffolding — Initialize Tauri + React/TypeScript frontend
- [x] IPC bridge — Rust ↔ JS command layer exposing all core engine functions
- [x] Main playlist view — Track list with columns (artist, title, duration, status)
- [x] Playlist tabs — Multiple playlist tabs with add/close/rename
- [x] Transport controls — Play, Stop, Skip, Seek bar, elapsed/remaining display
- [x] Drag-and-drop reordering — Reorder tracks within and between playlists
- [x] File browser / Add tracks — Dialog or drag-drop to add audio files to playlist
- [x] Now-playing display — Current track info, progress bar (no album art)
- [x] Auto-intro dot indicator — Visual dot on tracks that have a matching intro file
- [x] Crossfade settings panel — Configure fade duration and curve type
- [x] Silence detection settings — Configure threshold and skip duration
- [x] Auto-intro config — Set intros folder path, enable/disable
- [x] Track metadata editor — Inline or dialog editing of artist, title, etc.
- [x] Schedule side pane — Editable schedule list in a side panel
- [x] Log pane — Playback events and system logs underneath the schedule pane
- [x] Level meter — Real-time audio level visualization
- [x] Waveform display — Waveform overview for the currently playing track
- [x] Settings config window — Centralized settings dialog for all engine configuration (crossfade, silence detection, intros, now-playing, conflict policy)
- [x] Theme / dark mode — Dark-first UI with optional light theme

## Phase F: Ad Inserter / Scheduler System

- [x] Ad scheduler handler — Intelligent hourly ad scheduling with lecture detection and track boundary awareness
- [x] Ad inserter service — Internal MP3 concatenation via rodio, queue-based insertion into active playlist
- [x] Ad play logger — JSON play statistics (per-ad, per-date, per-hour), failure tracking
- [x] Ad report generator — CSV and PDF verified-play reports with hourly/daily breakdowns
- [x] Ad config UI — Modal editor for ad CRUD, enable/disable, MP3 file picker, day/hour scheduling
- [x] Ad statistics UI — Play calendar, sortable treeview, date filtering, export/report generation

## Phase G: RDS Engine (Radio Data System)

- [x] RDS message rotation handler — TCP socket protocol (DPSTEXT commands), keepalive resends, configurable rotation timing
- [x] RDS message filtering — Enable/disable, lecture detection, placeholder support ({artist}, {title}), day/hour scheduling
- [x] Lecture detector — Track classification (blacklist > whitelist > starts-with-R), current/next track analysis
- [x] RDS config UI — Modal message editor with 64-char limit, duration, day/hour scheduling, live treeview

## Phase E4: Unified App Architecture

- [x] Design unified architecture — Plan how to merge CLI+GUI into one Tauri app while preserving full testability
- [x] Migrate CLI commands into Tauri — Move all CLI functionality into Tauri commands (remove standalone CLI binary)
- [x] Remove polling/Mutex overhead — Replace Mutex-based state sharing with direct engine ownership or event-driven architecture
- [x] Headless test harness — Ensure all features are testable via `cargo test` without launching the GUI
- [x] Remove standalone CLI binary — Delete `src/main.rs` CLI once all functionality is covered by the unified app + test suite

## Phase E2: GUI Playlist Interaction

- [x] Playlist scrollbar dark mode — Fix scrollbar styling to match dark theme
- [x] Row selection — Click a row to highlight/select it visually
- [x] Play from selection — Double-click or press Play to start playback from the selected row
- [x] Pause/unpause — Add a Pause button; Play resumes from where it paused instead of restarting
- [x] Right-click context menu — Custom context menu on rows (suppress browser default)
- [x] Drag-to-reorder rows — Drag selected row(s) to a new position in the playlist
- [x] Cut/copy/paste via context menu — Right-click cut, copy, paste; paste inserts immediately after the selected row
- [x] Multi-select — Shift+click for range select, Ctrl+click for toggle individual rows
- [x] Resizable columns — Drag column header edges to resize column widths
- [x] Row number column — Display a sequential row number as the first column
- [x] File path column — Show the file path in a dedicated column
- [x] Find bar — Search rows by any text field; include a dedicated row-number jump input
- [x] Auto-advance playback — Automatically play the next track when the current one ends, respecting crossfade settings

## Phase E3: Dev Experience

- [x] Graceful Vite shutdown — Ensure the Vite dev server releases port 1420 when the Tauri app closes (no orphan node.exe processes)

## Phase H: Future / Long-Term

- [ ] Hosted web interface — Browser-based remote control and monitoring
- [ ] Advanced auto playlist builder — Rule-based automatic playlist generation
