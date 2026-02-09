#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use signal_flow::auto_intro;
use signal_flow::engine::Engine;
use signal_flow::level_monitor::LevelMonitor;
use signal_flow::player::Player;
use signal_flow::rds::{RdsMessage, RdsSchedule};
use signal_flow::scheduler::{ConflictPolicy, ScheduleMode, Priority, parse_time};
use chrono::Local;
use serde::Serialize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::State;

/// Wrapper to make Player Send+Sync for Tauri state.
/// Safety: Player is only ever accessed behind a Mutex, ensuring exclusive access.
struct SendPlayer(Option<Player>);
unsafe impl Send for SendPlayer {}
unsafe impl Sync for SendPlayer {}

const LOG_BUFFER_MAX: usize = 500;

#[derive(Clone, Serialize)]
struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
}

struct LogBuffer {
    entries: VecDeque<LogEntry>,
}

impl LogBuffer {
    fn new() -> Self {
        LogBuffer {
            entries: VecDeque::new(),
        }
    }

    fn push(&mut self, level: &str, message: String) {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        self.entries.push_back(LogEntry {
            timestamp,
            level: level.to_string(),
            message,
        });
        while self.entries.len() > LOG_BUFFER_MAX {
            self.entries.pop_front();
        }
    }
}

struct AppState {
    engine: Mutex<Engine>,
    player: Mutex<SendPlayer>,
    playback: Arc<Mutex<PlaybackState>>,
    logs: Mutex<LogBuffer>,
    level_monitor: LevelMonitor,
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
    recurring_intro_interval_secs: f32,
    recurring_intro_duck_volume: f32,
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
    recurring_intro_interval_secs: f32,
    recurring_intro_duck_volume: f32,
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
    track_path: Option<String>,
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
        recurring_intro_interval_secs: engine.recurring_intro_interval_secs,
        recurring_intro_duck_volume: engine.recurring_intro_duck_volume,
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
    let track_artist = pl.tracks[idx].artist.clone();
    let track_title = pl.tracks[idx].title.clone();
    let playlist_name = pl.name.clone();
    pl.current_index = Some(idx);
    engine.save().ok(); // best-effort persist

    // Play the file with level monitoring
    state.level_monitor.reset();
    let player_guard = state.player.lock().unwrap();
    let player = player_guard.0.as_ref().unwrap();
    player.stop(); // stop any current playback
    player.play_file_with_level(&track_path, state.level_monitor.clone())?;

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

    // Log
    state.logs.lock().unwrap().push("info", format!("Playing: {} — {}", track_artist, track_title));

    Ok(())
}

#[tauri::command]
fn transport_stop(state: State<AppState>) -> Result<(), String> {
    let player_guard = state.player.lock().unwrap();
    if let Some(player) = player_guard.0.as_ref() {
        player.stop();
    }
    state.level_monitor.reset();

    let mut pb = state.playback.lock().unwrap();
    pb.reset();

    state.logs.lock().unwrap().push("info", "Playback stopped".to_string());

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
        state.logs.lock().unwrap().push("info", "Playback resumed".to_string());
    } else {
        // Pause
        player.pause();
        pb.is_paused = true;
        pb.pause_start = Some(Instant::now());
        state.logs.lock().unwrap().push("info", "Playback paused".to_string());
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
            state.logs.lock().unwrap().push("info", "Reached end of playlist".to_string());
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

    // Log skip
    {
        let engine = state.engine.lock().unwrap();
        if let Some(pl) = engine.active_playlist() {
            if let Some(track) = pl.tracks.get(next_idx) {
                state.logs.lock().unwrap().push("info", format!("Skipped to: {} — {}", track.artist, track.title));
            }
        }
    }

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
    let (artist, title, next_artist, next_title, track_path) = if let (Some(idx), Some(pl_name)) = (pb.track_index, &pb.playlist_name) {
        let engine = state.engine.lock().unwrap();
        if let Some(pl) = engine.find_playlist(pl_name) {
            let current = if let Some(track) = pl.tracks.get(idx) {
                (Some(track.artist.clone()), Some(track.title.clone()), Some(track.path.to_string_lossy().to_string()))
            } else {
                (None, None, None)
            };
            let next = if let Some(next_track) = pl.tracks.get(idx + 1) {
                (Some(next_track.artist.clone()), Some(next_track.title.clone()))
            } else {
                (None, None)
            };
            (current.0, current.1, next.0, next.1, current.2)
        } else {
            (None, None, None, None, None)
        }
    } else {
        (None, None, None, None, None)
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
        track_path,
    }
}

// ── Audio Level ─────────────────────────────────────────────────────────────

#[tauri::command]
fn get_audio_level(state: State<AppState>) -> f32 {
    state.level_monitor.level()
}

// ── Waveform ────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_waveform(path: String) -> Result<Vec<f32>, String> {
    signal_flow::waveform::generate_peaks_default(std::path::Path::new(&path))
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
        label.clone(),
        days_vec,
    );
    engine.save()?;
    let display = label.unwrap_or_else(|| file.clone());
    state.logs.lock().unwrap().push("info", format!("Schedule event added: {} at {}", display, time));
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
        recurring_intro_interval_secs: engine.recurring_intro_interval_secs,
        recurring_intro_duck_volume: engine.recurring_intro_duck_volume,
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
fn set_recurring_intro(
    state: State<AppState>,
    interval_secs: f32,
    duck_volume: f32,
) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    engine.recurring_intro_interval_secs = interval_secs;
    engine.recurring_intro_duck_volume = duck_volume;
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

// ── Ads ─────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct AdInfo {
    index: usize,
    name: String,
    enabled: bool,
    mp3_file: String,
    scheduled: bool,
    days: Vec<String>,
    hours: Vec<u8>,
}

#[tauri::command]
fn get_ads(state: State<AppState>) -> Vec<AdInfo> {
    let engine = state.engine.lock().unwrap();
    engine
        .ads
        .iter()
        .enumerate()
        .map(|(i, ad)| AdInfo {
            index: i,
            name: ad.name.clone(),
            enabled: ad.enabled,
            mp3_file: ad.mp3_file.to_string_lossy().to_string(),
            scheduled: ad.scheduled,
            days: ad.days.clone(),
            hours: ad.hours.clone(),
        })
        .collect()
}

#[tauri::command]
fn add_ad(state: State<AppState>, name: String, mp3_file: String) -> Result<usize, String> {
    let mut engine = state.engine.lock().unwrap();
    let ad = signal_flow::ad_scheduler::AdConfig::new(name, PathBuf::from(mp3_file));
    let idx = engine.add_ad(ad);
    engine.save()?;
    Ok(idx)
}

#[tauri::command]
fn remove_ad(state: State<AppState>, index: usize) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    engine.remove_ad(index)?;
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn toggle_ad(state: State<AppState>, index: usize) -> Result<bool, String> {
    let mut engine = state.engine.lock().unwrap();
    let new_state = engine.toggle_ad(index)?;
    engine.save()?;
    Ok(new_state)
}

#[tauri::command]
fn update_ad(
    state: State<AppState>,
    index: usize,
    name: String,
    enabled: bool,
    mp3_file: String,
    scheduled: bool,
    days: Vec<String>,
    hours: Vec<u8>,
) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    let len = engine.ads.len();
    let ad = engine.ads.get_mut(index).ok_or_else(|| {
        format!("Ad index {} out of range ({} ads)", index, len)
    })?;
    ad.name = name;
    ad.enabled = enabled;
    ad.mp3_file = PathBuf::from(mp3_file);
    ad.scheduled = scheduled;
    ad.days = days;
    ad.hours = hours;
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn reorder_ad(state: State<AppState>, from: usize, to: usize) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    let len = engine.ads.len();
    if from >= len || to >= len {
        return Err(format!("Ad index out of range ({} ads)", len));
    }
    let ad = engine.ads.remove(from);
    engine.ads.insert(to, ad);
    engine.save()?;
    Ok(())
}

// ── Ad Statistics & Reports ──────────────────────────────────────────────────

#[derive(Serialize)]
struct AdStatsResponse {
    total_plays: usize,
    per_ad: Vec<AdStatEntryResponse>,
}

#[derive(Serialize)]
struct AdStatEntryResponse {
    name: String,
    play_count: usize,
}

#[derive(Serialize)]
struct AdDailyCountResponse {
    date: String,
    count: usize,
}

#[derive(Serialize)]
struct AdFailureResponse {
    timestamp: String,
    ads: Vec<String>,
    error: String,
}

#[tauri::command]
fn get_ad_stats(start: Option<String>, end: Option<String>) -> AdStatsResponse {
    let logger = signal_flow::ad_logger::AdPlayLogger::new(std::path::Path::new("."));
    let stats = match (start, end) {
        (Some(s), Some(e)) => logger.get_ad_statistics_filtered(&s, &e),
        _ => logger.get_ad_statistics(),
    };
    AdStatsResponse {
        total_plays: stats.total_plays,
        per_ad: stats.per_ad.into_iter().map(|e| AdStatEntryResponse {
            name: e.name,
            play_count: e.play_count,
        }).collect(),
    }
}

#[tauri::command]
fn get_ad_daily_counts(ad_name: String) -> Vec<AdDailyCountResponse> {
    let logger = signal_flow::ad_logger::AdPlayLogger::new(std::path::Path::new("."));
    let counts = logger.get_daily_play_counts(&ad_name);
    let mut entries: Vec<AdDailyCountResponse> = counts.into_iter()
        .map(|(date, count)| AdDailyCountResponse { date, count })
        .collect();
    entries.sort_by(|a, b| a.date.cmp(&b.date));
    entries
}

#[tauri::command]
fn get_ad_failures() -> Vec<AdFailureResponse> {
    let logger = signal_flow::ad_logger::AdPlayLogger::new(std::path::Path::new("."));
    logger.get_failures().into_iter().map(|f| AdFailureResponse {
        timestamp: f.t,
        ads: f.ads,
        error: f.err,
    }).collect()
}

#[tauri::command]
fn generate_ad_report(
    start: String,
    end: String,
    output_dir: String,
    ad_name: Option<String>,
    company_name: Option<String>,
) -> Result<Vec<String>, String> {
    let logger = signal_flow::ad_logger::AdPlayLogger::new(std::path::Path::new("."));
    let reporter = signal_flow::ad_report::AdReportGenerator::new(&logger);
    let out_path = std::path::Path::new(&output_dir);

    if !out_path.is_dir() {
        return Err(format!("'{}' is not a valid directory", output_dir));
    }

    match ad_name {
        Some(name) => {
            match reporter.generate_single_report(&name, &start, &end, company_name.as_deref(), out_path) {
                Some(r) => Ok(vec![
                    r.csv_path.to_string_lossy().to_string(),
                    r.pdf_path.to_string_lossy().to_string(),
                ]),
                None => Ok(vec![]),
            }
        }
        None => {
            let results = reporter.generate_report(&start, &end, company_name.as_deref(), out_path);
            Ok(results.into_iter().flat_map(|r| vec![
                r.csv_path.to_string_lossy().to_string(),
                r.pdf_path.to_string_lossy().to_string(),
            ]).collect())
        }
    }
}

// ── RDS ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct RdsMessageInfo {
    index: usize,
    text: String,
    enabled: bool,
    duration: u32,
    scheduled: bool,
    days: Vec<String>,
    hours: Vec<u8>,
}

#[derive(Serialize)]
struct RdsConfigResponse {
    ip: String,
    port: u16,
    default_message: String,
    messages: Vec<RdsMessageInfo>,
}

#[tauri::command]
fn get_rds_config(state: State<AppState>) -> RdsConfigResponse {
    let engine = state.engine.lock().unwrap();
    RdsConfigResponse {
        ip: engine.rds.ip.clone(),
        port: engine.rds.port,
        default_message: engine.rds.default_message.clone(),
        messages: engine.rds.messages.iter().enumerate().map(|(i, m)| RdsMessageInfo {
            index: i,
            text: m.text.clone(),
            enabled: m.enabled,
            duration: m.duration,
            scheduled: m.scheduled.enabled,
            days: m.scheduled.days.clone(),
            hours: m.scheduled.hours.clone(),
        }).collect(),
    }
}

#[tauri::command]
fn add_rds_message(state: State<AppState>, text: String) -> Result<usize, String> {
    let mut engine = state.engine.lock().unwrap();
    let msg = RdsMessage::new(&text);
    engine.rds.messages.push(msg);
    let idx = engine.rds.messages.len() - 1;
    engine.save()?;
    Ok(idx)
}

#[tauri::command]
fn remove_rds_message(state: State<AppState>, index: usize) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    let len = engine.rds.messages.len();
    if index >= len {
        return Err(format!("RDS message index {} out of range ({} messages)", index, len));
    }
    engine.rds.messages.remove(index);
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn toggle_rds_message(state: State<AppState>, index: usize) -> Result<bool, String> {
    let mut engine = state.engine.lock().unwrap();
    let len = engine.rds.messages.len();
    let msg = engine.rds.messages.get_mut(index).ok_or_else(|| {
        format!("RDS message index {} out of range ({} messages)", index, len)
    })?;
    msg.enabled = !msg.enabled;
    let new_state = msg.enabled;
    engine.save()?;
    Ok(new_state)
}

#[tauri::command]
fn update_rds_message(
    state: State<AppState>,
    index: usize,
    text: String,
    enabled: bool,
    duration: u32,
    scheduled: bool,
    days: Vec<String>,
    hours: Vec<u8>,
) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    let len = engine.rds.messages.len();
    let msg = engine.rds.messages.get_mut(index).ok_or_else(|| {
        format!("RDS message index {} out of range ({} messages)", index, len)
    })?;
    msg.text = text;
    msg.enabled = enabled;
    msg.duration = duration.clamp(1, 60);
    msg.scheduled = RdsSchedule {
        enabled: scheduled,
        days,
        hours,
    };
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn reorder_rds_message(state: State<AppState>, from: usize, to: usize) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    let len = engine.rds.messages.len();
    if from >= len || to >= len {
        return Err(format!("RDS message index out of range ({} messages)", len));
    }
    let msg = engine.rds.messages.remove(from);
    engine.rds.messages.insert(to, msg);
    engine.save()?;
    Ok(())
}

#[tauri::command]
fn update_rds_settings(
    state: State<AppState>,
    ip: String,
    port: u16,
    default_message: String,
) -> Result<(), String> {
    let mut engine = state.engine.lock().unwrap();
    engine.rds.ip = ip;
    engine.rds.port = port;
    engine.rds.default_message = default_message;
    engine.save()?;
    Ok(())
}

// ── Logs ────────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_logs(state: State<AppState>, since_index: Option<usize>) -> Vec<LogEntry> {
    let logs = state.logs.lock().unwrap();
    let start = since_index.unwrap_or(0);
    logs.entries.iter().skip(start).cloned().collect()
}

#[tauri::command]
fn clear_logs(state: State<AppState>) {
    state.logs.lock().unwrap().entries.clear();
}

// ── App entry ───────────────────────────────────────────────────────────────

fn main() {
    let engine = Engine::load();
    let playback = Arc::new(Mutex::new(PlaybackState::new()));
    let level_monitor = LevelMonitor::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            engine: Mutex::new(engine),
            player: Mutex::new(SendPlayer(None)),
            playback,
            logs: Mutex::new(LogBuffer::new()),
            level_monitor,
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
            get_audio_level,
            get_waveform,
            // Schedule
            get_schedule,
            add_schedule_event,
            remove_schedule_event,
            toggle_schedule_event,
            // Ads
            get_ads,
            add_ad,
            remove_ad,
            toggle_ad,
            update_ad,
            reorder_ad,
            // RDS
            get_rds_config,
            add_rds_message,
            remove_rds_message,
            toggle_rds_message,
            update_rds_message,
            reorder_rds_message,
            update_rds_settings,
            // Ad Statistics & Reports
            get_ad_stats,
            get_ad_daily_counts,
            get_ad_failures,
            generate_ad_report,
            // Logs
            get_logs,
            clear_logs,
            // Config
            get_config,
            set_crossfade,
            set_silence_detection,
            set_intros_folder,
            set_recurring_intro,
            set_conflict_policy,
            set_nowplaying_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
