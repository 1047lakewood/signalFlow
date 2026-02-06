# Audio Engine — Design Doc

## Current State (as of 2026-02-06)

### What's Built
- **Track** (`src/track.rs`) — Struct with path, title, artist, duration. Metadata parsing via lofty with filename fallback.
- **Playlist** (`src/playlist.rs`) — Struct with id, name, tracks vec, current_index. Add track support.
- **Engine** (`src/engine.rs`) — Struct with playlists vec, active_playlist_id, next_id. JSON persistence to `signalflow_state.json`. Playlist CRUD (create, list, add tracks, show).
- **CLI** (`src/main.rs`) — clap subcommands: `playlist create <name>`, `playlist list`, `playlist add <playlist> <file>...`, `playlist show <playlist>`, `status`.
- **Lib** (`src/lib.rs`) — Re-exports all modules.
- **Tests** — 8 unit tests passing across engine, playlist, and track modules.

### What's NOT Built Yet
- No actual audio playback (rodio sinks not wired up)
- No active context switching
- No transport controls (play/stop/skip/seek)
- No crossfading
- No silence detection

## Data Model

### Track
- `path: PathBuf` — absolute path to audio file
- `title: String` — parsed from metadata or filename
- `artist: String` — parsed from metadata or "Unknown"
- `duration: Duration` — from lofty metadata

### Playlist
- `id: u32` — unique identifier
- `name: String` — user-facing label (e.g. "Main", "Jingles")
- `tracks: Vec<Track>` — ordered track list
- `current_index: Option<usize>` — which track is selected/playing

### Engine
- `playlists: Vec<Playlist>` — all loaded playlists
- `active_playlist_id: Option<u32>` — which playlist is "live"
- `next_id: u32` — auto-increment for playlist IDs

## Persistence
- Engine state serialized to `signalflow_state.json` via serde
- Loaded on CLI startup, saved after mutations

## Active Context Switching (DONE)
- `engine.set_active(name)` — sets `active_playlist_id` by name lookup
- `engine.active_playlist()` — returns immutable reference to the active playlist
- `engine.active_playlist_mut()` — returns mutable reference to the active playlist
- CLI: `playlist activate <name>` — manually set active playlist, persists to state file
- Playing a track (once transport is built) will auto-activate its playlist

## Transport Controls (DONE)
- **Player** (`src/player.rs`) — runtime-only struct wrapping rodio `OutputStream` + `Sink`
- Methods: `play_file`, `stop`, `pause`, `resume`, `skip_one`, `try_seek`, `is_empty`, `is_paused`
- `play_playlist()` — takes a slice of tracks and start index, auto-advances through playlist, blocks until done
- Player is NOT serialized (created fresh per CLI session)
- CLI: `play [--track N]` — plays active playlist, blocks in foreground
- CLI: `stop` — clears `current_index` (no persistent playback in CLI mode)
- CLI: `skip` — advances `current_index` to next track
- `status` shows current track info when set

## Crossfading (DONE)
- Dual-sink approach: `Player` creates independent sinks via `create_sink()` from the shared `OutputStreamHandle`
- `play_playlist()` manages two sinks — active track and standby (next track)
- When elapsed time reaches `track_duration - crossfade_secs`, next track starts on a new sink with `fade_in()`
- Current sink fades out via `set_volume()` ramp (linear, ~50ms steps)
- After crossfade completes, old sink is stopped and dropped; new sink becomes current
- `should_crossfade()` helper handles edge cases: disabled, no next track, track too short
- Engine persists `crossfade_secs` config; CLI exposes `config crossfade` and `play --crossfade`

## Next Up: Silence Detection
Auto-skip when signal drops below threshold for X seconds.
