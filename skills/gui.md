# signalFlow — GUI (Tauri) Design Doc

## Architecture

```
signalFlow/
├── src/                   # Core Rust library (signal_flow crate)
├── src/main.rs            # CLI binary
├── src-tauri/             # Tauri v2 backend (signalflow-gui crate)
│   ├── Cargo.toml         # Depends on signal_flow via path = ".."
│   ├── build.rs           # tauri_build::build()
│   ├── tauri.conf.json    # Tauri config (window, build, bundle)
│   ├── capabilities/      # Tauri v2 permission capabilities
│   │   └── default.json   # Core permissions for main window
│   ├── icons/
│   │   └── icon.ico       # App icon (placeholder)
│   └── src/
│       └── main.rs        # Tauri app entry, AppState, IPC commands
├── gui/                   # React + TypeScript frontend
│   ├── package.json       # Dependencies (@tauri-apps/api, react, vite)
│   ├── vite.config.ts     # Vite dev server on port 1420
│   ├── tsconfig.json      # Strict TS config
│   ├── index.html         # Entry HTML
│   └── src/
│       ├── main.tsx       # React root mount
│       ├── App.tsx        # Main app component
│       └── styles.css     # Dark-first theme
└── Cargo.toml             # Workspace root (members: src-tauri)
```

## Workspace Setup (DONE)

- Root `Cargo.toml` defines `[workspace]` with `members = ["src-tauri"]`
- Core library (`signal_flow`) remains at root, unchanged
- Tauri binary (`signalflow-gui`) in `src-tauri/` depends on `signal_flow` via path

## Tauri Backend (DONE)

- `AppState` wraps `Engine` in `Mutex` for thread-safe IPC access
- `Engine::load()` called at startup to restore persisted state
- Full IPC command layer exposing all core engine functions (see IPC Commands table)
- Tauri v2 with capabilities-based permissions (`core:default`)
- All commands return structured serde-serializable JSON (not formatted strings)

## Frontend (DONE)

- React 19 + TypeScript + Vite 6
- Dark-first theme (CSS custom properties): `--bg-primary: #1a1a2e`, `--highlight: #e94560`
- `@tauri-apps/api` for IPC via `invoke()`
- Dev server: `http://localhost:1420`
- Build output: `gui/dist/`

## Running

- Dev mode: `cd src-tauri && cargo tauri dev` (starts Vite + Tauri together)
- Build: `cd src-tauri && cargo tauri build`
- Frontend only: `cd gui && npm run dev`

## IPC Commands (DONE)

| Command | Args | Returns | Status |
|---------|------|---------|--------|
| **Status** | | | |
| `get_status` | none | `StatusResponse` (JSON) | DONE |
| **Playlist CRUD** | | | |
| `get_playlists` | none | `Vec<PlaylistInfo>` | DONE |
| `create_playlist` | `name` | `u32` (id) | DONE |
| `delete_playlist` | `name` | `()` | DONE |
| `rename_playlist` | `old_name, new_name` | `()` | DONE |
| `set_active_playlist` | `name` | `u32` (id) | DONE |
| **Track Operations** | | | |
| `get_playlist_tracks` | `name` | `Vec<TrackInfo>` | DONE |
| `add_track` | `playlist, path` | `usize` (index) | DONE |
| `remove_tracks` | `playlist, indices[]` | `()` | DONE |
| `reorder_track` | `playlist, from, to` | `()` | DONE |
| `edit_track_metadata` | `playlist, track_index, artist?, title?` | `()` | DONE |
| **Schedule** | | | |
| `get_schedule` | none | `Vec<ScheduleEventInfo>` | DONE |
| `add_schedule_event` | `time, mode, file, priority?, label?, days?` | `u32` (id) | DONE |
| `remove_schedule_event` | `id` | `()` | DONE |
| `toggle_schedule_event` | `id` | `bool` (new state) | DONE |
| **Config** | | | |
| `get_config` | none | `ConfigResponse` | DONE |
| `set_crossfade` | `secs` | `()` | DONE |
| `set_silence_detection` | `threshold, duration_secs` | `()` | DONE |
| `set_intros_folder` | `path?` (None=disable) | `()` | DONE |
| `set_conflict_policy` | `policy` | `()` | DONE |
| `set_nowplaying_path` | `path?` (None=disable) | `()` | DONE |

### Response Types

- **StatusResponse**: playlist_count, active_playlist, schedule_event_count, crossfade_secs, conflict_policy, silence_threshold, silence_duration_secs, intros_folder, now_playing_path
- **PlaylistInfo**: id, name, track_count, is_active, current_index
- **TrackInfo**: index, path, title, artist, duration_secs, duration_display, played_duration_secs, has_intro
- **ScheduleEventInfo**: id, time, mode, file, priority, enabled, label, days
- **ConfigResponse**: crossfade_secs, silence_threshold, silence_duration_secs, intros_folder, conflict_policy, now_playing_path

## Main Playlist View (DONE)

- `PlaylistView` component: table with columns #, Status, Artist, Title, Duration
- Current track highlighted with `--bg-row-current` background and `--highlight` text color
- Playing indicator (triangle) on current track, intro dot (blue) on tracks with `has_intro`
- Sticky header, hover highlight, tabular-nums for duration column
- `types.ts`: TypeScript interfaces matching all IPC response types
- `App.tsx`: loads playlists via `get_playlists`, auto-selects active playlist, loads tracks via `get_playlist_tracks`
- Playlist tabs in header for quick switching between playlists (preview for Playlist Tabs feature)
- Empty state messaging for no playlists and empty playlists

## Playlist Tabs (DONE)

- `+` button creates new playlists via `create_playlist` IPC (browser `prompt()` for name)
- `×` close button on each tab deletes playlists via `delete_playlist` IPC (hidden until hover)
- Double-click tab to rename inline — commits on Enter/blur, cancels on Escape, calls `rename_playlist` IPC
- Tab click calls `set_active_playlist` to sync backend active context
- Auto-selects next available tab when closing the currently selected playlist

## Transport Controls (DONE)

- `Player` stored in `AppState` behind `Mutex<Option<Player>>`, lazily initialized on first play
- `PlaybackState` struct tracks: is_playing, is_paused, track_index, playlist_name, track_duration, start_time (Instant), total_paused (Duration), pause_start
- `PlaybackState::elapsed()` calculates accurate elapsed time accounting for pauses
- 6 IPC commands: `transport_play`, `transport_stop`, `transport_pause`, `transport_skip`, `transport_seek`, `transport_status`
- `TransportState` response: is_playing, is_paused, elapsed_secs, duration_secs, track_index, track_artist, track_title
- `transport_status` detects when rodio sink empties (track ended naturally)
- `TransportBar.tsx` component: Play/Pause toggle, Stop, Skip buttons, seek slider with filled progress, elapsed/remaining time, current track artist/title
- Polls every 500ms, seek via drag on range input
- Pinned to bottom of `.app` layout

### IPC Commands (Transport)

| Command | Args | Returns | Status |
|---------|------|---------|--------|
| `transport_play` | `track_index?` | `()` | DONE |
| `transport_stop` | none | `()` | DONE |
| `transport_pause` | none | `()` | DONE |
| `transport_skip` | none | `()` | DONE |
| `transport_seek` | `position_secs` | `()` | DONE |
| `transport_status` | none | `TransportState` | DONE |

## Next Steps

- [ ] Drag-and-drop reordering — Reorder tracks within and between playlists
