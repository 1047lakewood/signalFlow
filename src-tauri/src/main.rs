#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
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

/// Shared editor playback state — written by editor IPC commands, read by editor_status.
#[derive(Default)]
struct EditorPlaybackState {
    is_playing: bool,
    start_secs: f64,
    started_at: Option<std::time::Instant>,
}

struct AppState {
    core: Arc<Mutex<AppCore>>,
    audio: AudioHandle,
    level_monitor: LevelMonitor,
    /// Dedicated audio handle for the in-app editor (independent of main transport).
    editor_audio: AudioHandle,
    /// Shared editor playback position tracking.
    editor_info: Arc<Mutex<EditorPlaybackState>>,
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
async fn list_available_drives() -> Vec<String> {
    tokio::task::spawn_blocking(signal_flow::app_core::list_available_drives)
        .await
        .unwrap_or_default()
}

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

// ── File / shell operations ──────────────────────────────────────────────────

/// Open the containing folder of a file in the system file manager,
/// with the file highlighted (Windows: explorer /select, macOS: open -R,
/// Linux: xdg-open on parent dir).
#[tauri::command]
fn open_file_location(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| format!("Failed to open file location: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| format!("Failed to open file location: {e}"))?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let parent = p
            .parent()
            .ok_or_else(|| "File has no parent directory".to_string())?;
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| format!("Failed to open file location: {e}"))?;
    }
    let _ = p; // suppress unused warning on non-Linux paths
    Ok(())
}

/// Open a file in Audacity (or the platform's default Audacity command).
#[tauri::command]
fn open_in_audacity(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let audacity = r"C:\Program Files\Audacity\audacity.exe";
        std::process::Command::new(audacity)
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to launch Audacity: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-a", "Audacity", &path])
            .spawn()
            .map_err(|e| format!("Failed to launch Audacity: {e}"))?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("audacity")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to launch Audacity: {e}"))?;
    }
    Ok(())
}

/// Update a track's path in the playlist without touching the file system.
/// Re-reads metadata from the new path and saves state.
#[tauri::command]
fn update_track_path(
    state: State<AppState>,
    playlist: String,
    track_index: usize,
    new_path: String,
) -> Result<(), String> {
    state
        .core
        .lock()
        .unwrap()
        .update_track_path(&playlist, track_index, std::path::Path::new(&new_path))
}

/// Rename / move a track's file on disk then update the playlist path.
/// Implements the truth-table from the spec:
///   original exists + new free  → rename/move (create dirs as needed)
///   original exists + new file  → overwrite move
///   original exists + new dir   → move into dir keeping filename
///   original absent + any       → path-only update, no file ops
#[tauri::command]
async fn rename_track_file(
    state: State<'_, AppState>,
    playlist: String,
    track_index: usize,
    new_path: String,
) -> Result<(), String> {
    use std::path::Path;

    // Grab current path while holding lock briefly.
    let old_path_str = {
        let core = state.core.lock().unwrap();
        let tracks = core.get_playlist_tracks(&playlist)?;
        tracks
            .get(track_index)
            .map(|t| t.path.clone())
            .ok_or_else(|| format!("Track index {track_index} out of range"))?
    };

    let old_path = std::path::PathBuf::from(&old_path_str);
    let mut target = std::path::PathBuf::from(&new_path);

    let resolved_path = tokio::task::spawn_blocking(move || -> Result<String, String> {
        if old_path.exists() {
            // Resolve directory target → keep original filename
            if target.is_dir() {
                let fname = old_path
                    .file_name()
                    .ok_or_else(|| "Source has no filename".to_string())?;
                target = target.join(fname);
            }
            // Create parent dirs if needed
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Cannot create directories: {e}"))?;
            }
            std::fs::rename(&old_path, &target)
                .map_err(|e| format!("Failed to rename file: {e}"))?;
        }
        // Path-only update in both cases (file existed or not)
        Ok(target.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("Rename task panicked: {e}"))??;

    state
        .core
        .lock()
        .unwrap()
        .update_track_path(&playlist, track_index, Path::new(&resolved_path))
}

/// Convert one or more tracks to MP3 using ffmpeg.
/// Returns a summary: (converted, skipped, failed).
#[tauri::command]
async fn convert_tracks_to_mp3(
    state: State<'_, AppState>,
    playlist: String,
    indices: Vec<usize>,
) -> Result<(usize, usize, usize), String> {
    use std::path::Path;

    // Grab paths while holding lock briefly.
    let paths: Vec<(usize, String)> = {
        let core = state.core.lock().unwrap();
        let tracks = core.get_playlist_tracks(&playlist)?;
        indices
            .iter()
            .filter_map(|&i| tracks.get(i).map(|t| (i, t.path.clone())))
            .collect()
    };

    let mut converted = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for (idx, path_str) in paths {
        let p = std::path::PathBuf::from(&path_str);
        let ext = p
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if ext == "mp3" {
            skipped += 1;
            continue;
        }

        let mp3_path = p.with_extension("mp3");
        let p_clone = p.clone();
        let mp3_clone = mp3_path.clone();

        let ok = tokio::task::spawn_blocking(move || -> bool {
            let status = std::process::Command::new("ffmpeg")
                .args([
                    "-i",
                    p_clone.to_str().unwrap_or(""),
                    "-q:a",
                    "0",
                    mp3_clone.to_str().unwrap_or(""),
                    "-y",
                ])
                .status();
            match status {
                Ok(s) if s.success() => {
                    // Delete original
                    let _ = std::fs::remove_file(&p_clone);
                    true
                }
                _ => false,
            }
        })
        .await
        .unwrap_or(false);

        if ok {
            // Update track path in playlist
            if let Ok(()) = state
                .core
                .lock()
                .unwrap()
                .update_track_path(&playlist, idx, Path::new(&mp3_path))
            {
                converted += 1;
            } else {
                failed += 1;
            }
        } else {
            failed += 1;
        }
    }

    Ok((converted, skipped, failed))
}

/// Replace a track's file with the processed version from the `macro-output`
/// subfolder in the same directory. Waits for the file to finish writing
/// (stable size for 2 consecutive 500ms checks) before moving it.
#[tauri::command]
async fn replace_from_macro_output(
    state: State<'_, AppState>,
    playlist: String,
    track_index: usize,
) -> Result<String, String> {
    use std::path::Path;

    let path_str = {
        let core = state.core.lock().unwrap();
        let tracks = core.get_playlist_tracks(&playlist)?;
        tracks
            .get(track_index)
            .map(|t| t.path.clone())
            .ok_or_else(|| format!("Track index {track_index} out of range"))?
    };

    let new_path = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let orig = Path::new(&path_str);
        let parent = orig
            .parent()
            .ok_or_else(|| "File has no parent directory".to_string())?;
        let macro_dir = parent.join("macro-output");

        if !macro_dir.is_dir() {
            return Err(format!(
                "No macro-output folder found in '{}'",
                parent.display()
            ));
        }

        let stem = orig
            .file_stem()
            .ok_or_else(|| "Source file has no stem".to_string())?
            .to_string_lossy()
            .to_lowercase();

        // Find a file in macro-output whose stem matches (case-insensitive)
        let entry = std::fs::read_dir(&macro_dir)
            .map_err(|e| format!("Cannot read macro-output: {e}"))?
            .filter_map(|e| e.ok())
            .find(|e| {
                let name = e.file_name();
                let s = name.to_string_lossy();
                let entry_stem = Path::new(s.as_ref())
                    .file_stem()
                    .map(|x| x.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                entry_stem == stem
            })
            .ok_or_else(|| format!("No matching file in macro-output for '{}'", stem))?;

        let macro_file = entry.path();

        // Wait for file to be stable (2 × 500ms checks)
        let mut prev_size = macro_file
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let size1 = macro_file
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);
        if size1 != prev_size {
            prev_size = size1;
            std::thread::sleep(std::time::Duration::from_millis(500));
            let size2 = macro_file
                .metadata()
                .map(|m| m.len())
                .unwrap_or(0);
            if size2 != prev_size {
                return Err("Macro output file is still being written. Try again shortly.".to_string());
            }
        }

        // Determine destination path (may differ in extension)
        let macro_ext = macro_file
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        let orig_ext = orig
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let dest = parent.join(
            macro_file
                .file_name()
                .ok_or_else(|| "macro-output file has no name".to_string())?,
        );

        std::fs::rename(&macro_file, &dest)
            .map_err(|e| format!("Failed to move macro output file: {e}"))?;

        // Verify destination is non-empty
        let dest_size = dest.metadata().map(|m| m.len()).unwrap_or(0);
        if dest_size == 0 {
            return Err("Moved file is empty at destination".to_string());
        }

        // Delete original if extension changed
        if macro_ext.to_lowercase() != orig_ext && orig.exists() {
            let _ = std::fs::remove_file(orig);
        }

        // Clean up empty macro-output dir
        if let Ok(mut dir_entries) = std::fs::read_dir(&macro_dir) {
            if dir_entries.next().is_none() {
                let _ = std::fs::remove_dir(&macro_dir);
            }
        }

        Ok(dest.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("Replace task panicked: {e}"))??;

    state
        .core
        .lock()
        .unwrap()
        .update_track_path(&playlist, track_index, std::path::Path::new(&new_path))?;

    Ok(new_path)
}

/// Append " AM" to a track's filename and title tag.
/// Renames the file on disk (if it exists) and updates in-playlist metadata.
#[tauri::command]
async fn add_am_to_filename(
    state: State<'_, AppState>,
    playlist: String,
    track_index: usize,
) -> Result<(), String> {
    use std::path::Path;

    let path_str = {
        let core = state.core.lock().unwrap();
        let tracks = core.get_playlist_tracks(&playlist)?;
        tracks
            .get(track_index)
            .map(|t| t.path.clone())
            .ok_or_else(|| format!("Track index {track_index} out of range"))?
    };

    let p = std::path::PathBuf::from(&path_str);
    let stem = p
        .file_stem()
        .ok_or_else(|| "File has no stem".to_string())?
        .to_string_lossy()
        .to_string();

    if stem.ends_with(" AM") {
        return Err("Filename already ends with \" AM\"".to_string());
    }

    let ext = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let new_filename = format!("{} AM{}", stem, ext);
    let parent = p
        .parent()
        .ok_or_else(|| "File has no parent directory".to_string())?;
    let new_path = parent.join(&new_filename);

    if new_path.exists() {
        return Err(format!("'{}' already exists", new_path.display()));
    }

    let path_str_clone = path_str.clone();
    let new_path_clone = new_path.clone();
    let stem_clone = stem.clone();

    let final_path = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let orig = Path::new(&path_str_clone);
        if orig.exists() {
            std::fs::rename(orig, &new_path_clone)
                .map_err(|e| format!("Failed to rename file: {e}"))?;
        }

        // Write " AM" title tag if file now exists
        let target = if new_path_clone.exists() {
            new_path_clone.clone()
        } else {
            // File didn't exist (path-only mode) - can't write tags
            return Ok(new_path_clone.to_string_lossy().to_string());
        };

        // Update the title tag via lofty
        use lofty::file::TaggedFileExt;
        use lofty::prelude::TagExt;
        use lofty::tag::Accessor;
        let mut tagged = lofty::read_from_path(&target)
            .map_err(|e| format!("Failed to read tags: {e}"))?;
        let tag = match tagged.primary_tag_mut() {
            Some(t) => t,
            None => match tagged.first_tag_mut() {
                Some(t) => t,
                None => {
                    let tag_type = tagged.primary_tag_type();
                    tagged.insert_tag(lofty::tag::Tag::new(tag_type));
                    tagged.primary_tag_mut().unwrap()
                }
            },
        };

        let current_title: String = tag
            .title()
            .map(|t| t.to_string())
            .unwrap_or_else(|| stem_clone.clone());
        let new_title = if current_title.ends_with(" AM") {
            current_title
        } else {
            format!("{} AM", current_title)
        };
        tag.set_title(new_title);

        tag.save_to_path(&target, lofty::config::WriteOptions::default())
            .map_err(|e| format!("Failed to write title tag: {e}"))?;

        Ok(target.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("Add AM task panicked: {e}"))??;

    state
        .core
        .lock()
        .unwrap()
        .update_track_path(&playlist, track_index, std::path::Path::new(&final_path))
}

// ── In-App Audio Editor ─────────────────────────────────────────────────────

/// High-resolution peak data returned to the editor frontend.
#[derive(Serialize)]
struct EditorWaveformResponse {
    peaks: Vec<f32>,
    duration_secs: f64,
    sample_rate: u32,
    num_peaks: usize,
    resolution_ms: u32,
}

/// Audio file technical information.
#[derive(Serialize)]
struct AudioFileInfo {
    format: String,
    duration_secs: f64,
    sample_rate: u32,
    channels: u32,
    bitrate_kbps: u32,
    file_size_bytes: u64,
}

/// Editor playback status returned to the frontend.
#[derive(Serialize)]
struct EditorStatusData {
    is_playing: bool,
    position_secs: f64,
}

/// Export request sent from the frontend.
#[derive(Deserialize)]
struct ExportRequest {
    input_path: String,
    output_path: String,
    format: String,
    quality: u8,
    operations: signal_flow::audio_editor::EditorOperations,
}

/// Fetch high-resolution waveform peaks for the audio editor.
/// `resolution_ms` = milliseconds per peak (10 → ~100 peaks/sec). Cached on disk.
#[tauri::command]
async fn get_editor_waveform(
    path: String,
    resolution_ms: u32,
) -> Result<EditorWaveformResponse, String> {
    let resolution_ms = resolution_ms.clamp(5, 500);
    tokio::task::spawn_blocking(move || {
        let data = signal_flow::waveform::generate_editor_peaks_cached(
            std::path::Path::new(&path),
            resolution_ms,
        )?;
        Ok(EditorWaveformResponse {
            peaks: data.peaks,
            duration_secs: data.duration_secs,
            sample_rate: data.sample_rate,
            num_peaks: data.num_peaks,
            resolution_ms: data.resolution_ms,
        })
    })
    .await
    .map_err(|e| format!("Waveform task panicked: {e}"))?
}

/// Return technical information about an audio file using lofty.
#[tauri::command]
async fn get_audio_info(path: String) -> Result<AudioFileInfo, String> {
    tokio::task::spawn_blocking(move || {
        use lofty::file::AudioFile;
        let p = std::path::Path::new(&path);
        let file_size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let tagged =
            lofty::read_from_path(p).map_err(|e| format!("Cannot read audio info: {e}"))?;
        let props = tagged.properties();
        Ok(AudioFileInfo {
            format: ext.to_uppercase(),
            duration_secs: props.duration().as_secs_f64(),
            sample_rate: props.sample_rate().unwrap_or(0),
            channels: props.channels().unwrap_or(0) as u32,
            bitrate_kbps: props.audio_bitrate().unwrap_or(0) as u32,
            file_size_bytes: file_size,
        })
    })
    .await
    .map_err(|e| format!("Audio info task panicked: {e}"))?
}

/// Start editor audio playback from `start_secs` into the file.
/// Sends Play then Seek to the dedicated editor audio handle.
#[tauri::command]
fn editor_play(state: State<AppState>, path: String, start_secs: f64) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);
    state
        .editor_audio
        .play(path_buf, state.level_monitor.clone());
    if start_secs > 0.01 {
        state
            .editor_audio
            .seek(std::time::Duration::from_secs_f64(start_secs.max(0.0)));
    }

    let mut info = state.editor_info.lock().unwrap();
    info.is_playing = true;
    info.start_secs = start_secs.max(0.0);
    info.started_at = Some(std::time::Instant::now());
    Ok(())
}

/// Stop editor audio playback, recording the current position for resume.
#[tauri::command]
fn editor_stop(state: State<AppState>) -> Result<(), String> {
    state.editor_audio.stop();
    let mut info = state.editor_info.lock().unwrap();
    if let Some(started_at) = info.started_at.take() {
        info.start_secs += started_at.elapsed().as_secs_f64();
    }
    info.is_playing = false;
    Ok(())
}

/// Seek the editor audio to `position_secs`.
#[tauri::command]
fn editor_seek(state: State<AppState>, position_secs: f64) -> Result<(), String> {
    let pos = std::time::Duration::from_secs_f64(position_secs.max(0.0));
    state.editor_audio.seek(pos);
    let mut info = state.editor_info.lock().unwrap();
    info.start_secs = position_secs.max(0.0);
    if info.is_playing {
        info.started_at = Some(std::time::Instant::now());
    }
    Ok(())
}

/// Return current editor playback status (position computed from elapsed wall time).
#[tauri::command]
fn editor_status(state: State<AppState>) -> EditorStatusData {
    let info = state.editor_info.lock().unwrap();
    let position_secs = if info.is_playing {
        if let Some(started_at) = info.started_at {
            info.start_secs + started_at.elapsed().as_secs_f64()
        } else {
            info.start_secs
        }
    } else {
        info.start_secs
    };
    EditorStatusData {
        is_playing: info.is_playing,
        position_secs,
    }
}

/// Export the edited audio file via ffmpeg.
/// Builds and runs the filter chain, writes to `output_path`.
#[tauri::command]
async fn export_edited_audio(request: ExportRequest) -> Result<String, String> {
    use signal_flow::audio_editor::{build_ffmpeg_args, run_ffmpeg};

    tokio::task::spawn_blocking(move || {
        let args = build_ffmpeg_args(
            &request.input_path,
            &request.output_path,
            &request.operations,
            &request.format,
            request.quality,
        );
        run_ffmpeg(&args)?;
        Ok(request.output_path)
    })
    .await
    .map_err(|e| format!("Export task panicked: {e}"))?
}

/// Scan an audio file for silence regions below `threshold_db` dB lasting at
/// least `min_duration_secs` seconds. Returns a list of silence regions.
#[tauri::command]
async fn detect_silence_regions(
    path: String,
    threshold_db: f64,
    min_duration_secs: f64,
) -> Result<Vec<signal_flow::audio_editor::SilenceRegion>, String> {
    tokio::task::spawn_blocking(move || {
        signal_flow::audio_editor::detect_silence_regions(
            std::path::Path::new(&path),
            threshold_db,
            min_duration_secs,
        )
    })
    .await
    .map_err(|e| format!("Silence detection task panicked: {e}"))?
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

            // Spawn dedicated editor audio runtime (independent of main transport)
            let editor_info: Arc<Mutex<EditorPlaybackState>> =
                Arc::new(Mutex::new(EditorPlaybackState::default()));
            let editor_info_for_cb = editor_info.clone();
            let editor_audio = spawn_audio_runtime(None, move |event| {
                // On track end or stop, mark editor as stopped
                match event {
                    AudioEvent::TrackFinished | AudioEvent::Stopped => {
                        let mut info = editor_info_for_cb.lock().unwrap();
                        info.is_playing = false;
                        info.started_at = None;
                    }
                    _ => {}
                }
            });

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
                editor_audio,
                editor_info,
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
            list_available_drives,
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
            // File / shell operations
            open_file_location,
            open_in_audacity,
            update_track_path,
            rename_track_file,
            convert_tracks_to_mp3,
            replace_from_macro_output,
            add_am_to_filename,
            // In-app audio editor
            get_editor_waveform,
            get_audio_info,
            editor_play,
            editor_stop,
            editor_seek,
            editor_status,
            export_edited_audio,
            detect_silence_regions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
