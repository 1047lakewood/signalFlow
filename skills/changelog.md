# signalFlow — Changelog

## 2026-02-06 — Metadata Enhancement
- Filename fallback: parses `Artist - Title.ext` pattern when lofty tags are missing
- Smart fallback logic: uses tag data when available, fills gaps from filename pattern
- Added `played_duration` field to `Track` — records actual playback time per track
- `played_duration` is `Option<Duration>`, defaults to `None`, backward-compatible with existing state files
- `play_playlist()` now returns `PlaybackResult` with `last_index` and per-track `played_durations`
- `playlist show` displays a "Played" column when any track has played_duration data
- Added `played_duration_display()` method to `Track`
- 49 unit tests passing (+7 new: filename parsing, played duration display, serialization)

## 2026-02-06 — Playlist CRUD
- Added `insert_tracks()` to `Playlist` — bulk insert at position or append
- Added `copy_tracks()` to `Engine` — clone tracks from a playlist by indices
- Added `paste_tracks()` to `Engine` — insert tracks into a playlist at position or append
- CLI: `playlist remove <name> <track_numbers...>` — remove tracks by 1-based index (handles descending removal)
- CLI: `playlist move <name> <from> <to>` — reorder a track within a playlist (1-based)
- CLI: `playlist copy <source> <dest> <track_numbers...> [--at <pos>]` — copy tracks between playlists
- CLI: `playlist add` now supports `--at <pos>` for insert-at-position
- 42 unit tests passing (+12 new: 5 insert_tracks, 7 engine copy/paste)

## 2026-02-06 — Silence Detection
- Created `src/silence.rs` — `SilenceDetector<S>` source wrapper + `SilenceMonitor` shared flag
- `SilenceDetector` wraps any `Source<Item=f32>`, measures RMS amplitude per ~100ms window
- Sets `SilenceMonitor` atomic flag when continuous silence exceeds configured duration
- `SilenceMonitor` uses `Arc<AtomicBool>` for lock-free cross-thread signaling
- Engine config: `silence_threshold` (f32, default 0.01) + `silence_duration_secs` (f32, default 0 = disabled)
- Both fields `#[serde(default)]` for backward compat with existing state files
- Player: `play_file_new_sink_monitored()` and `play_file_new_sink_fadein_monitored()` — wrap source with silence detection
- `play_playlist()` now accepts `SilenceConfig` — checks monitor in poll loops, auto-skips on silence
- Silence detection works with both sequential and crossfade playback modes
- CLI: `config silence set <threshold> <duration>` — enable silence detection
- CLI: `config silence off` — disable
- CLI: `play --silence-threshold <f32> --silence-duration <f32>` — per-session override
- `config show` and `status` display silence detection settings
- Created `skills/silence.md` design doc
- 30 unit tests passing (+11 new: 6 silence detector tests, 4 silence config tests, 1 engine serialization test)

## 2026-02-06 — Crossfading
- Dual-sink architecture in `Player` — `create_sink()`, `play_file_new_sink()`, `play_file_new_sink_fadein()`
- `play_playlist()` now accepts `crossfade_secs` parameter for overlapping track transitions
- Fade-in via rodio `Source::fade_in()`, fade-out via linear `Sink::set_volume()` ramp (~50ms steps)
- Edge cases handled: tracks shorter than 2x crossfade skip the fade, last track plays normally, decode errors fall back to sequential
- `Engine.crossfade_secs` persisted config field (`#[serde(default)]` for backward compat)
- CLI: `config crossfade <seconds>` — set default crossfade duration (0 = disabled)
- CLI: `config show` — display current configuration
- CLI: `play --crossfade <seconds>` (`-x`) — override crossfade for this session
- `status` now shows crossfade setting
- Created `skills/crossfade.md` design doc
- 16 unit tests passing (+5 new: `create_sink_works`, `should_crossfade_basic_cases`, `crossfade_secs_defaults_to_zero`, `crossfade_secs_survives_serialization`, `crossfade_secs_defaults_when_missing_from_json`)

## 2026-02-06 — Transport Controls
- Created `src/player.rs` — Player struct wrapping rodio OutputStream + Sink
- Player API: `play_file`, `stop`, `pause`, `resume`, `skip_one`, `try_seek`, `is_empty`, `is_paused`
- `play_playlist()` function — auto-advances through tracks, blocks until done
- CLI `play` command — plays active playlist from current track (or `--track N` for 1-based index)
- CLI `stop` command — resets `current_index` to None (cleared state)
- CLI `skip` command — advances `current_index` to next track and displays info
- `status` command now shows current track info when available
- 11 unit tests passing (added `player_creation_succeeds_or_fails_gracefully`, `play_file_rejects_missing_file`)

## 2026-02-06 — Active Context Switching
- Added `playlist activate <name>` CLI command to set the active playlist
- Added `active_playlist_mut()` on Engine for mutable access to the active context
- 9 unit tests passing (added `active_playlist_mut_allows_modification`)
- Active playlist marked with `*` in `playlist list` output (already worked)

## 2026-02-06 — Multi-Instance Playback
- Added `Track` struct with lofty metadata parsing (`src/track.rs`)
- Added `Playlist` struct with add/remove/reorder (`src/playlist.rs`)
- Added `Engine` struct with JSON persistence, playlist CRUD (`src/engine.rs`)
- CLI commands: `playlist create`, `playlist list`, `playlist add`, `playlist show`
- 8 unit tests passing (engine, playlist, track)
- Created `skills/audio_engine.md` design doc

## 2026-02-06 — Project Initialization
- Initialized Rust project with library/binary split (`src/lib.rs` + `src/main.rs`)
- Added dependencies: rodio, lofty, serde, serde_json, clap
- Created `skills/` directory with `todo.md` and `changelog.md`
- Scaffolded minimal CLI with `status` subcommand via clap
- Added `.cargo/config.toml` for Windows SSL compatibility
- Verified: `cargo check` and `cargo test` pass
