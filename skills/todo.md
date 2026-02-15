# signalFlow — Todo

> Maintenance note (2026-02-13): treat this as a living design record. Verify behavior against current code and tests before implementation decisions.

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

## Phase H: Unified App Architecture

- [x] Design unified architecture — Plan how to merge CLI+GUI into one Tauri app while preserving full testability
- [x] Migrate CLI commands into Tauri — Move all CLI functionality into Tauri commands (remove standalone CLI binary)
- [x] Remove polling/Mutex overhead — Replace Mutex-based state sharing with direct engine ownership or event-driven architecture
- [x] Headless test harness — Ensure all features are testable via `cargo test` without launching the GUI
- [x] Remove standalone CLI binary — Delete `src/main.rs` CLI once all functionality is covered by the unified app + test suite

## Phase I: GUI Playlist Interaction

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

## Phase J: Dev Experience

- [x] Graceful Vite shutdown — Ensure the Vite dev server releases port 1420 when the Tauri app closes (no orphan node.exe processes)

## Phase K: Playlist UX Enhancements

- [x] Start time column — Add a "Start Time" column (2nd column) showing calculated future play times based on current track start time + cumulative durations; when a track begins playing, stamp its actual start time
- [x] Resizable columns — Allow all playlist columns to be resized by dragging column header edges (done in Phase I)

## Phase L: File Browser

- [x] File browser panel — Side panel or drawer showing the local filesystem tree for browsing audio files
- [x] Directory navigation — Expand/collapse folders, back/up buttons, path breadcrumb or address bar
- [x] Audio file filtering — Only show supported audio formats (mp3, wav, flac, ogg, etc.) and directories
- [x] File metadata preview — Show artist, title, duration on hover or selection in the browser
- [x] Drag from browser to playlist — Drag files or folders from the file browser into a playlist
- [x] Double-click to add — Double-click a file to append it to the active playlist
- [x] Favorite/pinned folders — Save frequently used directories for quick access

## Phase M: UI Polish & Transport Fixes

- [x] Playtime format — Display playtime as `Sun 4:54:25 PM` (day-of-week + 12-hour clock) instead of raw elapsed time
- [ ] File path display — Show drive-letter paths (`D:\Music\...`) instead of UNC paths (`\\UNC\...`) in the file path column
- [ ] Playlist right-margin padding — Add padding to the last column so text (e.g. "DURATION") doesn't touch the panel edge
- [ ] Separate Play and Pause buttons — Replace the combined play/pause toggle with distinct Play and Pause buttons; Play should start playback of the currently selected track (even while another track is playing), Pause should pause the current track
- [ ] Suppress browser context menu — Ensure the default browser right-click context menu never appears anywhere in the app; only show custom context menus where defined

## Phase N: Streaming & Recording

- [ ] Internet streaming output — Add ability to stream audio output to an internet streaming service (Icecast, Shoutcast, or similar)
- [ ] Playback recording — Record all playback output to audio files, one file per calendar day; evaluate whether loopback from the streaming service or direct audio capture is the best approach

## Phase O: File Browser Enhancements

- [ ] Instant file search — Add a fast search/filter bar to the file browser for real-time filename matching across indexed locations
- [ ] Context-menu filename search — Right-click a track (in playlist or file browser) to search for its filename across all indexed locations
- [ ] External drive indexing — Allow adding external/removable drives to the file search index so they are included in instant search results
- [ ] Favorite folders pane — Add a collapsible favorites sidebar in the file browser; when collapsed, show only folder icons; expand on hover to reveal full folder names/paths

## Phase P: Settings & Layout

- [ ] Options/settings window — Add a centralized settings/configuration window (discuss and confirm which settings to include before implementing)
- [ ] Header bar declutter — Reduce button density on the header bar by relocating actions to the left sidebar pane; discuss exact layout and grouping before implementing

## Phase Z: Future / Long-Term

- [ ] Hosted web interface — Browser-based remote control and monitoring
- [ ] Advanced auto playlist builder — Rule-based automatic playlist generation
- [ ] In-app audio editor — Audacity-style editor with audio preview, trim, volume, playback speed, and similar edits; support batch editing by loading a file list, applying edits globally, and previewing before saving
