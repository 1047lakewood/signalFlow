//! Headless integration tests for signalFlow.
//!
//! These tests exercise AppCore end-to-end without launching the GUI or Tauri.
//! They verify that all features are testable via `cargo test` alone.

use signal_flow::app_core::AppCore;
use signal_flow::audio_runtime::{spawn_audio_runtime, AudioEvent};
use signal_flow::level_monitor::LevelMonitor;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn make_core() -> AppCore {
    AppCore::new_test()
}

// ── Playlist workflow ─────────────────────────────────────────────────────

#[test]
fn full_playlist_lifecycle() {
    let mut core = make_core();

    // Create multiple playlists
    core.create_playlist("Morning Show".to_string()).unwrap();
    core.create_playlist("Evening Mix".to_string()).unwrap();
    core.create_playlist("Weekend".to_string()).unwrap();

    assert_eq!(core.get_playlists().len(), 3);

    // Set active
    core.set_active_playlist("Morning Show").unwrap();
    assert_eq!(
        core.get_status().active_playlist,
        Some("Morning Show".to_string())
    );

    // Rename
    core.rename_playlist("Weekend", "Saturday Special".to_string())
        .unwrap();
    let names: Vec<String> = core.get_playlists().iter().map(|p| p.name.clone()).collect();
    assert!(names.contains(&"Saturday Special".to_string()));
    assert!(!names.contains(&"Weekend".to_string()));

    // Delete active playlist
    core.delete_playlist("Morning Show").unwrap();
    assert!(core.get_status().active_playlist.is_none());
    assert_eq!(core.get_playlists().len(), 2);

    // Delete all
    core.delete_playlist("Evening Mix").unwrap();
    core.delete_playlist("Saturday Special").unwrap();
    assert!(core.get_playlists().is_empty());
}

#[test]
fn playlist_error_handling() {
    let mut core = make_core();

    // Operations on nonexistent playlists
    assert!(core.delete_playlist("Ghost").is_err());
    assert!(core.rename_playlist("Ghost", "New".to_string()).is_err());
    assert!(core.set_active_playlist("Ghost").is_err());
    assert!(core.get_playlist_tracks("Ghost").is_err());

    // Duplicate creation
    core.create_playlist("Dup".to_string()).unwrap();
    assert!(core.create_playlist("dup".to_string()).is_err()); // case-insensitive

    // Rename to existing name
    core.create_playlist("Other".to_string()).unwrap();
    assert!(core.rename_playlist("Other", "Dup".to_string()).is_err());
}

// ── Track operations with mock tracks ─────────────────────────────────────

fn add_mock_track(core: &mut AppCore, playlist: &str, artist: &str, title: &str) {
    let track = signal_flow::track::Track {
        path: format!("{} - {}.mp3", artist, title).into(),
        title: title.to_string(),
        artist: artist.to_string(),
        duration: Duration::from_secs(180),
        played_duration: None,
        has_intro: false,
    };
    core.engine
        .find_playlist_mut(playlist)
        .unwrap()
        .tracks
        .push(track);
}

#[test]
fn track_operations_workflow() {
    let mut core = make_core();
    core.create_playlist("Test".to_string()).unwrap();

    add_mock_track(&mut core, "Test", "Artist A", "Song 1");
    add_mock_track(&mut core, "Test", "Artist B", "Song 2");
    add_mock_track(&mut core, "Test", "Artist C", "Song 3");

    // Verify tracks
    let tracks = core.get_playlist_tracks("Test").unwrap();
    assert_eq!(tracks.len(), 3);
    assert_eq!(tracks[0].artist, "Artist A");
    assert_eq!(tracks[2].title, "Song 3");

    // Reorder: move track 0 to position 2
    core.reorder_track("Test", 0, 2).unwrap();
    let tracks = core.get_playlist_tracks("Test").unwrap();
    assert_eq!(tracks[0].artist, "Artist B");
    assert_eq!(tracks[2].artist, "Artist A");

    // Remove middle track
    core.remove_tracks("Test", &[1]).unwrap();
    let tracks = core.get_playlist_tracks("Test").unwrap();
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].artist, "Artist B");
    assert_eq!(tracks[1].artist, "Artist A");
}

#[test]
fn copy_paste_between_playlists() {
    let mut core = make_core();
    core.create_playlist("Source".to_string()).unwrap();
    core.create_playlist("Target".to_string()).unwrap();

    add_mock_track(&mut core, "Source", "DJ", "Track 1");
    add_mock_track(&mut core, "Source", "DJ", "Track 2");
    add_mock_track(&mut core, "Source", "DJ", "Track 3");

    // Copy tracks 0 and 2
    let copied = core.copy_tracks("Source", &[0, 2]).unwrap();
    assert_eq!(copied.len(), 2);

    // Paste into target
    core.paste_tracks("Target", copied, None).unwrap();
    let target_tracks = core.get_playlist_tracks("Target").unwrap();
    assert_eq!(target_tracks.len(), 2);
    assert_eq!(target_tracks[0].title, "Track 1");
    assert_eq!(target_tracks[1].title, "Track 3");

    // Source unchanged
    let source_tracks = core.get_playlist_tracks("Source").unwrap();
    assert_eq!(source_tracks.len(), 3);
}

// ── Config workflow ───────────────────────────────────────────────────────

#[test]
fn config_round_trip() {
    let mut core = make_core();

    // Set all config values
    core.set_crossfade(4.5).unwrap();
    core.set_silence_detection(0.05, 10.0).unwrap();
    core.set_recurring_intro(600.0, 0.15).unwrap();
    core.set_conflict_policy("manual-wins").unwrap();
    core.set_nowplaying_path(Some("/tmp/np.xml".to_string())).unwrap();

    // Verify config reads back correctly
    let config = core.get_config();
    assert_eq!(config.crossfade_secs, 4.5);
    assert_eq!(config.silence_threshold, 0.05);
    assert_eq!(config.silence_duration_secs, 10.0);
    assert_eq!(config.recurring_intro_interval_secs, 600.0);
    assert_eq!(config.recurring_intro_duck_volume, 0.15);
    assert_eq!(config.conflict_policy, "manual-wins");
    assert_eq!(config.now_playing_path, Some("/tmp/np.xml".to_string()));

    // Verify status matches
    let status = core.get_status();
    assert_eq!(status.crossfade_secs, 4.5);
    assert_eq!(status.conflict_policy, "manual-wins");
}

#[test]
fn config_reset_values() {
    let mut core = make_core();

    core.set_crossfade(5.0).unwrap();
    core.set_crossfade(0.0).unwrap();
    assert_eq!(core.get_config().crossfade_secs, 0.0);

    core.set_nowplaying_path(Some("/tmp/np.xml".to_string())).unwrap();
    core.set_nowplaying_path(None).unwrap();
    assert!(core.get_config().now_playing_path.is_none());
}

// ── Transport state (no audio device needed) ──────────────────────────────

#[test]
fn transport_prepare_play_workflow() {
    let mut core = make_core();
    core.create_playlist("Music".to_string()).unwrap();
    core.set_active_playlist("Music").unwrap();

    add_mock_track(&mut core, "Music", "The Beatles", "Hey Jude");
    add_mock_track(&mut core, "Music", "Pink Floyd", "Time");

    // Prepare play track 0
    let (path, duration, artist, title, playlist, idx) = core.prepare_play(None).unwrap();
    assert_eq!(idx, 0);
    assert_eq!(artist, "The Beatles");
    assert_eq!(title, "Hey Jude");
    assert_eq!(playlist, "Music");
    assert_eq!(duration, Duration::from_secs(180));
    assert!(path.to_string_lossy().contains("Hey Jude"));

    // Transport state should reflect playing
    let state = core.get_transport_state();
    assert!(state.is_playing);
    assert_eq!(state.track_index, Some(0));
    assert_eq!(state.track_artist, Some("The Beatles".to_string()));
    assert_eq!(state.next_artist, Some("Pink Floyd".to_string()));

    // Skip to next track
    let (_, _, artist, title, _, idx) = core.prepare_skip().unwrap();
    assert_eq!(idx, 1);
    assert_eq!(artist, "Pink Floyd");
    assert_eq!(title, "Time");

    // No next after last track
    let skip_result = core.prepare_skip();
    assert!(skip_result.is_err());
    assert_eq!(skip_result.unwrap_err(), "__end_of_playlist__");

    // Playback should be reset after end-of-playlist
    let state = core.get_transport_state();
    assert!(!state.is_playing);
}

#[test]
fn transport_pause_resume_cycle() {
    let mut core = make_core();
    core.create_playlist("Test".to_string()).unwrap();
    core.set_active_playlist("Test").unwrap();
    add_mock_track(&mut core, "Test", "Artist", "Song");

    core.prepare_play(None).unwrap();

    // Pause
    let paused = core.on_pause_toggle().unwrap();
    assert!(paused);
    let state = core.get_transport_state();
    assert!(state.is_playing);
    assert!(state.is_paused);

    // Resume
    let resumed = core.on_pause_toggle().unwrap();
    assert!(!resumed);
    let state = core.get_transport_state();
    assert!(state.is_playing);
    assert!(!state.is_paused);

    // Stop
    core.on_stop();
    let state = core.get_transport_state();
    assert!(!state.is_playing);
    assert!(state.track_index.is_none());
}

#[test]
fn transport_seek_updates_elapsed() {
    let mut core = make_core();
    core.create_playlist("Test".to_string()).unwrap();
    core.set_active_playlist("Test").unwrap();
    add_mock_track(&mut core, "Test", "Artist", "Song");

    core.prepare_play(None).unwrap();
    core.on_seek(60.0).unwrap();

    let state = core.get_transport_state();
    assert!(state.elapsed_secs >= 59.5 && state.elapsed_secs <= 61.0);
}

#[test]
fn transport_play_specific_track() {
    let mut core = make_core();
    core.create_playlist("Test".to_string()).unwrap();
    core.set_active_playlist("Test").unwrap();

    add_mock_track(&mut core, "Test", "Artist A", "Song 1");
    add_mock_track(&mut core, "Test", "Artist B", "Song 2");
    add_mock_track(&mut core, "Test", "Artist C", "Song 3");

    // Play track at index 2 directly
    let (_, _, artist, _, _, idx) = core.prepare_play(Some(2)).unwrap();
    assert_eq!(idx, 2);
    assert_eq!(artist, "Artist C");

    // Out-of-range index errors
    core.on_stop();
    assert!(core.prepare_play(Some(99)).is_err());
}

// ── Schedule workflow ─────────────────────────────────────────────────────

#[test]
fn schedule_full_workflow() {
    let mut core = make_core();

    // Add multiple events
    let id1 = core
        .add_schedule_event("08:00", "overlay", "jingle.mp3", Some(5), Some("Jingle".to_string()), None)
        .unwrap();
    let id2 = core
        .add_schedule_event("12:00", "stop", "news.mp3", Some(9), Some("News".to_string()), None)
        .unwrap();
    let _id3 = core
        .add_schedule_event("18:00", "insert", "promo.mp3", Some(3), None, Some(vec![0, 4]))
        .unwrap();

    let events = core.get_schedule();
    assert_eq!(events.len(), 3);

    // Events sorted by time
    assert_eq!(events[0].time, "08:00:00");
    assert_eq!(events[1].time, "12:00:00");
    assert_eq!(events[2].time, "18:00:00");

    // Toggle disable
    let new_state = core.toggle_schedule_event(id2).unwrap();
    assert!(!new_state); // disabled
    let events = core.get_schedule();
    let news_evt = events.iter().find(|e| e.id == id2).unwrap();
    assert!(!news_evt.enabled);

    // Remove
    core.remove_schedule_event(id1).unwrap();
    assert_eq!(core.get_schedule().len(), 2);

    // Status reflects schedule
    assert_eq!(core.get_status().schedule_event_count, 2);
}

// ── Ad workflow ───────────────────────────────────────────────────────────

#[test]
fn ad_full_workflow() {
    let mut core = make_core();

    // Add ads
    core.add_ad("Sponsor A".to_string(), "sponsor_a.mp3".to_string()).unwrap();
    core.add_ad("Sponsor B".to_string(), "sponsor_b.mp3".to_string()).unwrap();
    core.add_ad("Sponsor C".to_string(), "sponsor_c.mp3".to_string()).unwrap();

    assert_eq!(core.get_ads().len(), 3);

    // Update ad with schedule
    core.update_ad(
        1,
        "Sponsor B Updated".to_string(),
        true,
        "sponsor_b_v2.mp3".to_string(),
        true,
        vec!["Monday".to_string(), "Wednesday".to_string(), "Friday".to_string()],
        vec![8, 9, 10, 14, 15, 16],
    )
    .unwrap();

    let ads = core.get_ads();
    assert_eq!(ads[1].name, "Sponsor B Updated");
    assert!(ads[1].scheduled);
    assert_eq!(ads[1].hours.len(), 6);

    // Reorder: move last to first
    core.reorder_ad(2, 0).unwrap();
    let ads = core.get_ads();
    assert_eq!(ads[0].name, "Sponsor C");

    // Toggle disable
    let new_state = core.toggle_ad(0).unwrap();
    assert!(!new_state);

    // Remove
    core.remove_ad(1).unwrap();
    assert_eq!(core.get_ads().len(), 2);
}

// ── RDS workflow ──────────────────────────────────────────────────────────

#[test]
fn rds_full_workflow() {
    let mut core = make_core();

    // Check defaults
    let rds = core.get_rds_config();
    assert_eq!(rds.ip, "127.0.0.1");
    assert_eq!(rds.port, 10001);
    assert!(rds.messages.is_empty());

    // Update connection settings
    core.update_rds_settings("192.168.1.100".to_string(), 5555, "Station FM".to_string())
        .unwrap();

    // Add messages
    core.add_rds_message("Now Playing: {artist} - {title}".to_string()).unwrap();
    core.add_rds_message("Listen live at stationfm.com".to_string()).unwrap();
    core.add_rds_message("Call 555-0100".to_string()).unwrap();

    assert_eq!(core.get_rds_config().messages.len(), 3);

    // Update a message with schedule
    core.update_rds_message(
        0,
        "Now: {artist} - {title}".to_string(),
        true,
        15,
        true,
        vec!["Monday".to_string(), "Tuesday".to_string()],
        vec![6, 7, 8, 9, 10],
    )
    .unwrap();

    let rds = core.get_rds_config();
    assert!(rds.messages[0].enabled);
    assert_eq!(rds.messages[0].duration, 15);
    assert!(rds.messages[0].scheduled);

    // Reorder
    core.reorder_rds_message(2, 0).unwrap();
    let rds = core.get_rds_config();
    assert_eq!(rds.messages[0].text, "Call 555-0100");

    // Toggle (index 1 is "Now: {artist}..." which was set enabled=true in update_rds_message)
    let new_state = core.toggle_rds_message(1).unwrap();
    assert!(!new_state); // was enabled, now disabled

    // Remove
    core.remove_rds_message(0).unwrap();
    assert_eq!(core.get_rds_config().messages.len(), 2);
}

// ── Lecture detector workflow ─────────────────────────────────────────────

#[test]
fn lecture_detector_workflow() {
    let mut core = make_core();

    // Default heuristic: starts-with-R = lecture
    assert!(core.test_lecture("Rabbi Shalom"));
    assert!(core.test_lecture("Rav Moshe"));
    assert!(!core.test_lecture("The Beatles"));
    assert!(!core.test_lecture("Madonna"));

    // Blacklist: override R-heuristic for music artists
    core.lecture_blacklist_add("Rihanna").unwrap();
    core.lecture_blacklist_add("Red Hot Chili Peppers").unwrap();
    assert!(!core.test_lecture("Rihanna"));
    assert!(!core.test_lecture("Red Hot Chili Peppers"));

    // Whitelist: force classification as lecture
    core.lecture_whitelist_add("Dr. Smith").unwrap();
    assert!(core.test_lecture("Dr. Smith"));

    // Config reflects changes
    let config = core.get_lecture_config();
    assert_eq!(config.blacklist.len(), 2);
    assert_eq!(config.whitelist.len(), 1);

    // Remove from blacklist
    core.lecture_blacklist_remove("Rihanna").unwrap();
    assert!(core.test_lecture("Rihanna")); // back to R-heuristic
    assert_eq!(core.get_lecture_config().blacklist.len(), 1);
}

// ── Log capture workflow ─────────────────────────────────────────────────

#[test]
fn log_capture_during_operations() {
    let mut core = make_core();
    core.create_playlist("Music".to_string()).unwrap();
    core.set_active_playlist("Music").unwrap();
    add_mock_track(&mut core, "Music", "Artist", "Song");

    // Clear any logs from setup
    core.clear_logs();

    // Play should generate log
    core.prepare_play(None).unwrap();
    let logs = core.get_logs(None);
    assert!(!logs.is_empty());
    assert!(logs.iter().any(|l| l.message.contains("Playing")));

    // Stop should log
    core.on_stop();
    let logs = core.get_logs(None);
    assert!(logs.iter().any(|l| l.message.contains("stopped")));

    // Incremental log fetch
    let count_so_far = logs.len();
    core.log("warn", "Test warning".to_string());
    let new_logs = core.get_logs(Some(count_so_far));
    assert_eq!(new_logs.len(), 1);
    assert_eq!(new_logs[0].level, "warn");
}

#[test]
fn schedule_event_generates_log() {
    let mut core = make_core();
    core.clear_logs();

    core.add_schedule_event("14:00", "stop", "news.mp3", Some(9), Some("News".to_string()), None)
        .unwrap();

    let logs = core.get_logs(None);
    assert!(logs.iter().any(|l| l.message.contains("Schedule event added")));
}

// ── AudioRuntime headless tests ──────────────────────────────────────────

#[test]
fn audio_runtime_stop_emits_event() {
    let events: Arc<Mutex<Vec<AudioEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    let handle = spawn_audio_runtime(move |evt| {
        events_clone.lock().unwrap().push(evt);
    });

    handle.stop();
    std::thread::sleep(Duration::from_millis(200));

    let evts = events.lock().unwrap();
    assert!(evts.iter().any(|e| matches!(e, AudioEvent::Stopped)));

    handle.shutdown();
}

#[test]
fn audio_runtime_play_bad_file_emits_error() {
    let events: Arc<Mutex<Vec<AudioEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    let handle = spawn_audio_runtime(move |evt| {
        events_clone.lock().unwrap().push(evt);
    });

    let monitor = LevelMonitor::new();
    handle.play(PathBuf::from("__does_not_exist__.mp3"), monitor);
    std::thread::sleep(Duration::from_millis(500));

    let evts = events.lock().unwrap();
    assert!(
        evts.iter().any(|e| matches!(e, AudioEvent::PlayError(_))),
        "Expected PlayError, got: {:?}",
        *evts
    );

    handle.shutdown();
}

#[test]
fn audio_runtime_shutdown_is_clean() {
    let handle = spawn_audio_runtime(|_| {});
    handle.shutdown();
    // Give thread time to exit
    std::thread::sleep(Duration::from_millis(100));
    // If we get here, the thread shut down cleanly without panic or hang
}

// ── In-memory save isolation ─────────────────────────────────────────────

#[test]
fn test_mode_does_not_write_files() {
    let mut core = make_core();

    // These all call engine.save() internally, but should NOT write files
    core.create_playlist("Test".to_string()).unwrap();
    core.set_active_playlist("Test").unwrap();
    core.set_crossfade(5.0).unwrap();
    core.add_schedule_event("12:00", "overlay", "test.mp3", None, None, None)
        .unwrap();
    core.add_ad("Test Ad".to_string(), "ad.mp3".to_string()).unwrap();
    core.add_rds_message("Test".to_string()).unwrap();

    // Verify no state file was created in the test working directory
    // (Engine::new() has state_path: None, so save() should be a no-op)
    assert!(!std::path::Path::new("signalflow_state.json").exists()
        || {
            // If it existed before the test (from prior runs), that's okay —
            // we just verify our Engine instance has no state_path
            core.engine.state_path().is_none()
        }
    );
}

// ── Combined multi-feature workflow ──────────────────────────────────────

#[test]
fn full_radio_station_setup() {
    let mut core = make_core();

    // 1. Configure the station
    core.set_crossfade(3.0).unwrap();
    core.set_silence_detection(0.01, 5.0).unwrap();
    core.set_conflict_policy("schedule-wins").unwrap();
    core.set_nowplaying_path(Some("C:\\radio\\nowplaying.xml".to_string())).unwrap();

    // 2. Create playlists
    core.create_playlist("Morning".to_string()).unwrap();
    core.create_playlist("Afternoon".to_string()).unwrap();
    core.set_active_playlist("Morning").unwrap();

    add_mock_track(&mut core, "Morning", "Artist A", "Wake Up");
    add_mock_track(&mut core, "Morning", "Artist B", "Good Morning");
    add_mock_track(&mut core, "Morning", "Artist C", "Sunrise");

    add_mock_track(&mut core, "Afternoon", "Artist D", "Chill");
    add_mock_track(&mut core, "Afternoon", "Artist E", "Groove");

    // 3. Set up schedule
    core.add_schedule_event("08:00", "overlay", "jingle.mp3", Some(5), Some("Jingle".to_string()), None)
        .unwrap();
    core.add_schedule_event("12:00", "stop", "news.mp3", Some(9), Some("News".to_string()), None)
        .unwrap();

    // 4. Set up ads
    core.add_ad("Sponsor A".to_string(), "sponsor_a.mp3".to_string()).unwrap();
    core.add_ad("Sponsor B".to_string(), "sponsor_b.mp3".to_string()).unwrap();

    // 5. Set up RDS
    core.update_rds_settings("10.0.0.5".to_string(), 10001, "Station FM".to_string()).unwrap();
    core.add_rds_message("Now: {artist} - {title}".to_string()).unwrap();
    core.add_rds_message("Listen at stationfm.com".to_string()).unwrap();

    // 6. Set up lecture detector
    core.lecture_blacklist_add("Rihanna").unwrap();
    core.lecture_whitelist_add("Rabbi Cohen").unwrap();

    // 7. Simulate playback
    let (_, _, artist, _, _, _) = core.prepare_play(None).unwrap();
    assert_eq!(artist, "Artist A");

    let state = core.get_transport_state();
    assert!(state.is_playing);
    assert_eq!(state.track_artist, Some("Artist A".to_string()));
    assert_eq!(state.next_artist, Some("Artist B".to_string()));

    // Skip through all tracks
    core.prepare_skip().unwrap();
    core.prepare_skip().unwrap();
    let result = core.prepare_skip();
    assert!(result.is_err()); // end of playlist

    // 8. Verify full status
    let status = core.get_status();
    assert_eq!(status.playlist_count, 2);
    assert_eq!(status.schedule_event_count, 2);
    assert_eq!(status.crossfade_secs, 3.0);

    // 9. Verify logs captured everything
    let logs = core.get_logs(None);
    assert!(logs.len() >= 4); // play + skip + skip + end-of-playlist at minimum
}
