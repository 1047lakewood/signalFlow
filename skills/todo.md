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

## Phase C: Scheduler

- [x] Scheduler data model — Scheduled events with time, mode, file path, priority
- [x] Overlay mode — Play sound on top of current audio
- [x] Stop mode — Kill current audio, play scheduled item
- [ ] Insert mode — Queue scheduled item as next track
- [ ] Conflict resolution — Define behavior when manual play conflicts with schedule

## Phase D: Data & Integration

- [ ] Track metadata editing — Edit artist, title, etc. and persist to file tags
- [ ] Now-Playing XML export — Output XML with current/next track info and playback state

## Phase E: GUI (Tauri)

- [ ] Tauri project scaffolding — Initialize Tauri + React/TypeScript frontend
- [ ] IPC bridge — Rust ↔ JS command layer exposing all core engine functions
- [ ] Main playlist view — Track list with columns (artist, title, duration, status)
- [ ] Playlist tabs — Multiple playlist tabs with add/close/rename
- [ ] Transport controls — Play, Stop, Skip, Seek bar, elapsed/remaining display
- [ ] Drag-and-drop reordering — Reorder tracks within and between playlists
- [ ] File browser / Add tracks — Dialog or drag-drop to add audio files to playlist
- [ ] Now-playing display — Current track info, progress bar, album art if available
- [ ] Auto-intro dot indicator — Visual dot on tracks that have a matching intro file
- [ ] Crossfade settings panel — Configure fade duration and curve type
- [ ] Silence detection settings — Configure threshold and skip duration
- [ ] Auto-intro config — Set intros folder path, enable/disable
- [ ] Track metadata editor — Inline or dialog editing of artist, title, etc.
- [ ] Schedule side pane — Editable schedule list in a side panel
- [ ] Log pane — Playback events and system logs underneath the schedule pane
- [ ] Level meter — Real-time audio level visualization
- [ ] Waveform display — Waveform overview for the currently playing track
- [ ] Theme / dark mode — Dark-first UI with optional light theme

## Phase F: Future / Long-Term

- [ ] Hosted web interface — Browser-based remote control and monitoring
- [ ] Ad scheduler — Spot scheduling with rotation, frequency caps, reporting
- [ ] Advanced auto playlist builder — Rule-based automatic playlist generation
