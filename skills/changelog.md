# signalFlow — Changelog

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
