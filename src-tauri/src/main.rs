#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use signal_flow::app_core::{
    list_directory_at, search_files_in_locations, AdData, AppCore, ConfigData, FileBrowserEntry,
    FileSearchResult, LogEntry, PlaylistData, PlaylistProfileData, RdsConfigData,
    ScheduleEventData, StatusData, TrackData, TransportData,
};
use signal_flow::audio_runtime::{spawn_audio_runtime, AudioEvent, AudioHandle};
use signal_flow::level_monitor::LevelMonitor;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState {
    core: Arc<Mutex<AppCore>>,
    audio: AudioHandle,
    level_monitor: LevelMonitor,
}

// ── Ad stats response types (thin wrappers for field name mapping) ──────────

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

// ── Status ──────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_status(state: State<AppState>) -> StatusData {
    state.core.lock().unwrap().get_status()
}

// ── Playlist CRUD ───────────────────────────────────────────────────────────

#[tauri::command]
fn get_playlists(state: State<AppState>) -> Vec<PlaylistData> {
    state.core.lock().unwrap().get_playlists()
}

#[tauri::command]
fn create_playlist(state: State<AppState>, name: String) -> Result<u32, String> {
    state.core.lock().unwrap().create_playlist(name)
}

#[tauri::command]
fn delete_playlist(state: State<AppState>, name: String) -> Result<(), String> {
    state.core.lock().unwrap().delete_playlist(&name)
}

#[tauri::command]
fn rename_playlist(
    state: State<AppState>,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    state
        .core
        .lock()
        .unwrap()
        .rename_playlist(&old_name, new_name)
}

#[tauri::command]
fn set_active_playlist(state: State<AppState>, name: String) -> Result<u32, String> {
    state.core.lock().unwrap().set_active_playlist(&name)
}

#[tauri::command]
fn get_playlist_profiles(state: State<AppState>) -> Vec<PlaylistProfileData> {
    state.core.lock().unwrap().get_playlist_profiles()
}

#[tauri::command]
fn save_playlist_profile(state: State<AppState>, name: String) -> Result<(), String> {
    state.core.lock().unwrap().save_playlist_profile(&name)
}

#[tauri::command]
fn load_playlist_profile(state: State<AppState>, name: String) -> Result<(), String> {
    state.core.lock().unwrap().load_playlist_profile(&name)
}

#[tauri::command]
fn delete_playlist_profile(state: State<AppState>, name: String) -> Result<(), String> {
    state.core.lock().unwrap().delete_playlist_profile(&name)
}

#[tauri::command]
async fn import_m3u_playlist(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<String, String> {
    use signal_flow::auto_intro;
    use signal_flow::track::Track;

    // Phase 1: parse M3U + read all track metadata off the lock.
    let intros_folder = state.core.lock().unwrap().intros_folder();
    let (name_stem, source_path, track_paths, loaded) =
        tokio::task::spawn_blocking(move || {
            let (stem, src, paths) = signal_flow::app_core::AppCore::parse_m3u_file(&file_path)?;
            let mut tracks = Vec::new();
            for p in &paths {
                if let Ok(mut t) = Track::from_path(p) {
                    if let Some(ref folder) = intros_folder {
                        t.has_intro = auto_intro::has_intro(std::path::Path::new(folder), &t.artist);
                    }
                    tracks.push(t);
                }
            }
            Ok::<_, String>((stem, src, paths, tracks))
        })
        .await
        .map_err(|e| format!("Import task panicked: {e}"))??;

    let _ = track_paths; // paths used only for loading above
    // Phase 2: lock briefly to create playlist + save.
    state
        .core
        .lock()
        .unwrap()
        .import_preloaded_m3u(&name_stem, &source_path, loaded)
}

#[tauri::command]
async fn export_playlist_to_m3u(
    state: State<'_, AppState>,
    playlist_name: String,
    file_path: Option<String>,
) -> Result<String, String> {
    // Phase 1: extract track paths while briefly holding the lock.
    let (track_paths, existing_source) = {
        let core = state.core.lock().unwrap();
        core.get_m3u_export_data(&playlist_name)?
    };

    let target_path = if let Some(path) = file_path {
        std::path::PathBuf::from(path)
    } else {
        std::path::PathBuf::from(
            existing_source.ok_or_else(|| "Playlist has no source file; choose Save As".to_string())?,
        )
    };
    let target_str = target_path.to_string_lossy().to_string();

    // Phase 2: write the M3U file on a blocking thread (no lock held).
    let target_path_clone = target_path.clone();
    tokio::task::spawn_blocking(move || {
        let mut lines = vec!["#EXTM3U".to_string()];
        lines.extend(track_paths);
        std::fs::write(&target_path_clone, lines.join("\n") + "\n").map_err(|e| {
            format!("Failed to write playlist '{}': {}", target_path_clone.display(), e)
        })
    })
    .await
    .map_err(|e| format!("Export task panicked: {e}"))??;

    // Phase 3: update source_path in core and save.
    state
        .core
        .lock()
        .unwrap()
        .set_playlist_source_path(&playlist_name, &target_str)?;

    Ok(target_str)
}

// ── Track operations ────────────────────────────────────────────────────────

#[tauri::command]
fn get_playlist_tracks(state: State<AppState>, name: String) -> Result<Vec<TrackData>, String> {
    state.core.lock().unwrap().get_playlist_tracks(&name)
}

#[tauri::command]
fn add_track(state: State<AppState>, playlist: String, path: String) -> Result<usize, String> {
    state.core.lock().unwrap().add_track(&playlist, &path)
}

#[tauri::command]
async fn add_tracks(
    state: State<'_, AppState>,
    playlist: String,
    paths: Vec<String>,
) -> Result<usize, String> {
    use signal_flow::auto_intro;
    use signal_flow::track::Track;

    // Phase 1: extract intros_folder while briefly holding the lock, then drop it.
    let intros_folder = {
        let core = state.core.lock().unwrap();
        if core.engine.find_playlist(&playlist).is_none() {
            return Err(format!("Playlist '{}' not found", playlist));
        }
        core.intros_folder()
    };

    // Phase 2: read all file metadata on a blocking thread (no lock held).
    let loaded = tokio::task::spawn_blocking(move || {
        let mut tracks = Vec::new();
        for path_str in &paths {
            match Track::from_path(std::path::Path::new(path_str)) {
                Ok(mut t) => {
                    if let Some(ref folder) = intros_folder {
                        t.has_intro = auto_intro::has_intro(std::path::Path::new(folder), &t.artist);
                    }
                    tracks.push(t);
                }
                Err(e) => eprintln!("Failed to load '{}': {}", path_str, e),
            }
        }
        tracks
    })
    .await
    .map_err(|e| format!("Track loading panicked: {e}"))?;

    // Phase 3: lock briefly to push pre-built tracks + save.
    state.core.lock().unwrap().push_preloaded_tracks(&playlist, loaded)
}

#[tauri::command]
fn remove_tracks(
    state: State<AppState>,
    playlist: String,
    indices: Vec<usize>,
) -> Result<(), String> {
    state
        .core
        .lock()
        .unwrap()
        .remove_tracks(&playlist, &indices)
}

#[tauri::command]
fn reorder_track(
    state: State<AppState>,
    playlist: String,
    from: usize,
    to: usize,
) -> Result<(), String> {
    state
        .core
        .lock()
        .unwrap()
        .reorder_track(&playlist, from, to)
}

#[tauri::command]
fn copy_paste_tracks(
    state: State<AppState>,
    from_playlist: String,
    indices: Vec<usize>,
    to_playlist: String,
    at: Option<usize>,
) -> Result<(), String> {
    let mut core = state.core.lock().unwrap();
    let tracks = core.copy_tracks(&from_playlist, &indices)?;
    core.paste_tracks(&to_playlist, tracks, at)
}

#[tauri::command]
fn edit_track_metadata(
    state: State<AppState>,
    playlist: String,
    track_index: usize,
    artist: Option<String>,
    title: Option<String>,
) -> Result<(), String> {
    state.core.lock().unwrap().edit_track_metadata(
        &playlist,
        track_index,
        artist.as_deref(),
        title.as_deref(),
    )
}

// ── File browser / search ───────────────────────────────────────────────

#[tauri::command]
async fn list_directory(
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<Vec<FileBrowserEntry>, String> {
    let target = {
        let core = state.core.lock().unwrap();
        core.resolve_directory_path(path)
    };
    tokio::task::spawn_blocking(move || list_directory_at(target))
        .await
        .map_err(|e| format!("Directory read task panicked: {e}"))?
}

#[tauri::command]
async fn search_indexed_files(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<FileSearchResult>, String> {
    // Extract indexed_locations while holding the lock briefly, then drop it
    // before doing the blocking filesystem walk so we don't freeze all IPC.
    let locations = {
        let core = state.core.lock().unwrap();
        core.engine.indexed_locations.clone()
    };
    tokio::task::spawn_blocking(move || Ok(search_files_in_locations(&locations, &query)))
        .await
        .map_err(|e| format!("Search task panicked: {e}"))?
}

// ── Transport controls ─────────────────────────────────────────────────────

#[tauri::command]
fn transport_play(
    state: State<AppState>,
    app: AppHandle,
    track_index: Option<usize>,
) -> Result<(), String> {
    // Lock core: prepare play state (updates engine, playback, logs)
    let (track_path, ..) = {
        let mut core = state.core.lock().unwrap();
        core.prepare_play(track_index)?
    }; // core lock dropped

    // Send play command to audio thread (file decode happens there)
    state.audio.play(track_path, state.level_monitor.clone());

    // Emit events so frontend updates immediately
    let _ = app.emit("transport-changed", ());
    let _ = app.emit("logs-changed", ());

    Ok(())
}

#[tauri::command]
fn transport_stop(state: State<AppState>, app: AppHandle) -> Result<(), String> {
    // Stop audio
    state.audio.stop();
    state.level_monitor.reset();

    // Update core state
    {
        let mut core = state.core.lock().unwrap();
        core.on_stop();
    }

    let _ = app.emit("transport-changed", ());
    let _ = app.emit("logs-changed", ());

    Ok(())
}

#[tauri::command]
fn transport_pause(state: State<AppState>, app: AppHandle) -> Result<(), String> {
    // Toggle pause state in core
    let now_paused = {
        let mut core = state.core.lock().unwrap();
        core.on_pause_toggle()?
    };

    // Send to audio thread
    if now_paused {
        state.audio.pause();
    } else {
        state.audio.resume();
    }

    let _ = app.emit("transport-changed", ());
    let _ = app.emit("logs-changed", ());

    Ok(())
}

#[tauri::command]
fn transport_skip(state: State<AppState>, app: AppHandle) -> Result<(), String> {
    // Stop current playback
    state.audio.stop();

    // Advance to next track in core
    let skip_result = {
        let mut core = state.core.lock().unwrap();
        core.prepare_skip()
    };

    let (track_path, ..) = match skip_result {
        Ok(data) => data,
        Err(ref e) if e == "__end_of_playlist__" => {
            let _ = app.emit("transport-changed", ());
            let _ = app.emit("logs-changed", ());
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    // Play next track on audio thread
    state.audio.play(track_path, state.level_monitor.clone());

    let _ = app.emit("transport-changed", ());
    let _ = app.emit("logs-changed", ());

    Ok(())
}

#[tauri::command]
fn transport_seek(
    state: State<AppState>,
    app: AppHandle,
    position_secs: f64,
) -> Result<(), String> {
    // Update timing in core
    {
        let mut core = state.core.lock().unwrap();
        core.on_seek(position_secs)?;
    }

    // Seek on audio thread
    let seek_pos = Duration::from_secs_f64(position_secs.max(0.0));
    state.audio.seek(seek_pos);

    let _ = app.emit("transport-changed", ());

    Ok(())
}

#[tauri::command]
fn transport_status(state: State<AppState>) -> TransportData {
    // Simplified: only locks core, no player check needed.
    // Track-end detection is handled by the audio runtime thread.
    state.core.lock().unwrap().get_transport_state()
}

// ── Audio Level ─────────────────────────────────────────────────────────────

#[tauri::command]
fn get_audio_level(state: State<AppState>) -> f32 {
    state.level_monitor.level()
}

// ── Waveform ────────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_waveform(path: String) -> Result<Vec<f32>, String> {
    tokio::task::spawn_blocking(move || AppCore::get_waveform(&path))
        .await
        .map_err(|e| format!("Waveform task failed: {}", e))?
}

// ── Schedule ────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_schedule(state: State<AppState>) -> Vec<ScheduleEventData> {
    state.core.lock().unwrap().get_schedule()
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
    state
        .core
        .lock()
        .unwrap()
        .add_schedule_event(&time, &mode, &file, priority, label, days)
}

#[tauri::command]
fn remove_schedule_event(state: State<AppState>, id: u32) -> Result<(), String> {
    state.core.lock().unwrap().remove_schedule_event(id)
}

#[tauri::command]
fn toggle_schedule_event(state: State<AppState>, id: u32) -> Result<bool, String> {
    state.core.lock().unwrap().toggle_schedule_event(id)
}

// ── Config ──────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_config(state: State<AppState>) -> ConfigData {
    state.core.lock().unwrap().get_config()
}

#[tauri::command]
async fn list_output_devices() -> Vec<String> {
    // Device enumeration via cpal can stall 200-500ms on Windows — run it
    // off the async executor so we don't block other IPC commands.
    tokio::task::spawn_blocking(signal_flow::player::list_output_devices)
        .await
        .unwrap_or_default()
}

#[tauri::command]
fn set_output_device(state: State<AppState>, name: Option<String>) -> Result<(), String> {
    state.core.lock().unwrap().set_output_device(name.clone())?;
    state.audio.set_device(name);
    Ok(())
}

#[tauri::command]
fn set_crossfade(state: State<AppState>, secs: f32) -> Result<(), String> {
    state.core.lock().unwrap().set_crossfade(secs)
}

#[tauri::command]
fn set_silence_detection(
    state: State<AppState>,
    threshold: f32,
    duration_secs: f32,
) -> Result<(), String> {
    state
        .core
        .lock()
        .unwrap()
        .set_silence_detection(threshold, duration_secs)
}

#[tauri::command]
fn set_intros_folder(state: State<AppState>, path: Option<String>) -> Result<(), String> {
    state.core.lock().unwrap().set_intros_folder(path)
}

#[tauri::command]
fn set_recurring_intro(
    state: State<AppState>,
    interval_secs: f32,
    duck_volume: f32,
) -> Result<(), String> {
    state
        .core
        .lock()
        .unwrap()
        .set_recurring_intro(interval_secs, duck_volume)
}

#[tauri::command]
fn set_conflict_policy(state: State<AppState>, policy: String) -> Result<(), String> {
    state.core.lock().unwrap().set_conflict_policy(&policy)
}

#[tauri::command]
fn set_stream_output(
    state: State<AppState>,
    enabled: bool,
    endpoint_url: String,
) -> Result<(), String> {
    state
        .core
        .lock()
        .unwrap()
        .set_stream_output(enabled, endpoint_url)
}

#[tauri::command]
fn set_recording(
    state: State<AppState>,
    enabled: bool,
    output_dir: Option<String>,
) -> Result<(), String> {
    state
        .core
        .lock()
        .unwrap()
        .set_recording(enabled, output_dir)
}

#[tauri::command]
fn set_indexed_locations(state: State<AppState>, locations: Vec<String>) -> Result<(), String> {
    state.core.lock().unwrap().set_indexed_locations(locations)
}

#[tauri::command]
fn set_favorite_folders(state: State<AppState>, folders: Vec<String>) -> Result<(), String> {
    state.core.lock().unwrap().set_favorite_folders(folders)
}

#[tauri::command]
fn set_nowplaying_path(state: State<AppState>, path: Option<String>) -> Result<(), String> {
    state.core.lock().unwrap().set_nowplaying_path(path)
}

// ── Ads ─────────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_ads(state: State<AppState>) -> Vec<AdData> {
    state.core.lock().unwrap().get_ads()
}

#[tauri::command]
fn add_ad(state: State<AppState>, name: String, mp3_file: String) -> Result<usize, String> {
    state.core.lock().unwrap().add_ad(name, mp3_file)
}

#[tauri::command]
fn remove_ad(state: State<AppState>, index: usize) -> Result<(), String> {
    state.core.lock().unwrap().remove_ad(index)
}

#[tauri::command]
fn toggle_ad(state: State<AppState>, index: usize) -> Result<bool, String> {
    state.core.lock().unwrap().toggle_ad(index)
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
    state
        .core
        .lock()
        .unwrap()
        .update_ad(index, name, enabled, mp3_file, scheduled, days, hours)
}

#[tauri::command]
fn reorder_ad(state: State<AppState>, from: usize, to: usize) -> Result<(), String> {
    state.core.lock().unwrap().reorder_ad(from, to)
}

// ── Ad Statistics & Reports ──────────────────────────────────────────────────

#[tauri::command]
fn get_ad_stats(
    state: State<AppState>,
    start: Option<String>,
    end: Option<String>,
) -> signal_flow::ad_logger::AdStatistics {
    state
        .core
        .lock()
        .unwrap()
        .get_ad_stats(start.as_deref(), end.as_deref())
}

#[tauri::command]
fn get_ad_daily_counts(state: State<AppState>, ad_name: String) -> Vec<AdDailyCountResponse> {
    state
        .core
        .lock()
        .unwrap()
        .get_ad_daily_counts(&ad_name)
        .into_iter()
        .map(|(date, count)| AdDailyCountResponse { date, count })
        .collect()
}

#[tauri::command]
fn get_ad_failures(state: State<AppState>) -> Vec<AdFailureResponse> {
    state
        .core
        .lock()
        .unwrap()
        .get_ad_failures()
        .into_iter()
        .map(|f| AdFailureResponse {
            timestamp: f.t,
            ads: f.ads,
            error: f.err,
        })
        .collect()
}

#[tauri::command]
async fn generate_ad_report(
    start: String,
    end: String,
    output_dir: String,
    ad_name: Option<String>,
    company_name: Option<String>,
) -> Result<Vec<String>, String> {
    // Report generation reads/writes files; run on blocking thread pool so we
    // don't stall the async runtime or hold the core mutex.
    tokio::task::spawn_blocking(move || {
        use signal_flow::ad_logger::AdPlayLogger;
        use signal_flow::ad_report::AdReportGenerator;
        use std::path::Path;
        let logger = AdPlayLogger::new(Path::new("."));
        let reporter = AdReportGenerator::new(&logger);
        let out_path = Path::new(&output_dir);
        if !out_path.is_dir() {
            return Err(format!("'{}' is not a valid directory", output_dir));
        }
        match ad_name.as_deref() {
            Some(name) => match reporter.generate_single_report(name, &start, &end, company_name.as_deref(), out_path) {
                Some(r) => Ok(vec![
                    r.csv_path.to_string_lossy().to_string(),
                    r.pdf_path.to_string_lossy().to_string(),
                ]),
                None => Ok(vec![]),
            },
            None => {
                let results = reporter.generate_report(&start, &end, company_name.as_deref(), out_path);
                Ok(results.into_iter().flat_map(|r| vec![
                    r.csv_path.to_string_lossy().to_string(),
                    r.pdf_path.to_string_lossy().to_string(),
                ]).collect())
            }
        }
    })
    .await
    .map_err(|e| format!("Report task panicked: {e}"))?
}

// ── RDS ─────────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_rds_config(state: State<AppState>) -> RdsConfigData {
    state.core.lock().unwrap().get_rds_config()
}

#[tauri::command]
fn add_rds_message(state: State<AppState>, text: String) -> Result<usize, String> {
    state.core.lock().unwrap().add_rds_message(text)
}

#[tauri::command]
fn remove_rds_message(state: State<AppState>, index: usize) -> Result<(), String> {
    state.core.lock().unwrap().remove_rds_message(index)
}

#[tauri::command]
fn toggle_rds_message(state: State<AppState>, index: usize) -> Result<bool, String> {
    state.core.lock().unwrap().toggle_rds_message(index)
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
    state
        .core
        .lock()
        .unwrap()
        .update_rds_message(index, text, enabled, duration, scheduled, days, hours)
}

#[tauri::command]
fn reorder_rds_message(state: State<AppState>, from: usize, to: usize) -> Result<(), String> {
    state.core.lock().unwrap().reorder_rds_message(from, to)
}

#[tauri::command]
fn update_rds_settings(
    state: State<AppState>,
    ip: String,
    port: u16,
    default_message: String,
) -> Result<(), String> {
    state
        .core
        .lock()
        .unwrap()
        .update_rds_settings(ip, port, default_message)
}

// ── Logs ────────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_logs(state: State<AppState>) -> Vec<LogEntry> {
    state.core.lock().unwrap().get_logs(None)
}

#[tauri::command]
fn clear_logs(state: State<AppState>) {
    state.core.lock().unwrap().clear_logs();
}

// ── App entry ───────────────────────────────────────────────────────────────

fn main() {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("signalFlow");
    let state_path = data_dir.join("signalflow_state.json");
    let level_monitor = LevelMonitor::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let core = Arc::new(Mutex::new(AppCore::new(&state_path)));
            let app_handle = app.handle().clone();
            let level_monitor_for_audio = level_monitor.clone();
            let audio_for_callback: Arc<Mutex<Option<AudioHandle>>> = Arc::new(Mutex::new(None));

            // Spawn audio runtime with event callback
            let initial_device = core.lock().unwrap().get_config().output_device_name;
            let core_for_audio = core.clone();
            let audio_for_callback_clone = audio_for_callback.clone();
            let audio = spawn_audio_runtime(initial_device, move |event| {
                match event {
                    AudioEvent::TrackFinished => {
                        let next_track = {
                            let mut core = core_for_audio.lock().unwrap();
                            core.prepare_skip()
                        };

                        match next_track {
                            Ok((track_path, ..)) => {
                                if let Some(audio) =
                                    audio_for_callback_clone.lock().unwrap().as_ref()
                                {
                                    audio.play(track_path, level_monitor_for_audio.clone());
                                }
                            }
                            Err(ref e) if e == "__end_of_playlist__" => {
                                // End of playlist is handled in AppCore::prepare_skip.
                            }
                            Err(e) => {
                                let mut core = core_for_audio.lock().unwrap();
                                core.on_stop();
                                core.log("error", format!("Auto-advance failed: {}", e));
                            }
                        }
                        let _ = app_handle.emit("transport-changed", ());
                        let _ = app_handle.emit("logs-changed", ());
                    }
                    AudioEvent::PlayError(ref e) => {
                        let mut c = core_for_audio.lock().unwrap();
                        c.on_stop();
                        c.log("error", format!("Audio error: {}", e));
                        let _ = app_handle.emit("transport-changed", ());
                        let _ = app_handle.emit("logs-changed", ());
                    }
                    AudioEvent::Playing
                    | AudioEvent::Stopped
                    | AudioEvent::Paused
                    | AudioEvent::Resumed
                    | AudioEvent::Seeked(_) => {
                        let _ = app_handle.emit("transport-changed", ());
                    }
                }
            });
            *audio_for_callback.lock().unwrap() = Some(audio.clone());

            app.manage(AppState {
                core,
                audio,
                level_monitor,
            });

            Ok(())
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
            get_playlist_profiles,
            save_playlist_profile,
            load_playlist_profile,
            delete_playlist_profile,
            import_m3u_playlist,
            export_playlist_to_m3u,
            // Track operations
            get_playlist_tracks,
            add_track,
            add_tracks,
            remove_tracks,
            reorder_track,
            copy_paste_tracks,
            edit_track_metadata,
            list_directory,
            search_indexed_files,
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
            // Ad Statistics & Reports
            get_ad_stats,
            get_ad_daily_counts,
            get_ad_failures,
            generate_ad_report,
            // RDS
            get_rds_config,
            add_rds_message,
            remove_rds_message,
            toggle_rds_message,
            update_rds_message,
            reorder_rds_message,
            update_rds_settings,
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
            set_stream_output,
            set_recording,
            set_indexed_locations,
            set_favorite_folders,
            set_nowplaying_path,
            list_output_devices,
            set_output_device,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
