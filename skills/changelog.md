# signalFlow — Changelog

## 2026-02-08 — Drag-and-Drop Reordering (GUI)
- Added HTML5 drag-and-drop to `PlaylistView.tsx` for reordering tracks within a playlist
- Drag state tracked via `dragIndex` and `dropTarget` — visual feedback with opacity and drop indicator line
- `handleDragStart` sets drag data and adds `.dragging` class; `handleDrop` calls `onReorder` callback
- `App.tsx` wires `onReorder` to `reorder_track` IPC command (0-based `from`/`to` indices), refreshes track list after reorder
- CSS: `.track-row` gets `cursor: grab`, `.dragging` reduces opacity to 0.4, `.drop-target` shows `--highlight` top border
- 123 unit tests passing (no new tests — frontend-only drag-and-drop wiring over tested `Playlist::reorder` core method)

## 2026-02-08 — Transport Controls (GUI)
- Added `Player` + `PlaybackState` to Tauri `AppState` for runtime audio playback
- Player lazily initialized on first play command — audio output stays alive across tracks
- `PlaybackState` tracks is_playing, is_paused, elapsed time (via `Instant` + pause accounting), track duration
- 6 new IPC commands: `transport_play`, `transport_stop`, `transport_pause`, `transport_skip`, `transport_seek`, `transport_status`
- `transport_play(track_index?)` — plays current or specified track from active playlist, stops any prior playback
- `transport_stop` — stops playback, resets state
- `transport_pause` — toggles pause/resume with accurate elapsed time tracking
- `transport_skip` — advances to next track, plays it (or stops at end of playlist)
- `transport_seek(position_secs)` — seeks to position, resets timing to match
- `transport_status` — returns `TransportState` (is_playing, is_paused, elapsed_secs, duration_secs, track_index, track_artist, track_title), also detects when sink empties (track ended naturally)
- Created `TransportBar.tsx` — Play/Pause toggle, Stop, Skip buttons + seek slider + elapsed/remaining time + track info
- Seek bar uses CSS custom property (`--progress`) for filled track visualization
- Polls `transport_status` every 500ms for real-time elapsed display
- Drag-to-seek on the slider with mousedown/mouseup handling
- `TransportBar` accepts `onTrackChange` callback to refresh playlist view on play/stop/skip
- Added `TransportState` TypeScript interface to `types.ts`
- Transport bar pinned to bottom of window with dark theme matching existing UI
- 123 unit tests passing (no new tests — IPC layer is thin wiring over tested core library)

## 2026-02-08 — Playlist Tabs
- Added `+` button to create new playlists (prompts for name, calls `create_playlist` IPC)
- Added `×` close button on each tab to delete playlists (calls `delete_playlist` IPC)
- Close button hidden by default, appears on tab hover, highlights red on hover
- Double-click a tab to rename inline — input field with Enter to commit, Escape to cancel, blur to commit
- Tab click now also calls `set_active_playlist` to sync backend state
- Add-tab button uses dashed border style to distinguish from playlist tabs
- Rename input styled with highlight border, auto-focused and text-selected on activation
- Auto-selects another tab when closing the currently selected playlist
- 123 unit tests passing (no new tests — frontend-only changes)

## 2026-02-08 — Main Playlist View
- Created `gui/src/types.ts` — TypeScript interfaces matching all IPC response types (PlaylistInfo, TrackInfo, StatusResponse, ScheduleEventInfo, ConfigResponse)
- Created `gui/src/PlaylistView.tsx` — track table component with columns: #, Status, Artist, Title, Duration
- Current track row highlighted with purple background and red accent text, triangle (▶) indicator
- Intro dot (blue ●) displayed on tracks with `has_intro` flag
- Sticky table header, hover highlight on rows, tabular-nums for duration column
- Updated `gui/src/App.tsx` — loads playlists via IPC, auto-selects active playlist, displays track list
- Playlist tab buttons in header for quick switching (preview for full Playlist Tabs feature)
- Empty state messaging for no playlists and empty playlists
- Updated `gui/src/styles.css` — full dark-theme track table styles, playlist tab styles, status indicators
- 123 unit tests passing (no new tests — frontend-only changes)

## 2026-02-08 — IPC Bridge
- Created 20 Tauri IPC commands exposing all core engine functions to the frontend
- Structured JSON response types: `StatusResponse`, `PlaylistInfo`, `TrackInfo`, `ScheduleEventInfo`, `ConfigResponse`
- **Playlist CRUD:** `get_playlists`, `create_playlist`, `delete_playlist`, `rename_playlist`, `set_active_playlist`
- **Track operations:** `get_playlist_tracks`, `add_track`, `remove_tracks` (batch, descending removal), `reorder_track`, `edit_track_metadata`
- **Schedule:** `get_schedule` (sorted by time), `add_schedule_event`, `remove_schedule_event`, `toggle_schedule_event`
- **Config:** `get_config`, `set_crossfade`, `set_silence_detection`, `set_intros_folder` (validates directory), `set_conflict_policy`, `set_nowplaying_path`
- All mutation commands persist state via `Engine::save()` after changes
- Upgraded `get_status` from plain string to structured `StatusResponse` JSON
- 123 unit tests passing (no new tests — IPC layer is thin wiring over tested core library)

## 2026-02-08 — Tauri Project Scaffolding
- Converted to Cargo workspace: root `[workspace]` with `members = ["src-tauri"]`
- Created `src-tauri/` — Tauri v2 backend binary (`signalflow-gui` crate)
- `AppState` wraps `Engine` in `Mutex<Engine>` for thread-safe IPC access
- Initial IPC command: `get_status` — returns engine summary (playlists, active, schedule, crossfade, conflict policy)
- Tauri v2 capabilities: `core:default` permissions for main window
- Created `gui/` — React 19 + TypeScript + Vite 6 frontend
- Dark-first CSS theme with custom properties (`--bg-primary: #1a1a2e`, `--highlight: #e94560`)
- `App.tsx` calls `invoke("get_status")` on mount to display engine status
- Vite dev server on port 1420, build output to `gui/dist/`
- Placeholder ICO icon in `src-tauri/icons/`
- Created `skills/gui.md` design doc
- 123 unit tests passing (no new tests — scaffolding only)

## 2026-02-07 — Now-Playing XML Export
- Created `src/now_playing.rs` — `NowPlaying` struct, `PlaybackState` enum, XML generation
- `NowPlaying::from_engine(engine, elapsed)` — builds snapshot from engine state with optional elapsed time
- `NowPlaying::to_xml()` — renders XML with `<nowplaying>`, `<state>`, `<playlist>`, `<current>` (artist, title, duration, elapsed, remaining), `<next>` (artist, title, duration)
- `NowPlaying::write_xml(path)` — writes XML to a file
- XML escaping for special characters (&, <, >, ", ')
- `PlaybackState` enum: Stopped, Playing (with Display impl)
- Remaining time clamps to zero when elapsed exceeds duration
- Added `Engine.now_playing_path: Option<String>` — persisted config, `#[serde(default)]`
- CLI: `nowplaying [file]` — writes XML snapshot to file (uses config path if no argument)
- CLI: `config nowplaying set <path>` — set default XML output path
- CLI: `config nowplaying off` — disable XML export
- CLI: `config show` now displays now-playing XML path
- 123 unit tests passing (+11 new now_playing tests)

## 2026-02-07 — Track Metadata Editing
- Added `Track::write_tags(artist, title)` — edits in-memory fields and persists to audio file tags via lofty
- Gets or creates the primary tag on the file, sets artist/title via `Accessor` trait, saves with `WriteOptions::default()`
- Added `Engine::edit_track_metadata(playlist, index, artist, title)` — finds the track in a playlist and calls `write_tags`
- CLI: `track edit <playlist> <track_num> [--artist <val>] [--title <val>]` — edit metadata for a track (1-based index)
- Validates: at least one of --artist/--title required, playlist exists, track index in range
- Updates both the engine state file and the audio file's embedded tags
- 112 unit tests passing (+6 new: 3 track write_tags tests, 3 engine edit_track_metadata tests)

## 2026-02-07 — Conflict Resolution
- Added `ConflictPolicy` enum: `ScheduleWins` (default, all events fire) and `ManualWins` (only priority 7+ events fire during manual playback)
- `ConflictPolicy::from_str_loose()` — parse from string (schedule-wins, manual-wins, schedule, manual)
- `ConflictPolicy::manual_override_threshold()` — returns minimum priority for events to fire during manual activity
- Added `Schedule::resolve_time_conflicts(events)` — resolves same-time events, one winner per mode (highest priority wins)
- Added `Schedule::filter_for_manual_playback(events, policy)` — filters events based on conflict policy
- Added `Schedule::events_at_time(time, tolerance_secs)` — query events within a time window
- Added `Engine.conflict_policy: ConflictPolicy` — persisted config, `#[serde(default)]` for backward compat
- CLI: `config conflict <policy>` — set conflict resolution policy
- CLI: `config show` and `status` now display conflict policy
- 106 unit tests passing (+18 new: 12 scheduler conflict tests, 3 engine conflict_policy tests, 2 events_at_time, 1 filter test)

## 2026-02-07 — Insert Mode (Queue Next)
- Added `Engine::insert_next_track(path)` — creates a Track from a file and inserts it after `current_index` in the active playlist (or at position 0 if no current track)
- CLI: `insert <file>` — inserts an audio file as the next track in the active playlist
- Validates file exists before attempting insertion
- Establishes the API for scheduler-driven insert mode when the real-time monitoring loop is built
- 88 unit tests passing (+3 new: insert_next_track_at_beginning_when_no_current, insert_next_track_after_current_index, insert_next_track_no_active_playlist_errors)

## 2026-02-07 — Stop Mode (Interrupt)
- Added `Player::play_stop_mode(path)` — stops the default sink, plays file on a new independent sink, blocks until finished
- CLI: `interrupt <file>` — stops current audio and plays the specified file (hard break / stop mode)
- Validates file exists before playback attempt
- Establishes the API for scheduler-driven stop mode when the real-time monitoring loop is built
- 85 unit tests passing (+1 new: `play_stop_mode_rejects_missing_file`)

## 2026-02-06 — Overlay Mode
- Added `Player::play_overlay(path)` — plays audio file on new independent sink, blocks until finished
- CLI: `overlay <file>` — plays a sound on top of current audio
- Works via OS-level audio mixing (WASAPI shared mode) — run alongside `play` in another terminal
- Validates file exists before playback attempt
- 84 unit tests passing (+1 new: `play_overlay_rejects_missing_file`)

## 2026-02-06 — Scheduler Data Model
- Created `src/scheduler.rs` — `Schedule`, `ScheduleEvent`, `ScheduleMode`, `Priority` types
- `ScheduleMode` enum: Overlay (play on top), Stop (kill + play), Insert (queue next)
- `ScheduleEvent` struct: id, time (NaiveTime), mode, file, priority(1-9), enabled, label, days
- `Schedule` struct: CRUD (`add_event`, `remove_event`, `find_event`, `toggle_event`), `events_by_time()` sorted view
- `Priority` type with constants: LOW(1), NORMAL(5), HIGH(9)
- `parse_time()` utility: accepts HH:MM or HH:MM:SS formats
- Days field: `Vec<u8>` (0=Mon..6=Sun), empty = daily recurrence
- Added `Engine.schedule: Schedule` — `#[serde(default)]` for backward compat
- CLI: `schedule add <time> <mode> <file> [-p priority] [-l label] [-d days]`
- CLI: `schedule list` — sorted by time, shows mode/priority/status/days
- CLI: `schedule remove <id>` — remove event by ID
- CLI: `schedule toggle <id>` — enable/disable event
- `status` command now shows schedule event count
- Added `chrono` dependency for time handling
- Created `skills/scheduler.md` design doc
- 83 unit tests passing (+19 new scheduler tests)

## 2026-02-06 — Auto-Intro System
- Created `src/auto_intro.rs` — `find_intro()` and `has_intro()` functions
- `find_intro()` scans intros folder for `Artist.*` files (case-insensitive, supports mp3/wav/flac/ogg/aac/m4a)
- Skips "Unknown" artists and empty strings
- Added `Engine.intros_folder: Option<String>` — persisted config, `#[serde(default)]`
- Added `Track.has_intro: bool` — data flag for future GUI dot indicator, `#[serde(default)]`
- `play_playlist()` now accepts `intros_folder: Option<&Path>` parameter
- Plays artist intro before each track when configured; skips for consecutive same-artist tracks
- CLI: `config intros set <path>` — set intros folder (validates directory exists)
- CLI: `config intros off` — disable auto-intros
- CLI: `config show` displays intros folder setting
- CLI: `status` displays intros setting
- CLI: `play` output shows "auto-intros: on" when enabled
- Created `skills/auto_intro.md` design doc
- 64 unit tests passing (+15 new: 9 auto_intro, 3 engine intros_folder, 3 track has_intro)

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
