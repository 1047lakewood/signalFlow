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
