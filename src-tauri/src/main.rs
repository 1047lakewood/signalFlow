#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use signal_flow::app_core::{
    AppCore, AdData, ConfigData, LogEntry, PlaylistData, RdsConfigData,
    ScheduleEventData, StatusData, TrackData, TransportData,
};
use signal_flow::level_monitor::LevelMonitor;
use signal_flow::player::Player;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;

/// Wrapper to make Player Send+Sync for Tauri state.
/// Safety: Player is only ever accessed behind a Mutex, ensuring exclusive access.
struct SendPlayer(Option<Player>);
unsafe impl Send for SendPlayer {}
unsafe impl Sync for SendPlayer {}

struct AppState {
    core: Mutex<AppCore>,
    player: Mutex<SendPlayer>,
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
fn rename_playlist(state: State<AppState>, old_name: String, new_name: String) -> Result<(), String> {
    state.core.lock().unwrap().rename_playlist(&old_name, new_name)
}

#[tauri::command]
fn set_active_playlist(state: State<AppState>, name: String) -> Result<u32, String> {
    state.core.lock().unwrap().set_active_playlist(&name)
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
fn add_tracks(state: State<AppState>, playlist: String, paths: Vec<String>) -> Result<usize, String> {
    state.core.lock().unwrap().add_tracks(&playlist, &paths)
}

#[tauri::command]
fn remove_tracks(state: State<AppState>, playlist: String, indices: Vec<usize>) -> Result<(), String> {
    state.core.lock().unwrap().remove_tracks(&playlist, &indices)
}

#[tauri::command]
fn reorder_track(state: State<AppState>, playlist: String, from: usize, to: usize) -> Result<(), String> {
    state.core.lock().unwrap().reorder_track(&playlist, from, to)
}

#[tauri::command]
fn edit_track_metadata(
    state: State<AppState>,
    playlist: String,
    track_index: usize,
    artist: Option<String>,
    title: Option<String>,
) -> Result<(), String> {
    state.core.lock().unwrap().edit_track_metadata(&playlist, track_index, artist.as_deref(), title.as_deref())
}

// ── Transport controls ─────────────────────────────────────────────────────

fn ensure_player(player_lock: &Mutex<SendPlayer>) -> Result<(), String> {
    let mut sp = player_lock.lock().unwrap();
    if sp.0.is_none() {
        sp.0 = Some(Player::new()?);
    }
    Ok(())
}

#[tauri::command]
fn transport_play(state: State<AppState>, track_index: Option<usize>) -> Result<(), String> {
    ensure_player(&state.player)?;

    // 1. Lock core: prepare play state (updates engine, playback, logs)
    let (track_path, ..) = {
        let mut core = state.core.lock().unwrap();
        core.prepare_play(track_index)?
    }; // core lock dropped

    // 2. Decode file BEFORE locking player (file I/O is slow)
    let prepared = Player::prepare_file_with_level(&track_path, state.level_monitor.clone())?;

    // 3. Lock player briefly: stop + play prepared source
    {
        state.level_monitor.reset();
        let player_guard = state.player.lock().unwrap();
        let player = player_guard.0.as_ref().unwrap();
        player.stop_and_play_prepared(prepared);
    } // player lock dropped

    Ok(())
}

#[tauri::command]
fn transport_stop(state: State<AppState>) -> Result<(), String> {
    // 1. Stop the player
    {
        let player_guard = state.player.lock().unwrap();
        if let Some(player) = player_guard.0.as_ref() {
            player.stop();
        }
    } // player lock dropped

    state.level_monitor.reset();

    // 2. Update core state (resets playback + logs)
    {
        let mut core = state.core.lock().unwrap();
        core.on_stop();
    } // core lock dropped

    Ok(())
}

#[tauri::command]
fn transport_pause(state: State<AppState>) -> Result<(), String> {
    // 1. Toggle pause state in core (validates + updates playback + logs)
    let now_paused = {
        let mut core = state.core.lock().unwrap();
        core.on_pause_toggle()?
    }; // core lock dropped

    // 2. Perform player action
    {
        let player_guard = state.player.lock().unwrap();
        let player = player_guard.0.as_ref().ok_or("No audio player")?;
        if now_paused {
            player.pause();
        } else {
            player.resume();
        }
    } // player lock dropped

    Ok(())
}

#[tauri::command]
fn transport_skip(state: State<AppState>) -> Result<(), String> {
    // 1. Stop current playback
    {
        let player_guard = state.player.lock().unwrap();
        if let Some(player) = player_guard.0.as_ref() {
            player.stop();
        }
    } // player lock dropped

    // 2. Advance to next track in core (updates engine, playback, logs)
    let skip_result = {
        let mut core = state.core.lock().unwrap();
        core.prepare_skip()
    }; // core lock dropped

    let (track_path, ..) = match skip_result {
        Ok(data) => data,
        Err(ref e) if e == "__end_of_playlist__" => return Ok(()),
        Err(e) => return Err(e),
    };

    // 3. Play next track
    {
        ensure_player(&state.player)?;
        let player_guard = state.player.lock().unwrap();
        let player = player_guard.0.as_ref().unwrap();
        player.play_file(&track_path)?;
    } // player lock dropped

    Ok(())
}

#[tauri::command]
fn transport_seek(state: State<AppState>, position_secs: f64) -> Result<(), String> {
    // 1. Update timing in core
    {
        let mut core = state.core.lock().unwrap();
        core.on_seek(position_secs)?;
    } // core lock dropped

    // 2. Seek on the player
    let seek_pos = Duration::from_secs_f64(position_secs.max(0.0));
    {
        let player_guard = state.player.lock().unwrap();
        let player = player_guard.0.as_ref().ok_or("No audio player")?;
        player.try_seek(seek_pos)?;
    } // player lock dropped

    Ok(())
}

#[tauri::command]
fn transport_status(state: State<AppState>) -> TransportData {
    // 1. Get transport state from core
    let mut transport = {
        let core = state.core.lock().unwrap();
        core.get_transport_state()
    }; // core lock dropped

    // 2. Check if the sink has finished playing (track ended naturally)
    if transport.is_playing && !transport.is_paused {
        let player_guard = state.player.lock().unwrap();
        let actually_playing = if let Some(player) = player_guard.0.as_ref() {
            !player.is_empty()
        } else {
            false
        };
        transport.is_playing = actually_playing;
    } // player lock dropped

    transport
}

// ── Audio Level ─────────────────────────────────────────────────────────────

#[tauri::command]
fn get_audio_level(state: State<AppState>) -> f32 {
    state.level_monitor.level()
}

// ── Waveform ────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_waveform(path: String) -> Result<Vec<f32>, String> {
    AppCore::get_waveform(&path)
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
    state.core.lock().unwrap().add_schedule_event(&time, &mode, &file, priority, label, days)
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
fn set_crossfade(state: State<AppState>, secs: f32) -> Result<(), String> {
    state.core.lock().unwrap().set_crossfade(secs)
}

#[tauri::command]
fn set_silence_detection(state: State<AppState>, threshold: f32, duration_secs: f32) -> Result<(), String> {
    state.core.lock().unwrap().set_silence_detection(threshold, duration_secs)
}

#[tauri::command]
fn set_intros_folder(state: State<AppState>, path: Option<String>) -> Result<(), String> {
    state.core.lock().unwrap().set_intros_folder(path)
}

#[tauri::command]
fn set_recurring_intro(state: State<AppState>, interval_secs: f32, duck_volume: f32) -> Result<(), String> {
    state.core.lock().unwrap().set_recurring_intro(interval_secs, duck_volume)
}

#[tauri::command]
fn set_conflict_policy(state: State<AppState>, policy: String) -> Result<(), String> {
    state.core.lock().unwrap().set_conflict_policy(&policy)
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
    state.core.lock().unwrap().update_ad(index, name, enabled, mp3_file, scheduled, days, hours)
}

#[tauri::command]
fn reorder_ad(state: State<AppState>, from: usize, to: usize) -> Result<(), String> {
    state.core.lock().unwrap().reorder_ad(from, to)
}

// ── Ad Statistics & Reports ──────────────────────────────────────────────────

#[tauri::command]
fn get_ad_stats(state: State<AppState>, start: Option<String>, end: Option<String>) -> signal_flow::ad_logger::AdStatistics {
    state.core.lock().unwrap().get_ad_stats(start.as_deref(), end.as_deref())
}

#[tauri::command]
fn get_ad_daily_counts(state: State<AppState>, ad_name: String) -> Vec<AdDailyCountResponse> {
    state.core.lock().unwrap()
        .get_ad_daily_counts(&ad_name)
        .into_iter()
        .map(|(date, count)| AdDailyCountResponse { date, count })
        .collect()
}

#[tauri::command]
fn get_ad_failures(state: State<AppState>) -> Vec<AdFailureResponse> {
    state.core.lock().unwrap()
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
fn generate_ad_report(
    state: State<AppState>,
    start: String,
    end: String,
    output_dir: String,
    ad_name: Option<String>,
    company_name: Option<String>,
) -> Result<Vec<String>, String> {
    state.core.lock().unwrap().generate_ad_report(&start, &end, &output_dir, ad_name.as_deref(), company_name.as_deref())
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
    state.core.lock().unwrap().update_rds_message(index, text, enabled, duration, scheduled, days, hours)
}

#[tauri::command]
fn reorder_rds_message(state: State<AppState>, from: usize, to: usize) -> Result<(), String> {
    state.core.lock().unwrap().reorder_rds_message(from, to)
}

#[tauri::command]
fn update_rds_settings(state: State<AppState>, ip: String, port: u16, default_message: String) -> Result<(), String> {
    state.core.lock().unwrap().update_rds_settings(ip, port, default_message)
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
    let core = AppCore::new(&state_path);
    let level_monitor = LevelMonitor::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            core: Mutex::new(core),
            player: Mutex::new(SendPlayer(None)),
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
            set_nowplaying_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
