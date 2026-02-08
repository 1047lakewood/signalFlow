#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use signal_flow::engine::Engine;
use std::sync::Mutex;
use tauri::State;

struct AppState {
    engine: Mutex<Engine>,
}

#[tauri::command]
fn get_status(state: State<AppState>) -> String {
    let engine = state.engine.lock().unwrap();
    let playlist_count = engine.playlists.len();
    let active = engine
        .active_playlist()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "none".to_string());
    let schedule_count = engine.schedule.events.len();

    format!(
        "signalFlow v0.1.0\n\nPlaylists: {}\nActive: {}\nScheduled events: {}\nCrossfade: {}s\nConflict policy: {}",
        playlist_count,
        active,
        schedule_count,
        engine.crossfade_secs,
        engine.conflict_policy,
    )
}

fn main() {
    let engine = Engine::load();

    tauri::Builder::default()
        .manage(AppState {
            engine: Mutex::new(engine),
        })
        .invoke_handler(tauri::generate_handler![get_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
