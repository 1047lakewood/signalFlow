# Unified App Architecture — Design Doc

## Status: DESIGN PHASE

## Problem Statement

signalFlow currently has two separate entry points:
- **CLI binary** (`src/main.rs`) — ~1,948 lines, ~30 commands, synchronous blocking playback
- **Tauri binary** (`src-tauri/src/main.rs`) — ~1,314 lines, ~42 IPC handlers, Mutex-based state

Both duplicate command logic (validation, state mutation, save) with different patterns:
- CLI: load from CWD, create ephemeral Player per command, block until done
- Tauri: load from app data dir, persistent Player in Mutex, poll for status

### Pain Points
1. **Duplicated command logic** — same validation/mutation in both CLI and Tauri
2. **Mutex deadlock risk** — 3 separate Mutexes (engine, player, playback) with strict ordering rules
3. **Polling overhead** — GUI polls `transport_status` every 500ms instead of receiving events
4. **No crossfade/auto-advance in GUI** — `play_playlist()` is CLI-only (blocks thread)
5. **Unsafe Send impl** — `SendPlayer` uses `unsafe impl Send/Sync` to satisfy Tauri state requirements
6. **File-based state sharing** — CLI and Tauri could race on `signalflow_state.json`

## Goal

Merge CLI+GUI into a single Tauri app while:
- Preserving **full testability** via `cargo test` (no GUI needed)
- Eliminating duplicated command logic
- Replacing Mutex polling with an event-driven architecture
- Enabling full playlist playback (crossfade, auto-advance, silence skip) in the GUI

## Architecture Overview

### Layer Diagram

```
┌─────────────────────────────────────────────┐
│  GUI (React/TypeScript)                     │
│  Tauri IPC ←→ thin invoke wrappers          │
└────────────────────┬────────────────────────┘
                     │ Tauri commands (thin)
┌────────────────────┴────────────────────────┐
│  AppCore  (owns Engine + AudioRuntime)      │
│  - Central command dispatcher               │
│  - Event bus (state changes → subscribers)  │
│  - Single-threaded command execution        │
└────────────────────┬────────────────────────┘
                     │
┌────────────────────┴────────────────────────┐
│  Core Library (src/)                        │
│  engine.rs, player.rs, playlist.rs, etc.    │
│  Pure logic, no framework dependencies      │
└─────────────────────────────────────────────┘
```

### Key Design Decisions

#### 1. AppCore — Central Command Dispatcher

A new `AppCore` struct that owns all mutable state and exposes a single-threaded command API:

```rust
pub struct AppCore {
    engine: Engine,
    audio: AudioRuntime,
    logs: LogBuffer,
    event_tx: broadcast::Sender<AppEvent>,
}

impl AppCore {
    // All commands go through here — no direct Engine/Player access
    pub fn execute(&mut self, cmd: AppCommand) -> Result<AppResponse, AppError> { ... }

    // Subscribe to state changes
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> { ... }
}
```

**Why:** One `&mut self` entry point eliminates all lock ordering issues. No Mutexes needed inside AppCore — Tauri wraps it in a single `Mutex<AppCore>` (or uses a command channel).

#### 2. AppCommand Enum — Unified Command Set

```rust
pub enum AppCommand {
    // Transport
    Play { track_index: Option<usize> },
    Stop,
    Pause,
    Skip,
    Seek { position_secs: f64 },

    // Playlist
    CreatePlaylist { name: String },
    DeletePlaylist { name: String },
    RenamePlaylist { old_name: String, new_name: String },
    SetActivePlaylist { name: String },
    AddTracks { playlist: String, paths: Vec<PathBuf> },
    RemoveTracks { playlist: String, indices: Vec<usize> },
    ReorderTrack { playlist: String, from: usize, to: usize },

    // Config
    SetCrossfade { seconds: f32 },
    SetSilenceDetection { threshold: f32, duration: f32 },
    SetIntrosFolder { path: Option<String> },
    // ... etc for all config commands

    // Query
    GetStatus,
    GetTransportState,
    GetPlaylistTracks { playlist: String },
    GetConfig,
    GetAudioLevel,
    GetWaveform { path: String },
    // ... etc for all read commands
}
```

**Why:** Both CLI and Tauri serialize user intent into the same enum. Validation happens once in `execute()`.

#### 3. AudioRuntime — Background Playback Thread

Replace the blocking `play_playlist()` with a non-blocking runtime:

```rust
pub struct AudioRuntime {
    player: Option<Player>,
    state: PlaybackState,
    level_monitor: LevelMonitor,
    // Playback thread communicates via channels
    cmd_tx: Option<mpsc::Sender<AudioCmd>>,
}

enum AudioCmd {
    PlayTrack { path: PathBuf, fade_in: Option<Duration> },
    Stop,
    Pause,
    Resume,
    Seek(Duration),
    FadeOutAndPlay { next: PathBuf, fade_duration: Duration },
}
```

**Key change:** Playback runs on a dedicated thread. The main thread sends commands via channel. The audio thread sends events back (TrackFinished, SilenceDetected, LevelUpdate).

This enables:
- Auto-advance to next track (crossfade, silence skip) without polling
- GUI receives events instead of polling every 500ms
- No unsafe Send impl needed (Player stays on its own thread)

#### 4. AppEvent — Event Bus

```rust
pub enum AppEvent {
    TransportChanged(TransportState),
    TrackFinished { index: usize },
    TrackStarted { index: usize, artist: String, title: String },
    PlaylistChanged { name: String },
    ConfigChanged,
    AudioLevel(f32),
    LogEntry(LogEntry),
    Error(String),
}
```

**Tauri side:** A background task listens on the event receiver and emits Tauri events to the frontend. No more 500ms polling.

**Test side:** Tests can subscribe and assert on events directly.

#### 5. Remove CLI Binary

The standalone `src/main.rs` CLI binary is removed. All functionality lives in the Tauri app. For headless/scripted use, Tauri can be launched in headless mode or we add a simple CLI wrapper that sends commands to a running instance (future).

The `clap` dependency moves out of the core library.

### Migration Plan

#### Step 1: Create AppCore + AppCommand in core library
- New module `src/app_core.rs`
- Implement `execute()` for all commands, delegating to existing Engine/Player methods
- Write tests against AppCore directly (no Tauri, no GUI)
- Keep existing CLI and Tauri working during migration

#### Step 2: Create AudioRuntime
- New module `src/audio_runtime.rs`
- Background thread owns Player, receives AudioCmd via channel
- Sends AppEvent on track transitions, silence, level changes
- Auto-advance logic (crossfade, silence skip, auto-intro) runs on audio thread
- Test with unit tests (mock audio or short test files)

#### Step 3: Wire Tauri to AppCore
- Replace all 42 IPC handlers with thin wrappers:
  ```rust
  #[tauri::command]
  fn transport_play(state: State<Mutex<AppCore>>, track_index: Option<usize>) -> Result<...> {
      state.lock().unwrap().execute(AppCommand::Play { track_index })
  }
  ```
- Replace polling with Tauri event emission from AppEvent subscriber
- Remove Mutex<SendPlayer>, Mutex<PlaybackState>, Arc patterns
- Single `Mutex<AppCore>` for all state

#### Step 4: Create headless test harness
- `AppCore::new_test()` — creates AppCore with in-memory state (no file persistence)
- Integration tests create AppCore, execute commands, assert on responses and events
- No GUI needed for any test

#### Step 5: Remove CLI binary
- Delete `src/main.rs` (CLI)
- Remove `clap` from core library dependencies
- Update Cargo.toml to remove `[[bin]]` section
- All features verified through AppCore tests + Tauri app

### What Changes for Each Module

| Module | Change | Reason |
|--------|--------|--------|
| `engine.rs` | No change | Pure state container, AppCore wraps it |
| `player.rs` | Moves into AudioRuntime | Player lifecycle managed by audio thread |
| `playlist.rs` | No change | Pure data structure |
| `track.rs` | No change | Pure data structure |
| `scheduler.rs` | No change | Pure data model |
| `silence.rs` | Used by AudioRuntime | Silence detection on audio thread |
| `level_monitor.rs` | Used by AudioRuntime | Level monitoring on audio thread |
| `auto_intro.rs` | Used by AudioRuntime | Intro playback on audio thread |
| `waveform.rs` | Called from AppCore | CPU-bound, runs on command thread |
| `now_playing.rs` | Triggered by AppEvent | Write XML on TrackStarted event |
| `ad_scheduler.rs` | Owned by AppCore | Timer-based, checks on command dispatch |
| `rds.rs` | Owned by AppCore | Background thread, unchanged |
| `ad_logger.rs` | Called from AppCore | Logs on ad insertion events |
| `ad_report.rs` | Called from AppCore | On-demand report generation |

### Testing Strategy

1. **Unit tests** — existing module tests unchanged (engine, playlist, track, etc.)
2. **AppCore integration tests** — execute command sequences, assert responses
3. **AudioRuntime tests** — mock or short audio files, verify event sequences
4. **Event tests** — subscribe to events, assert correct emission on state changes
5. **No GUI tests needed** for core logic (Tauri IPC is thin passthrough)

### Open Questions

1. **Single Mutex vs command channel for AppCore?**
   - Mutex: simpler, synchronous responses
   - Channel: better parallelism, but adds complexity for request-response pattern
   - **Recommendation:** Start with Mutex (simpler), migrate to channel if needed

2. **How to handle long-running commands (waveform generation)?**
   - Option A: Block the Mutex (bad for GUI responsiveness)
   - Option B: Spawn to background, return immediately, emit event when done
   - **Recommendation:** Option B for waveform, keep Mutex for everything else

3. **State persistence frequency?**
   - Current: save after every mutation
   - Alternative: periodic save (every 5s) + save on shutdown
   - **Recommendation:** Keep current pattern (save after mutation) — simple, reliable

4. **GUI event delivery?**
   - Tauri events (`app.emit()`) are fire-and-forget
   - Frontend subscribes via `listen()` and updates React state
   - Replaces all polling intervals
