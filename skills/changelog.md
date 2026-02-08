# signalFlow — Changelog

## 2026-02-08 — Ad Inserter Service
- Created `src/ad_inserter.rs` — stateless ad insertion service with instant and scheduled modes
- `AdInsertionResult` struct: ad_count, ads_inserted (names), station_id_played
- `AdInserterService` — stateless struct with static methods taking engine/player as parameters
- `collect_valid_ads(ads)` — filters ads by enabled, file exists, and current day/hour schedule
- `collect_valid_ads_at(ads, day, hour)` — testable variant with explicit time parameters
- `insert_instant(player, engine, is_hour_start)` — creates new sink, appends station ID (if applicable) + all valid ads, blocks until finished
- `insert_scheduled(engine, is_hour_start)` — inserts valid ads as next tracks in active playlist via `insert_next_track()` in reverse order for correct playback sequence
- `run_insertion(player, engine, mode, is_hour_start)` — dispatches to instant or scheduled based on `AdInsertionMode`
- Station ID prepended at hour start when `station_id_enabled=true` and file exists
- `append_to_sink(sink, path)` helper — decodes audio file and appends to rodio sink
- Engine: added `current_track_path()` — returns path of currently playing track from active playlist
- CLI: `ad insert-instant` — manually trigger instant ad insertion (creates Player, plays all valid ads)
- CLI: `ad insert-scheduled` — manually trigger scheduled insertion (queues ads as next tracks, saves state)
- 183 unit tests passing (+12 new: 4 collect_valid_ads tests, 4 insert_scheduled tests, 2 AdInsertionResult tests, 1 run_insertion dispatch, 1 station_id skip test)

## 2026-02-08 — Ad Scheduler Handler
- Created `src/ad_scheduler.rs` — ad configuration data model, scheduling decision logic, and background handler
- `AdConfig` struct: name, enabled, mp3_file, scheduled (bool), days (Vec<String>), hours (Vec<u8>)
- `AdConfig::is_scheduled_for(day, hour)` — schedule matching with day/hour filters
- `AdConfig::is_valid_now(day, hour)` — checks enabled, file exists, and schedule match
- `AdInserterSettings` struct: output_mp3 path, station_id_enabled, station_id_file
- `AdInsertionMode` enum: Scheduled (wait for track boundary) vs Instant (interrupt immediately)
- `SchedulerDecision` enum: InsertInstant, InsertScheduled, WaitForTrackBoundary, Skip
- `decide_ad_insertion()` — pure decision function implementing the full lecture check flow (CHECK 0–3)
  - CHECK 0: Skip if no next track (playlist ended)
  - CHECK 1: Instant if < 3 min safety margin
  - CHECK 2: Instant if current track extends past hour
  - CHECK 3: Lecture-aware boundary detection (scheduled vs instant vs wait)
- Time calculation helpers: `minutes_remaining_in_hour()`, `seconds_until_next_hour()`, `track_ends_this_hour()`, `minutes_remaining_after_track()`, `is_hour_start()`, `current_day_name()`, `current_hour()`
- `AdSchedulerHandler` — background thread with hour boundary checks, track change detection (5s poll), dynamic sleep
- Created `src/lecture_detector.rs` — `LectureDetector` with blacklist > whitelist > starts-with-'R' classification
- `LectureDetector::is_lecture(artist)` — priority: empty=false, blacklist=false, whitelist=true, starts_with_r=true, else=false
- `add_blacklist/remove_blacklist/add_whitelist/remove_whitelist` methods for list management
- Engine: added `ads: Vec<AdConfig>`, `ad_inserter: AdInserterSettings`, `lecture_detector: LectureDetector` fields
- Engine: added `add_ad()`, `remove_ad()`, `find_ad()`, `toggle_ad()`, `current_track_info()`, `next_track_artist()`, `has_next_track()` methods
- CLI: `ad add <name> <file> [--scheduled] [--days "Mon,Fri"] [--hours "9,10,14"]` — add ad config
- CLI: `ad list` — show all ads with status, schedule, and file info
- CLI: `ad remove <num>` — remove ad by 1-based number
- CLI: `ad toggle <num>` — enable/disable ad
- CLI: `ad show <num>` — show ad details
- CLI: `config ad-inserter output <path>` — set concatenated ad roll output path
- CLI: `config ad-inserter station-id set <file>` / `off` — configure station ID
- CLI: `config lecture blacklist-add/blacklist-remove/whitelist-add/whitelist-remove <artist>` — manage lecture lists
- CLI: `config lecture show` — display blacklist/whitelist contents
- CLI: `config lecture test <artist>` — test classification result
- CLI: `config show` — now includes ads count, ad output path, station ID, lecture detector stats
- CLI: `status` — now shows enabled/total ad counts
- 171 unit tests passing (+34 new: 10 AdConfig, 2 AdInserterSettings, 4 time calc, 10 decision logic, 1 handler, 9 LectureDetector)

## 2026-02-08 — Theme / Dark Mode (GUI)
- Added light theme via `[data-theme="light"]` CSS custom properties alongside existing dark theme
- Dark theme remains default; light theme uses studio-friendly muted colors (#f0f0f5 bg, #d63050 highlight)
- Theme toggle button (sun/moon icon) added to header bar between schedule and settings buttons
- Theme preference persisted to `localStorage` (`signalflow-theme` key), restored on app load
- `document.documentElement` `data-theme` attribute drives all theme switching
- Replaced hardcoded overlay/shadow rgba values with `--overlay-bg` and `--shadow` CSS variables
- `WaveformDisplay.tsx` canvas rendering now reads `--highlight`, `--border`, `--text-primary` from computed styles instead of hardcoded hex colors
- Added `.header-theme-btn` CSS class matching existing header button styles
- 137 unit tests passing (no new tests — frontend-only CSS variable + state changes)

## 2026-02-08 — Settings Config Window (GUI)
- Created `SettingsWindow.tsx` — centralized tabbed settings dialog replacing three separate modals
- Tabbed sidebar navigation with 5 sections: Crossfade, Silence Detection, Auto-Intro, Now-Playing XML, Conflict Policy
- Crossfade tab: fade duration (0–30s) and curve type selector (linear only)
- Silence Detection tab: threshold (0–1), skip duration (0–300s), enable/disable status, Disable button
- Auto-Intro tab: folder browser, recurring interval (0–3600s), duck volume (0–1), enable/disable status, Disable button
- Now-Playing XML tab: file path browser (XML filter), enable/disable status, Disable button — previously had no GUI
- Conflict Policy tab: schedule-wins / manual-wins select with dynamic hint text — previously had no GUI
- Config loaded once on mount, shared across all tabs; Save button dispatches per-tab IPC commands
- Gear icon (⚙) now directly opens the settings window instead of a dropdown menu
- Deleted `CrossfadeSettings.tsx`, `SilenceSettings.tsx`, `IntroSettings.tsx` — consolidated into SettingsWindow
- Removed settings dropdown menu code from App.tsx and its CSS (`.settings-menu-wrapper`, `.settings-dropdown`, `.settings-dropdown-item`)
- Added CSS: `.settings-window` (560px wide), `.settings-window-body`, `.settings-tabs` (160px sidebar), `.settings-tab` (with active highlight), `.settings-content`
- 137 unit tests passing (no new tests — frontend-only changes over existing IPC commands)

## 2026-02-08 — Waveform Display (GUI)
- Created `src/waveform.rs` — `generate_peaks(path, num_peaks)` and `generate_peaks_default(path)` functions
- Decodes audio file via rodio, collects all samples, computes max absolute amplitude per time bucket
- Normalizes peaks so loudest bucket = 1.0, returns `Vec<f32>` of 200 values (configurable)
- Added `get_waveform` IPC command in Tauri backend — takes file path, returns peak data
- Extended `TransportState` with `track_path: Option<String>` field (Rust + TypeScript) for waveform loading
- Created `WaveformDisplay.tsx` — canvas-based waveform visualization in transport bar
- Canvas renders mirrored bars (top/bottom of center line) with played portion in highlight red, unplayed in dark gray
- Playhead rendered as white vertical line synced to elapsed/duration ratio
- Click-to-seek on the waveform replaces the old range slider for seeking
- Waveform data fetched once per track change (cached until track path changes)
- DPI-aware canvas rendering via `devicePixelRatio` scaling
- CLI: `waveform <file> [-p peaks]` — generates ASCII waveform visualization (every 10th peak printed)
- CSS: `.waveform-display` (flex: 1, 36px height, rounded, clickable), `.waveform-canvas`
- Removed old seek slider HTML and associated `isSeeking`/`seekValue` state from `TransportBar`
- 137 unit tests passing (+2 new: generate_peaks_rejects_missing_file, default_peaks_count)

## 2026-02-08 — Level Meter (GUI)
- Created `src/level_monitor.rs` — `LevelMonitor` (shared `Arc<AtomicU32>` storing f32 RMS as bits) and `LevelSource<S>` source wrapper
- `LevelSource` computes RMS over ~50ms windows, updates `LevelMonitor` atomically from the audio thread
- `LevelMonitor` API: `level()` (get current RMS), `reset()` (zero on stop), `new()` (constructor)
- Added `Player::play_file_with_level()` — wraps decoded source with `LevelSource` for level-monitored playback
- `LevelMonitor` stored in Tauri `AppState`, used by `transport_play` (level-monitored) and reset on `transport_stop`
- `get_audio_level` IPC command returns current f32 RMS level
- Created `LevelMeter.tsx` — horizontal bar with dB-scaled fill and peak hold indicator
- RMS → dB conversion (20*log10), mapped from -60dB..0dB to 0..100% width
- Peak hold with ~1 second hold time, then gradual decay
- Green→yellow→red gradient fill via CSS `linear-gradient`
- Peak indicator as 2px white marker line
- Polls `get_audio_level` every 60ms when playing, stops when paused/stopped
- Level meter placed in transport bar between seek slider and "Next up" panel
- CSS: `.level-meter` (80px fixed width), `.level-meter-track`, `.level-meter-fill`, `.level-meter-peak`
- 135 unit tests passing (+6 new: monitor_starts_at_zero, monitor_reset_sets_zero, level_source_passes_samples_unchanged, level_source_measures_loud_audio, level_source_measures_silence, level_source_preserves_source_properties)

## 2026-02-08 — Log Pane (GUI)
- Created `LogPane.tsx` — scrollable log list displayed underneath the schedule pane in the right side panel
- Refactored side pane layout: new `.side-pane` wrapper contains `SchedulePane` (top) + `LogPane` (bottom)
- Added in-memory `LogBuffer` (ring buffer, 500 entries max) to Tauri `AppState` using `VecDeque<LogEntry>`
- `LogEntry` struct with timestamp (HH:MM:SS via chrono), level (info/warn/error), message
- Playback events logged from transport IPC commands: play (artist — title), stop, pause, resume, skip, end-of-playlist
- Schedule events logged: event added (label/file + time)
- `get_logs` IPC command retrieves all entries (supports optional `since_index` for incremental fetch)
- `clear_logs` IPC command empties the buffer
- Frontend polls every 1 second, auto-scrolls to bottom (disabled when user scrolls up)
- "Clear" button in log header to empty the log display
- Monospace font (Consolas), color-coded levels: info=blue, warn=orange, error=red
- Added `chrono` dependency to `src-tauri/Cargo.toml` for timestamp formatting
- Added `LogEntry` interface to `types.ts`
- 129 unit tests passing (no new tests — frontend-only changes + thin IPC over in-memory buffer)

## 2026-02-08 — Schedule Side Pane (GUI)
- Created `SchedulePane.tsx` — collapsible side panel displaying all scheduled events
- Events listed by time with colored mode badges (overlay=blue, stop=red, insert=green), priority, days, and label/filename
- ON/OFF toggle button per event calls `toggle_schedule_event` IPC
- Remove button (×) per event calls `remove_schedule_event` IPC
- "+" button opens inline add form with: time input, mode selector, file browser (native dialog), priority (1–9), optional label, day-of-week toggle buttons
- Add form validates required fields (time, file) and calls `add_schedule_event` IPC
- Clock icon (⏰) in header bar toggles the schedule pane open/closed, highlighted when active
- Main content layout refactored to flexbox with `.main-content` wrapper for side-by-side playlist + schedule
- `.playlist-area` takes remaining space, `.schedule-pane` fixed at 320px width
- Disabled events shown at 50% opacity
- Full dark-theme CSS matching existing design: event hover, mode color coding, day toggle buttons
- 129 unit tests passing (no new tests — frontend-only changes over existing IPC commands)

## 2026-02-08 — Track Metadata Editor (GUI)
- Double-click Artist or Title cell in playlist view to inline edit
- Input field replaces cell text with current value pre-selected, highlight border
- Enter commits edit via `edit_track_metadata` IPC (persists to audio file tags + engine state)
- Escape cancels edit; blur commits if value changed, cancels if unchanged
- Dragging disabled on rows while editing to prevent conflicts
- Added `onTracksChanged` prop to `PlaylistView` — refreshes track list after edit
- Added `.editable-cell` (cursor: text on hover) and `.cell-edit-input` CSS classes
- 129 unit tests passing (no new tests — frontend-only changes over existing `edit_track_metadata` IPC)

## 2026-02-08 — Auto-Intro Config (GUI)
- Created `IntroSettings.tsx` — modal dialog for configuring auto-intro system
- Browse button opens native directory picker via `@tauri-apps/plugin-dialog` to select intros folder
- Read-only path display showing current intros folder, with placeholder when no folder selected
- Enabled/Disabled status indicator (green/gray) computed from folder presence
- Recurring intro interval input (seconds, 0 = disabled) with dynamic hint text
- Duck volume input (0–1) for main track volume during recurring intro overlay
- "Disable" button (red) clears intros folder and resets recurring settings
- Saves via `set_intros_folder` and `set_recurring_intro` IPC commands with "Saved!" feedback
- Added "Auto-Intro" item to settings dropdown menu in header
- Added `.settings-input-path` and `.settings-btn-browse` CSS classes
- Updated TypeScript `ConfigResponse` to include `recurring_intro_interval_secs` and `recurring_intro_duck_volume`
- 129 unit tests passing (no new tests — frontend-only changes over existing IPC commands)

## 2026-02-08 — Silence Detection Settings (GUI)
- Created `SilenceSettings.tsx` — modal dialog for configuring silence detection threshold and skip duration
- Numeric input for silence threshold (RMS amplitude 0–1, step 0.005) with hint text
- Numeric input for skip-after duration (seconds, 0 = disabled)
- Enabled/Disabled status indicator (green/gray) computed from current values
- "Disable" button (red) to quickly zero out both fields
- Saves via existing `set_silence_detection` IPC command with "Saved!" feedback
- Refactored gear icon (`⚙`) into a dropdown settings menu with "Crossfade" and "Silence Detection" items
- `.settings-menu-wrapper` with click-outside dismiss, `.settings-dropdown` and `.settings-dropdown-item` CSS
- Added reusable `.settings-status`, `.status-enabled`/`.status-disabled`, `.settings-btn-danger` CSS classes
- 129 unit tests passing (no new tests — frontend-only changes over existing `set_silence_detection` IPC)

## 2026-02-08 — Crossfade Settings Panel (GUI)
- Created `CrossfadeSettings.tsx` — modal dialog for configuring crossfade duration
- Numeric input for fade duration (0–30 seconds, step 0.5), loads current value from `get_config` IPC on mount
- Saves via existing `set_crossfade` IPC command with "Saved!" feedback
- Curve type selector (dropdown with "Linear" only — backend supports linear only; disabled with "coming soon" hint)
- Gear icon button (`⚙`) in the header bar opens the settings modal, click-outside or close button dismisses
- Full dark-theme CSS: overlay backdrop, panel with header/body/footer, input/select/button styles matching existing design
- Reusable `.settings-*` CSS classes ready for future settings panels (silence detection, auto-intro, etc.)
- 129 unit tests passing (no new tests — frontend-only changes over existing `set_crossfade` IPC)

## 2026-02-08 — Recurring Intro Overlay
- Added `RecurringIntroConfig` struct in `player.rs` with `interval_secs` and `duck_volume` fields
- `play_playlist()` now accepts `RecurringIntroConfig` parameter; checks elapsed time in both crossfade and sequential wait loops
- `maybe_play_recurring_intro()` helper: finds intro, plays on overlay sink, ducks main sink volume during playback, restores after
- Timer resets per track — each track gets its own recurring intro cycle
- Added `Engine.recurring_intro_interval_secs` (f32, default 0 = disabled) and `Engine.recurring_intro_duck_volume` (f32, default 0.3)
- Both fields `#[serde(default)]` for backward compat
- CLI: `config intros recurring set <interval> [--duck <vol>]` — enable with configurable interval and duck volume
- CLI: `config intros recurring off` — disable recurring intros
- CLI: `config show` and `play` output display recurring intro settings when enabled
- IPC: `set_recurring_intro(interval_secs, duck_volume)` Tauri command added
- IPC: `get_config` and `get_status` responses include `recurring_intro_interval_secs` and `recurring_intro_duck_volume`
- 129 unit tests passing (+6 new: 3 engine config tests, 3 RecurringIntroConfig tests)

## 2026-02-08 — Auto-Intro Dot Indicator (GUI)
- `get_playlist_tracks` IPC now dynamically computes `has_intro` by checking each track's artist against the engine's configured `intros_folder` via `auto_intro::has_intro()`
- `add_track` and `add_tracks` IPC commands set `has_intro` on newly added tracks at insertion time
- `set_intros_folder` IPC refreshes `has_intro` flags on all tracks in all playlists when the intros folder changes or is disabled
- Frontend rendering (blue ● dot in status column) and CSS (`.intro-dot`) were already in place from the Main Playlist View feature
- 123 unit tests passing (no new tests — IPC-layer changes over tested core `auto_intro::has_intro()`)

## 2026-02-08 — Now-Playing Display (GUI)
- Enhanced `TransportBar` into a full now-playing display with separate title (bold, 13px) and artist (11px, secondary color) lines
- Added "Next up" panel showing the upcoming track's artist and title
- Extended `TransportState` with `next_artist` and `next_title` fields (Rust backend + TypeScript interface)
- `transport_status` IPC now fetches next track info from the playlist (current index + 1)
- Empty states: "No track loaded" when idle, dash when no next track
- New CSS classes: `.now-playing-panel`, `.now-playing-title`, `.now-playing-artist`, `.now-playing-next`, `.next-label`, `.next-track`
- No album art per user specification
- 123 unit tests passing (no new tests — frontend-only display changes + thin IPC field addition)

## 2026-02-08 — File Browser / Add Tracks (GUI)
- Added `@tauri-apps/plugin-dialog` (JS) and `tauri-plugin-dialog` (Rust) for native file picker
- Dialog plugin registered in Tauri builder, `dialog:default` + `dialog:allow-open` capabilities added
- "Add Files" button opens native file dialog filtered to audio files (mp3, wav, flac, ogg, aac, m4a)
- New `add_tracks` IPC command — batch adds multiple file paths to a playlist in one call
- OS drag-and-drop support via Tauri `tauri://drag-drop`, `tauri://drag-enter`, `tauri://drag-leave` events
- Drop zone visual feedback: dashed highlight outline on `.playlist-view` / `.playlist-empty`, overlay text during hover
- Dropped files filtered by audio extension (non-audio files silently skipped)
- Empty playlist state shows "Add Files" CTA button + "or drag audio files here" hint
- `PlaylistView` accepts `onAddFiles` and `onFileDrop` callbacks, "+" toolbar button at bottom of track list
- Added `SendPlayer` wrapper with `unsafe impl Send/Sync` for `Player` to satisfy Tauri's `State<T: Send + Sync>` requirement
- Fixed Rust 2024 edition `ref` binding compatibility in `transport_status`
- 123 unit tests passing (no new tests — frontend-only changes + thin IPC wiring over tested core methods)

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
