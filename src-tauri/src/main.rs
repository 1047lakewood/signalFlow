#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use signal_flow::auto_intro;
use signal_flow::engine::Engine;
use signal_flow::player::Player;
use signal_flow::scheduler::{ConflictPolicy, ScheduleMode, Priority, parse_time};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::State;

/// Wrapper to make Player Send+Sync for Tauri state.
/// Safety: Player is only ever accessed behind a Mutex, ensuring exclusive access.
struct SendPlayer(Option<Player>);
unsafe impl Send for SendPlayer {}
unsafe impl Sync for SendPlayer {}

struct AppState {
    engine: Mutex<Engine>,
    player: Mutex<SendPlayer>,
    playback: Arc<Mutex<PlaybackState>>,
}

/// Tracks the state of the currently playing track.
struct PlaybackState {
    is_playing: bool,
    is_paused: bool,
    track_index: Option<usize>,
    playlist_name: Option<String>,
    track_duration: Duration,
    /// When playback started (or last resumed).
    start_time: Option<Instant>,
    /// Total time spent paused so far.
    total_paused: Duration,
    /// When the current pause started (if paused).
    pause_start: Option<Instant>,
}

impl PlaybackState {
    fn new() -> Self {
        PlaybackState {
            is_playing: false,
            is_paused: false,
            track_index: None,
            playlist_name: None,
            track_duration: Duration::ZERO,
            start_time: None,
            total_paused: Duration::ZERO,
            pause_start: None,
        }
    }

    fn elapsed(&self) -> Duration {
        match self.start_time {
            Some(start) => {
                let raw = start.elapsed();
                let paused = if let Some(ps) = self.pause_start {
                    self.total_paused + ps.elapsed()
                } else {
                    self.total_paused
                };
                raw.saturating_sub(paused)
            }
            None => Duration::ZERO,
        }
    }

    fn reset(&mut self) {
        self.is_playing = false;
        self.is_paused = false;
        self.track_index = None;
        self.playlist_name = None;
        self.track_duration = Duration::ZERO;
        self.start_time = None;
        self.total_paused = Duration::ZERO;
        self.pause_start = None;
    }
}

// ── Response types ──────────────────────────────────────────────────────────

#[derive(Serialize)]
struct StatusResponse {
    playlist_count: usize,
    active_playlist: Option<String>,
    schedule_event_count: usize,
    crossfade_secs: f32,
    conflict_policy: String,
    silence_threshold: f32,
    silence_duration_secs: f32,
    intros_folder: Option<String>,
    now_playing_path: Option<String>,
}

#[derive(Serialize)]
struct PlaylistInfo {
    id: u32,
    name: String,
    track_count: usize,
    is_active: bool,
    current_index: Option<usize>,
}

#[derive(Serialize)]
struct TrackInfo {
    index: usize,
    path: String,
    title: String,
    artist: String,
    duration_secs: f64,
    duration_display: String,
    played_duration_secs: Option<f64>,
    has_intro: bool,
}

#[derive(Serialize)]
struct ScheduleEventInfo {
    id: u32,
    time: String,
    mode: String,
    file: String,
    priority: u8,
    enabled: bool,
    label: Option<String>,
    days: String,
}

#[derive(Serialize)]
struct ConfigResponse {
    crossfade_secs: f32,
    silence_threshold: f32,
    silence_duration_secs: f32,
    intros_folder: Option<String>,
    conflict_policy: String,
    now_playing_path: Option<String>,
}

#[derive(Serialize)]
struct TransportState {
    is_playing: bool,
    is_paused: bool,
    elapsed_secs: f64,
    duration_secs: f64,
    track_index: Option<usize>,
    track_artist: Option<String>,
    track_title: Option<String>,
    next_artist: Option<String>,
    next_title: Option<String>,
}

// ── Status ──────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_status(state: State<AppState>) -> StatusResponse {
    let engine = state.engine.lock().unwrap();
    StatusResponse {
        playlist_count: engine.playlists.len(),
        active_playlist: engine.active_playlist().map(|p| p.name.clone()),
        schedule_event_count: engine.schedule.events.len(),
        crossfade_secs: engine.crossfade_secs,
        conflict_policy: engine.conflict_policy.to_string(),
        silence_threshold: engine.silence_threshold,
        silence_duration_secs: engine.silence_duration_secs,
        intros_folder: engine.intros_folder.clone(),
        now_playing_path: engine.now_playing_path.clone(),
    }
}

// ── Playlist CRUD ───────────────────────────────────────────────────────────

#[tauri::command]
fn get_playlists(state: State<AppState>) -> Vec<PlaylistInfo> {
    let engine = state.engine.lock().unwrap();
    engine
        .playlists
        .iter()
        .map(|p| PlaylistInfo {
            id: p.id,
            name: p.name.clone(),
            track_count: p.track_count(),
            is_active: engine.active_playlist_id == Some(p.id),
            current_index: p.current_index,
        })
        .collect()
}

#[tauri::command]
fn create_playlist(state: State<AppState>, name: String) -> Result<u32, String> {
    let mut engine = state.engine.lock().unwrap();
    if engine.find_playlist(&name).is_some() {
        return Err(format!("Playlist '{}' already exists", name));
    }
    let id = engine.create_playlist(name);
    engine.save()?;
    Ok(id)
}

#[tauri::command]
fn delete_playlist(state: State<AppState>, name: String) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    let pos = engine
        .playlists
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case(&name))
        .ok_or_else(|| format!("Playlist '{}' not found", name))?;
    let removed_id = engine.playlists[pos].id;
    engine.playlists.remove(pos);
    if engine.active_playlist_id == Some(removed_id) {
        engine.active_playlist_id = None;
    }
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn rename_playlist(state: State<AppState>, old_name: String, new_name: String) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    if engine.find_playlist(&new_name).is_some() {
        return Err(format!("Playlist '{}' already exists", new_name));
    }
    let pl = engine
        .find_playlist_mut(&old_name)
        .ok_or_else(|| format!("Playlist '{}' not found", old_name))?;
    pl.name = new_name;
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn set_active_playlist(state: State<AppState>, name: String) -> Result<u32, String> {
    let mut engine = state.engine.lock().unwrap();
    let id = engine.set_active(&name)?;
    engine.save()?;
    Ok(id)
}

// ── Track operations ────────────────────────────────────────────────────────

#[tauri::command]
fn get_playlist_tracks(state: State<AppState>, name: String) -> Result<Vec<TrackInfo>, String> {
    let engine = state.engine.lock().unwrap();
    let intros_folder = engine.intros_folder.as_ref().map(std::path::Path::new);
    let pl = engine
        .find_playlist(&name)
        .ok_or_else(|| format!("Playlist '{}' not found", name))?;
    Ok(pl
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let has_intro = intros_folder
                .map(|folder| auto_intro::has_intro(folder, &t.artist))
                .unwrap_or(false);
            TrackInfo {
                index: i,
                path: t.path.to_string_lossy().to_string(),
                title: t.title.clone(),
                artist: t.artist.clone(),
                duration_secs: t.duration.as_secs_f64(),
                duration_display: t.duration_display(),
                played_duration_secs: t.played_duration.map(|d| d.as_secs_f64()),
                has_intro,
            }
        })
        .collect())
}

#[tauri::command]
fn add_track(state: State<AppState>, playlist: String, path: String) -> Result<usize, String> {
    let mut engine = state.engine.lock().unwrap();
    let intros_folder = engine.intros_folder.clone();
    let pl = engine
        .find_playlist_mut(&playlist)
        .ok_or_else(|| format!("Playlist '{}' not found", playlist))?;
    let idx = pl.add_track(std::path::Path::new(&path))?;
    if let Some(ref folder) = intros_folder {
        pl.tracks[idx].has_intro = auto_intro::has_intro(std::path::Path::new(folder), &pl.tracks[idx].artist);
    }
    engine.save()?;
    Ok(idx)
}

#[tauri::command]
fn add_tracks(state: State<AppState>, playlist: String, paths: Vec<String>) -> Result<usize, String> {
    let mut engine = state.engine.lock().unwrap();
    let intros_folder = engine.intros_folder.clone();
    let pl = engine
        .find_playlist_mut(&playlist)
        .ok_or_else(|| format!("Playlist '{}' not found", playlist))?;
    let mut count = 0;
    for path in &paths {
        match pl.add_track(std::path::Path::new(path)) {
            Ok(idx) => {
                if let Some(ref folder) = intros_folder {
                    pl.tracks[idx].has_intro = auto_intro::has_intro(std::path::Path::new(folder), &pl.tracks[idx].artist);
                }
                count += 1;
            }
            Err(e) => eprintln!("Failed to add '{}': {}", path, e),
        }
    }
    engine.save()?;
    Ok(count)
}

#[tauri::command]
fn remove_tracks(
    state: State<AppState>,
    playlist: String,
    indices: Vec<usize>,
) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    let pl = engine
        .find_playlist_mut(&playlist)
        .ok_or_else(|| format!("Playlist '{}' not found", playlist))?;
    // Remove in descending order to preserve indices
    let mut sorted = indices;
    sorted.sort_unstable();
    sorted.dedup();
    for &idx in sorted.iter().rev() {
        pl.remove_track(idx)?;
    }
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn reorder_track(
    state: State<AppState>,
    playlist: String,
    from: usize,
    to: usize,
) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    let pl = engine
        .find_playlist_mut(&playlist)
        .ok_or_else(|| format!("Playlist '{}' not found", playlist))?;
    pl.reorder(from, to)?;
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn edit_track_metadata(
    state: State<AppState>,
    playlist: String,
    track_index: usize,
    artist: Option<String>,
    title: Option<String>,
) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    engine.edit_track_metadata(
        &playlist,
        track_index,
        artist.as_deref(),
        title.as_deref(),
    )?;
    engine.save()?;
    Ok(())
}

// ── Transport controls ─────────────────────────────────────────────────────

/// Ensure the Player is initialized, returning a reference to it.
fn ensure_player(player_lock: &Mutex<SendPlayer>) -> Result<(), String> {
    let mut sp = player_lock.lock().unwrap();
    if sp.0.is_none() {
        sp.0 = Some(Player::new()?);
    }
    Ok(())
}

#[tauri::command]
fn transport_play(
    state: State<AppState>,
    track_index: Option<usize>,
) -> Result<(), String> {
    ensure_player(&state.player)?;

    let mut engine = state.engine.lock().unwrap();
    let pl = engine
        .active_playlist_mut()
        .ok_or_else(|| "No active playlist".to_string())?;

    let idx = track_index.unwrap_or_else(|| pl.current_index.unwrap_or(0));
    if idx >= pl.tracks.len() {
        return Err(format!(
            "Track index {} out of range ({} tracks)",
            idx,
            pl.tracks.len()
        ));
    }

    let track_path = pl.tracks[idx].path.clone();
    let track_duration = pl.tracks[idx].duration;
    let playlist_name = pl.name.clone();
    pl.current_index = Some(idx);
    engine.save().ok(); // best-effort persist

    // Play the file
    let player_guard = state.player.lock().unwrap();
    let player = player_guard.0.as_ref().unwrap();
    player.stop(); // stop any current playback
    player.play_file(&track_path)?;

    // Update playback state
    let mut pb = state.playback.lock().unwrap();
    pb.is_playing = true;
    pb.is_paused = false;
    pb.track_index = Some(idx);
    pb.playlist_name = Some(playlist_name);
    pb.track_duration = track_duration;
    pb.start_time = Some(Instant::now());
    pb.total_paused = Duration::ZERO;
    pb.pause_start = None;

    Ok(())
}

#[tauri::command]
fn transport_stop(state: State<AppState>) -> Result<(), String> {
    let player_guard = state.player.lock().unwrap();
    if let Some(player) = player_guard.0.as_ref() {
        player.stop();
    }

    let mut pb = state.playback.lock().unwrap();
    pb.reset();

    Ok(())
}

#[tauri::command]
fn transport_pause(state: State<AppState>) -> Result<(), String> {
    let player_guard = state.player.lock().unwrap();
    let player = player_guard.0.as_ref().ok_or("No audio player")?;

    let mut pb = state.playback.lock().unwrap();
    if !pb.is_playing {
        return Err("Nothing is playing".to_string());
    }

    if pb.is_paused {
        // Resume
        player.resume();
        pb.is_paused = false;
        if let Some(ps) = pb.pause_start.take() {
            pb.total_paused += ps.elapsed();
        }
    } else {
        // Pause
        player.pause();
        pb.is_paused = true;
        pb.pause_start = Some(Instant::now());
    }

    Ok(())
}

#[tauri::command]
fn transport_skip(state: State<AppState>) -> Result<(), String> {
    // Stop current playback
    {
        let player_guard = state.player.lock().unwrap();
        if let Some(player) = player_guard.0.as_ref() {
            player.stop();
        }
    }

    // Advance index
    let next_idx;
    let track_path;
    let track_duration;
    let playlist_name;
    {
        let mut engine = state.engine.lock().unwrap();
        let pl = engine
            .active_playlist_mut()
            .ok_or_else(|| "No active playlist".to_string())?;

        let current = pl.current_index.unwrap_or(0);
        next_idx = current + 1;
        if next_idx >= pl.tracks.len() {
            // End of playlist
            pl.current_index = None;
            engine.save().ok();
            let mut pb = state.playback.lock().unwrap();
            pb.reset();
            return Ok(());
        }

        track_path = pl.tracks[next_idx].path.clone();
        track_duration = pl.tracks[next_idx].duration;
        playlist_name = pl.name.clone();
        pl.current_index = Some(next_idx);
        engine.save().ok();
    }

    // Play next track
    ensure_player(&state.player)?;
    let player_guard = state.player.lock().unwrap();
    let player = player_guard.0.as_ref().unwrap();
    player.play_file(&track_path)?;

    let mut pb = state.playback.lock().unwrap();
    pb.is_playing = true;
    pb.is_paused = false;
    pb.track_index = Some(next_idx);
    pb.playlist_name = Some(playlist_name);
    pb.track_duration = track_duration;
    pb.start_time = Some(Instant::now());
    pb.total_paused = Duration::ZERO;
    pb.pause_start = None;

    Ok(())
}

#[tauri::command]
fn transport_seek(state: State<AppState>, position_secs: f64) -> Result<(), String> {
    let player_guard = state.player.lock().unwrap();
    let player = player_guard.0.as_ref().ok_or("No audio player")?;

    let pb = state.playback.lock().unwrap();
    if !pb.is_playing {
        return Err("Nothing is playing".to_string());
    }
    drop(pb);

    let seek_pos = Duration::from_secs_f64(position_secs.max(0.0));
    player.try_seek(seek_pos)?;

    // Reset timing to reflect the seek position
    let mut pb = state.playback.lock().unwrap();
    pb.start_time = Some(Instant::now() - seek_pos);
    pb.total_paused = Duration::ZERO;
    pb.pause_start = if pb.is_paused { Some(Instant::now()) } else { None };

    Ok(())
}

#[tauri::command]
fn transport_status(state: State<AppState>) -> TransportState {
    let pb = state.playback.lock().unwrap();

    // Check if the sink has finished playing (track ended naturally)
    let actually_playing = if pb.is_playing && !pb.is_paused {
        let player_guard = state.player.lock().unwrap();
        if let Some(player) = player_guard.0.as_ref() {
            !player.is_empty()
        } else {
            false
        }
    } else {
        pb.is_playing
    };

    let elapsed = pb.elapsed();
    let (artist, title, next_artist, next_title) = if let (Some(idx), Some(pl_name)) = (pb.track_index, &pb.playlist_name) {
        let engine = state.engine.lock().unwrap();
        if let Some(pl) = engine.find_playlist(pl_name) {
            let current = if let Some(track) = pl.tracks.get(idx) {
                (Some(track.artist.clone()), Some(track.title.clone()))
            } else {
                (None, None)
            };
            let next = if let Some(next_track) = pl.tracks.get(idx + 1) {
                (Some(next_track.artist.clone()), Some(next_track.title.clone()))
            } else {
                (None, None)
            };
            (current.0, current.1, next.0, next.1)
        } else {
            (None, None, None, None)
        }
    } else {
        (None, None, None, None)
    };

    TransportState {
        is_playing: actually_playing,
        is_paused: pb.is_paused,
        elapsed_secs: elapsed.as_secs_f64(),
        duration_secs: pb.track_duration.as_secs_f64(),
        track_index: pb.track_index,
        track_artist: artist,
        track_title: title,
        next_artist,
        next_title,
    }
}

// ── Schedule ────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_schedule(state: State<AppState>) -> Vec<ScheduleEventInfo> {
    let engine = state.engine.lock().unwrap();
    engine
        .schedule
        .events_by_time()
        .into_iter()
        .map(|e| ScheduleEventInfo {
            id: e.id,
            time: e.time_display(),
            mode: e.mode.to_string(),
            file: e.file.to_string_lossy().to_string(),
            priority: e.priority.0,
            enabled: e.enabled,
            label: e.label.clone(),
            days: e.days_display(),
        })
        .collect()
}

#[tauri::command]
fn add_schedule_event(
    state: State<AppState>,
    time: String,
    mode: String,
    file: String,
    priority: Option<u8>,
    label: Option<String>,
    days: Option<Vec<u8>>,
) -> Result<u32, String> {
    let parsed_time = parse_time(&time)?;
    let parsed_mode = ScheduleMode::from_str_loose(&mode)?;
    let pri = Priority(priority.unwrap_or(5));
    let days_vec = days.unwrap_or_default();

    let mut engine = state.engine.lock().unwrap();
    let id = engine.schedule.add_event(
        parsed_time,
        parsed_mode,
        PathBuf::from(&file),
        pri,
        label,
        days_vec,
    );
    engine.save()?;
    Ok(id)
}

#[tauri::command]
fn remove_schedule_event(state: State<AppState>, id: u32) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    engine.schedule.remove_event(id)?;
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn toggle_schedule_event(state: State<AppState>, id: u32) -> Result<bool, String> {
    let mut engine = state.engine.lock().unwrap();
    let new_state = engine.schedule.toggle_event(id)?;
    engine.save()?;
    Ok(new_state)
}

// ── Config ──────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_config(state: State<AppState>) -> ConfigResponse {
    let engine = state.engine.lock().unwrap();
    ConfigResponse {
        crossfade_secs: engine.crossfade_secs,
        silence_threshold: engine.silence_threshold,
        silence_duration_secs: engine.silence_duration_secs,
        intros_folder: engine.intros_folder.clone(),
        conflict_policy: engine.conflict_policy.to_string(),
        now_playing_path: engine.now_playing_path.clone(),
    }
}

#[tauri::command]
fn set_crossfade(state: State<AppState>, secs: f32) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    engine.crossfade_secs = secs;
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn set_silence_detection(
    state: State<AppState>,
    threshold: f32,
    duration_secs: f32,
) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    engine.silence_threshold = threshold;
    engine.silence_duration_secs = duration_secs;
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn set_intros_folder(state: State<AppState>, path: Option<String>) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    if let Some(ref p) = path {
        if !std::path::Path::new(p).is_dir() {
            return Err(format!("'{}' is not a valid directory", p));
        }
    }
    engine.intros_folder = path.clone();
    // Refresh has_intro flags on all tracks
    for pl in &mut engine.playlists {
        for track in &mut pl.tracks {
            track.has_intro = match &path {
                Some(folder) => auto_intro::has_intro(std::path::Path::new(folder), &track.artist),
                None => false,
            };
        }
    }
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn set_conflict_policy(state: State<AppState>, policy: String) -> Result<(), String> {
    let parsed = ConflictPolicy::from_str_loose(&policy)?;
    let mut engine = state.engine.lock().unwrap();
    engine.conflict_policy = parsed;
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn set_nowplaying_path(state: State<AppState>, path: Option<String>) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    engine.now_playing_path = path;
    engine.save()?;
    Ok(())
}

// ── App entry ───────────────────────────────────────────────────────────────

fn main() {
    let engine = Engine::load();
    let playback = Arc::new(Mutex::new(PlaybackState::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            engine: Mutex::new(engine),
            player: Mutex::new(SendPlayer(None)),
            playback,
        })
        .invoke_handler(tauri::generate_handler![
            // Status
            get_status,
            // Playlist CRUD
            get_playlists,
            create_playlist,
            delete_playlist,
            rename_playlist,
            set_active_playlist,
            // Track operations
            get_playlist_tracks,
            add_track,
            add_tracks,
            remove_tracks,
            reorder_track,
            edit_track_metadata,
            // Transport
            transport_play,
            transport_stop,
            transport_pause,
            transport_skip,
            transport_seek,
            transport_status,
            // Schedule
            get_schedule,
            add_schedule_event,
            remove_schedule_event,
            toggle_schedule_event,
            // Config
            get_config,
            set_crossfade,
            set_silence_detection,
            set_intros_folder,
            set_conflict_policy,
            set_nowplaying_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
