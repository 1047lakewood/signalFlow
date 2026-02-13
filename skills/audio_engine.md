# Audio Engine — Design Doc

> Maintenance note (2026-02-13): treat this as a living design record. Verify behavior against current code and tests before implementation decisions.

## Current State (as of 2026-02-06)

All Phase A and Phase B features are implemented. The engine supports full playlist management, audio playback with crossfading, silence detection, and auto-intros.

### What's Built
- **Track** (`src/track.rs`) — Struct with path, title, artist, duration, played_duration, has_intro. Metadata parsing via lofty with filename fallback (`Artist - Title.mp3` pattern).
- **Playlist** (`src/playlist.rs`) — Struct with id, name, tracks vec, current_index. Full CRUD: add, remove, reorder, insert_tracks.
- **Engine** (`src/engine.rs`) — Struct with playlists, active_playlist_id, next_id, crossfade_secs, silence_threshold, silence_duration_secs, intros_folder. JSON persistence. Playlist CRUD, copy_tracks, paste_tracks.
- **Player** (`src/player.rs`) — Runtime-only struct wrapping rodio `OutputStream` + `OutputStreamHandle` + default `Sink`. Creates independent sinks via `create_sink()` for crossfade and intro playback.
- **Silence** (`src/silence.rs`) — `SilenceDetector` source wrapper + `SilenceMonitor` (Arc<AtomicBool>). RMS-based detection.
- **Auto-Intro** (`src/auto_intro.rs`) — `find_intro()` matches artist name to files in intros folder (case-insensitive).
- **Unified app commands** — transport/playlist/config operations are exposed through `AppCore` and consumed by Tauri IPC handlers.
- **Lib** (`src/lib.rs`) — Re-exports all modules.
- **Tests** — 64 unit tests passing across all modules.

### What's NOT Built Yet
- Scheduler (Phase C) — timed events, overlay/stop/insert modes
- Track metadata editing (Phase D) — write-back to file tags
- Now-Playing XML export (Phase D)
- GUI (Phase E) — Tauri + React

## Data Model

### Track
- `path: PathBuf` — absolute path to audio file
- `title: String` — parsed from metadata or filename
- `artist: String` — parsed from metadata or "Unknown"
- `duration: Duration` — from lofty metadata (calculated)
- `played_duration: Option<Duration>` — actual playback time, set after track finishes
- `has_intro: bool` — whether an intro file exists for this track's artist (default false)

### Playlist
- `id: u32` — unique identifier
- `name: String` — user-facing label (e.g. "Main", "Jingles")
- `tracks: Vec<Track>` — ordered track list
- `current_index: Option<usize>` — which track is selected/playing

Methods: `add_track`, `remove_track`, `reorder`, `insert_tracks`, `track_count`

### Engine
- `playlists: Vec<Playlist>` — all loaded playlists
- `active_playlist_id: Option<u32>` — which playlist is "live"
- `next_id: u32` — auto-increment for playlist IDs
- `crossfade_secs: f32` — crossfade duration (0 = disabled)
- `silence_threshold: f32` — RMS threshold (default 0.01)
- `silence_duration_secs: f32` — seconds before auto-skip (0 = disabled)
- `intros_folder: Option<String>` — path to artist intro files (None = disabled)

Methods: `create_playlist`, `find_playlist`, `find_playlist_mut`, `set_active`, `active_playlist`, `active_playlist_mut`, `copy_tracks`, `paste_tracks`, `load`, `save`

## Persistence
- Engine state serialized to `signalflow_state.json` via serde
- Loaded at app startup, saved after mutations
- Player is NOT serialized (created fresh per audio runtime thread)

## Active Context Switching (DONE)
- `engine.set_active(name)` — sets `active_playlist_id` by name lookup
- `engine.active_playlist()` / `active_playlist_mut()` — returns reference to active playlist
- AppCore: `set_active_playlist(name)` — manually set active playlist, persists to state file

## Transport Controls (DONE)
- **Player** (`src/player.rs`) — runtime-only struct wrapping rodio `OutputStream` + `OutputStreamHandle` + default `Sink`
- `create_sink()` — creates independent sinks on the shared output stream handle
- Methods: `play_file`, `play_file_new_sink`, `play_file_new_sink_fadein`, `play_file_new_sink_monitored`, `play_file_new_sink_fadein_monitored`, `stop`, `pause`, `resume`, `skip_one`, `try_seek`, `is_empty`, `is_paused`
- `play_playlist()` — free function that takes player, tracks, and config; auto-advances through playlist with crossfade + silence detection + auto-intros; blocks until done
- AppCore/Tauri transport commands: play (optional track index), stop (clears `current_index`), skip (advances to next track), pause/resume, seek

## Crossfading (DONE)
- `Player.create_sink()` creates independent sinks from the shared `OutputStreamHandle` — no persistent sink array
- `play_playlist()` manages current + next sinks; when elapsed reaches `track_duration - crossfade_secs`, next track starts on a new sink with `fade_in()`
- Current sink fades out via `set_volume()` ramp (linear, ~50ms steps)
- After crossfade completes, old sink is stopped and dropped; new sink becomes current
- `should_crossfade()` helper handles edge cases: disabled, no next track, track too short (must be > 2x crossfade duration)
- Engine persists `crossfade_secs` config; configurable via AppCore/Tauri settings commands

## Silence Detection (DONE)
- `SilenceDetector<S: Source>` wraps audio source, measures RMS over ~100ms windows
- `SilenceMonitor` (Arc<AtomicBool>) — shared flag checked by playback loop
- When continuous silence exceeds configured duration, flag is set and playback loop auto-skips
- Config: `silence_threshold` (default 0.01) + `silence_duration_secs` (default 0, disabled)
- Configurable via AppCore/Tauri settings commands

## Auto-Intro System (DONE)
- `find_intro(intros_folder, artist)` — case-insensitive filename match for `Artist.*`
- `play_playlist()` plays intro on its own sink before each track; skips intro for consecutive same-artist tracks
- `Track.has_intro: bool` — data flag for future GUI indicator
- Configurable via AppCore/Tauri settings commands
