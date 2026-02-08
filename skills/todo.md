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
- [ ] Settings config window — Centralized settings dialog for all engine configuration (crossfade, silence detection, intros, now-playing, conflict policy)
- [ ] Theme / dark mode — Dark-first UI with optional light theme

## Phase F: Ad Inserter / Scheduler System

- [ ] Ad scheduler handler — Intelligent hourly ad scheduling with lecture detection and track boundary awareness
- [ ] Ad inserter service — Internal MP3 concatenation via rodio, queue-based insertion into active playlist
- [ ] Ad play logger — JSON play statistics (per-ad, per-date, per-hour), failure tracking
- [ ] Ad report generator — CSV and PDF verified-play reports with hourly/daily breakdowns
- [ ] Ad config UI — Modal editor for ad CRUD, enable/disable, MP3 file picker, day/hour scheduling
- [ ] Ad statistics UI — Play calendar, sortable treeview, date filtering, export/report generation

## Phase G: RDS Engine (Radio Data System)

- [ ] RDS message rotation handler — TCP socket protocol (DPSTEXT commands), keepalive resends, configurable rotation timing
- [ ] RDS message filtering — Enable/disable, lecture detection, placeholder support ({artist}, {title}), day/hour scheduling
- [ ] Lecture detector — Track classification (blacklist > whitelist > starts-with-R), current/next track analysis
- [ ] RDS config UI — Modal message editor with 64-char limit, duration, day/hour scheduling, live treeview

## Phase H: Future / Long-Term

- [ ] Hosted web interface — Browser-based remote control and monitoring
- [ ] Advanced auto playlist builder — Rule-based automatic playlist generation
