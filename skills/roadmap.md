# signalFlow — Feature Roadmap

## Phase A: Core Audio Engine (Priority 1)

### Multi-Instance Playback
- Support multiple "Playlists" (Tabs) in memory
- Each playlist holds an ordered list of tracks with metadata

### Active Context
- Playing a track in a Playlist switches the "Active" context to that list
- Only one playlist is active at a time

### Transport Controls
- Play, Stop, Skip Next, Seek
- Track position reporting (elapsed / remaining)

### Crossfading
- Configurable fade duration between tracks (in seconds)
- Fade-out of ending track overlaps with fade-in of next track
- Crossfade curves (linear initially, configurable later)

### Silence Detection
- Monitor audio output levels in real-time
- Auto-skip if signal drops below configurable threshold for X seconds
- Threshold and duration both configurable

## Phase B: Playlist Management

### CRUD Operations
- Add, Remove, Reorder, Copy, Paste tracks within and between playlists

### Metadata Parsing
- Parse file paths to extract Artist, Title, Duration
- Support both embedded metadata (lofty) and filename fallback
- Track "Calculated Duration" vs "Played Duration"

### Auto-Intro System
- User-configured "Intros" folder path (in config)
- Before playing `Artist A - Song.mp3`, check Intros folder for `Artist A.mp3`
- If found: play intro, then crossfade into song (or mix over intro tail)
- Data structure must support a boolean "has_intro" flag for UI dot indicator
- Recurring intro overlay (DONE): re-play artist intro every N seconds as overlay during track playback
- Overlay mode (DONE): duck main track volume during intro, restore after
- Timer resets per track (DONE); configurable interval and duck volume
- CLI: `config intros recurring set <interval> [--duck <vol>]` / `config intros recurring off`

## Phase C: Scheduler

### Modes
- **Overlay:** Play sound on top of current audio (e.g., sound FX, jingles)
- **Stop:** Kill current audio, play scheduled item (e.g., hard news break)
- **Insert:** Queue scheduled item as the next track in the active playlist

### Scheduled Events
- Time-based triggers (e.g., "play news_open.mp3 at 14:00:00")
- Recurring events (hourly, daily patterns)
- Event metadata: time, mode, file path, priority

### Conflict Resolution
- If user manually plays a track during a scheduled event window, define behavior:
  - Schedule overrides (hard break)
  - Schedule waits until current track ends (soft break)
  - Schedule is skipped (manual override)
- Priority levels for scheduled events

## Phase D: Data & Integration

### Track Metadata Editing (DONE)
- Allow editing track metadata (artist, title, etc.) from within the app
- Changes persist to the audio file's embedded tags (via lofty)
- CLI: `track edit <playlist> <track_num> [--artist <val>] [--title <val>]`
- Uses `Track::write_tags()` → lofty `Accessor::set_artist/set_title` + `TagExt::save_to_path`

### Now-Playing XML Export (DONE)
- Output an XML file with current track info, next track info, and playback state
- Fields: artist, title, duration, elapsed, remaining, playlist name, state (playing/stopped)
- CLI: `nowplaying [file]` — snapshot export; `config nowplaying set <path>` / `off`
- `NowPlaying::from_engine()` + `to_xml()` + `write_xml()`
- XML escaping for special characters, remaining clamps to zero
- Useful for stream overlays, web widgets, and external integrations

## Phase E: GUI (Tauri)

### Tauri Project Scaffolding (DONE)
- Cargo workspace with `src-tauri/` member depending on core `signal_flow` library
- React 19 + TypeScript + Vite 6 frontend in `gui/`
- Tauri v2 backend with `AppState` (Mutex<Engine>), initial `get_status` IPC command
- Dark-first CSS theme, dev server on port 1420, Tauri capabilities configured

### IPC Bridge (DONE)
- 20 Tauri commands exposing all core engine functions to the frontend
- Structured JSON response types: StatusResponse, PlaylistInfo, TrackInfo, ScheduleEventInfo, ConfigResponse
- Playlist CRUD: get_playlists, create_playlist, delete_playlist, rename_playlist, set_active_playlist
- Track operations: get_playlist_tracks, add_track, remove_tracks, reorder_track, edit_track_metadata
- Schedule: get_schedule, add_schedule_event, remove_schedule_event, toggle_schedule_event
- Config: get_config, set_crossfade, set_silence_detection, set_intros_folder, set_conflict_policy, set_nowplaying_path
- All commands persist state via Engine::save() after mutations
- Event system for engine → frontend updates (track change, position, levels) — deferred to transport controls

### Main Playlist View (DONE)
- Track list table with columns: #, status, artist, title, duration
- `PlaylistView` component with sticky header, hover highlight, row selection
- Current track highlighted (purple background, red text, triangle indicator)
- Intro dot indicator (blue) on tracks with `has_intro` flag
- TypeScript types (`types.ts`) matching all IPC response types
- Auto-loads active playlist on mount, playlist tab switching in header
- Right-click context menu for track operations — deferred to later

### Playlist Tabs (DONE)
- Tabbed interface for multiple playlists
- `+` button to add new playlists, `×` button to close, double-click to rename inline
- Tab switching calls `set_active_playlist` to sync backend context
- Auto-selects next tab when closing the currently active one

### Transport Controls
- Play, Stop, Skip Next buttons
- Seek bar with click-to-seek
- Elapsed / remaining time display
- Volume control

### Drag-and-Drop
- Reorder tracks within a playlist via drag-and-drop
- Move/copy tracks between playlist tabs

### File Browser / Add Tracks (DONE)
- Native file dialog via `@tauri-apps/plugin-dialog` (filtered to mp3/wav/flac/ogg/aac/m4a)
- Drag-and-drop files from OS file explorer into playlist (Tauri drag-drop events)
- "Add Files" button in playlist toolbar and empty playlist CTA
- Batch `add_tracks` IPC command for adding multiple files at once

### Now-Playing Display
- Current track artist, title, duration
- Progress bar synced to playback position
- Album art display if embedded metadata contains artwork

### Auto-Intro Dot Indicator (DONE)
- Blue dot (●) in status column on tracks that have a matching intro file in the intros folder
- `get_playlist_tracks` IPC dynamically computes `has_intro` from engine's `intros_folder` config
- `add_track`/`add_tracks` set the flag at insertion; `set_intros_folder` refreshes all tracks

### Crossfade Settings Panel (DONE)
- Configure fade duration (seconds) via modal dialog with numeric input
- Curve type selector (Linear only — backend supports linear; dropdown disabled with "coming soon" hint)
- Gear icon in header bar opens settings modal; loads current value from `get_config`, saves via `set_crossfade`
- Preview/test crossfade behavior — deferred

### Silence Detection Settings (DONE)
- Configure silence threshold (RMS amplitude 0–1) and skip-after duration (seconds) via modal dialog
- Enabled/Disabled status indicator, "Disable" button to quickly turn off
- Accessible from settings gear dropdown in header bar

### Auto-Intro Configuration
- Set intros folder path via folder picker
- Enable/disable auto-intro system
- List detected intro files and their matched artists

### Track Metadata Editor
- Inline editing or modal dialog for artist, title, album fields
- Save changes back to file tags (via lofty through IPC)

### Schedule Side Pane
- Side pane displaying all scheduled events
- Inline editing of schedule entries (time, mode, file, priority)
- Add/remove schedule items from the pane

### Log Pane
- Log output panel underneath the schedule pane
- Shows playback events, schedule triggers, errors, and system messages

### Level Meter
- Real-time audio level visualization (VU or peak meter)
- Stereo L/R display

### Waveform Display
- Waveform overview for the currently playing track
- Playhead position indicator synced to playback

### Settings Config Window
- Centralized settings dialog (modal or dedicated page) for all engine configuration
- Sections: Crossfade, Silence Detection, Auto-Intro, Now-Playing Export, Conflict Policy
- Each section mirrors the existing `set_*` IPC commands with appropriate input controls
- Folder pickers for paths (intros folder, now-playing XML path)
- Numeric inputs with validation for durations, thresholds, volumes
- Dropdowns for enum choices (conflict policy, crossfade curve)
- Save/cancel with immediate backend persistence via existing IPC commands
- Accessible from a toolbar gear icon or menu

### Theme / Dark Mode
- Dark-first UI design suitable for studio environments
- Optional light theme toggle

## Phase F: Future / Long-Term

### Hosted Web Interface
- Browser-based remote control and monitoring dashboard
- Accessible over LAN or internet

### Ad Inserter / Scheduler System
- **AdSchedulerHandler**: Intelligent hourly ad scheduling with lecture detection, track boundary awareness, and safety-margin fallbacks
- **AdInserterService**: MP3 concatenation (pydub-style), RadioBoss URL triggering (schedule + instant modes), XML polling confirmation
- **AdPlayLogger**: Compact JSON play statistics (per-ad, per-date, per-hour), failure tracking (last 50), date-filtered queries
- **AdReportGenerator**: CSV and PDF verified-play reports with hourly/daily breakdowns, multi-ad matrix reports
- **Ad Config UI**: Modal editor for ad CRUD, enable/disable, MP3 file picker, day/hour scheduling, station ID prepend option
- **Ad Statistics UI**: Play calendar with dot indicators, sortable treeview, date filtering, export/report generation, failure viewer
- Reference spec: `skills/specs/ad-inserter-spec.md`

### RDS Engine (Radio Data System)
- **AutoRDSHandler**: RDS message rotation engine with TCP socket protocol (DPSTEXT commands), keepalive resends, configurable rotation timing
- **Message filtering**: Enable/disable, lecture detection (whitelist > blacklist > starts-with-R rule), placeholder availability ({artist}, {title}), day/hour scheduling
- **NowPlayingReader**: Robust XML reader with anti-caching (open+read+fromstring), retry logic, artist polling (wait_for_artist), file change detection
- **LectureDetector**: Track classification (blacklist > whitelist > starts-with-R), current/next track analysis, shared cross-station lists
- **RDS Config UI**: Modal message editor with 64-char limit, duration (1-60s), day/hour scheduling, live treeview updates, per-station state
- Reference spec: `skills/specs/rds-engine-spec.md`

### Advanced Auto Playlist Builder
- Rule-based automatic playlist generation (genre, tempo, artist separation, dayparting, etc.)
