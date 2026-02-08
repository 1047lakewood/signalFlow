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

### IPC Bridge
- Define Tauri commands exposing all core engine functions to the frontend
- Playlist CRUD, transport controls, scheduler, config read/write
- Event system for engine → frontend updates (track change, position, levels)

### Main Playlist View
- Track list table with columns: artist, title, duration, status
- Highlight currently playing track
- Right-click context menu for track operations (remove, copy, move)

### Playlist Tabs
- Tabbed interface for multiple playlists
- Add, close, rename tabs
- Tab switching sets active playlist context

### Transport Controls
- Play, Stop, Skip Next buttons
- Seek bar with click-to-seek
- Elapsed / remaining time display
- Volume control

### Drag-and-Drop
- Reorder tracks within a playlist via drag-and-drop
- Move/copy tracks between playlist tabs

### File Browser / Add Tracks
- Native file dialog to add audio files
- Drag-and-drop files from OS file explorer into playlist

### Now-Playing Display
- Current track artist, title, duration
- Progress bar synced to playback position
- Album art display if embedded metadata contains artwork

### Auto-Intro Dot Indicator
- Visual dot/icon on tracks that have a matching intro file in the intros folder
- Driven by the `has_intro: bool` flag on track data

### Crossfade Settings Panel
- Configure fade duration (seconds)
- Select crossfade curve type (linear, equal-power, etc.)
- Preview/test crossfade behavior

### Silence Detection Settings
- Configure silence threshold (dB)
- Configure minimum silence duration before auto-skip
- Enable/disable toggle

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

### Theme / Dark Mode
- Dark-first UI design suitable for studio environments
- Optional light theme toggle

## Phase F: Future / Long-Term

### Hosted Web Interface
- Browser-based remote control and monitoring dashboard
- Accessible over LAN or internet

### Ad Scheduler
- Dedicated ad/spot scheduling system with rotation, frequency caps, and reporting

### Advanced Auto Playlist Builder
- Rule-based automatic playlist generation (genre, tempo, artist separation, dayparting, etc.)
