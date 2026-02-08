#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use signal_flow::engine::Engine;
use signal_flow::scheduler::{ConflictPolicy, ScheduleMode, Priority, parse_time};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

struct AppState {
    engine: Mutex<Engine>,
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
    let pl = engine
        .find_playlist(&name)
        .ok_or_else(|| format!("Playlist '{}' not found", name))?;
    Ok(pl
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| TrackInfo {
            index: i,
            path: t.path.to_string_lossy().to_string(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            duration_secs: t.duration.as_secs_f64(),
            duration_display: t.duration_display(),
            played_duration_secs: t.played_duration.map(|d| d.as_secs_f64()),
            has_intro: t.has_intro,
        })
        .collect())
}

#[tauri::command]
fn add_track(state: State<AppState>, playlist: String, path: String) -> Result<usize, String> {
    let mut engine = state.engine.lock().unwrap();
    let pl = engine
        .find_playlist_mut(&playlist)
        .ok_or_else(|| format!("Playlist '{}' not found", playlist))?;
    let idx = pl.add_track(std::path::Path::new(&path))?;
    engine.save()?;
    Ok(idx)
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
    engine.intros_folder = path;
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

    tauri::Builder::default()
        .manage(AppState {
            engine: Mutex::new(engine),
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
            remove_tracks,
            reorder_track,
            edit_track_metadata,
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
