//! AppCore — Central command dispatcher for signalFlow.
//!
//! Unified interface for all engine operations. Both the Tauri GUI and tests
//! interact with the engine through AppCore methods. This eliminates duplicated
//! command logic between CLI and GUI, and provides a single point of validation.
//!
//! Audio playback (Player, AudioRuntime) is NOT owned by AppCore yet — that
//! will be added in Step 2 of the unified architecture migration.

use crate::ad_logger::{AdPlayLogger, AdStatistics};
use crate::ad_report::AdReportGenerator;
use crate::ad_scheduler::AdConfig;
use crate::auto_intro;
use crate::engine::Engine;
use crate::rds::{RdsMessage, RdsSchedule};
use crate::scheduler::{parse_time, ConflictPolicy, Priority, ScheduleMode};
use chrono::Local;
use serde::Serialize;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// ── Log buffer ──────────────────────────────────────────────────────────────

const LOG_BUFFER_MAX: usize = 500;

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

pub struct LogBuffer {
    entries: VecDeque<LogEntry>,
}

impl LogBuffer {
    pub fn new() -> Self {
        LogBuffer {
            entries: VecDeque::new(),
        }
    }

    pub fn push(&mut self, level: &str, message: String) {
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

    pub fn get(&self, since_index: usize) -> Vec<LogEntry> {
        self.entries.iter().skip(since_index).cloned().collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

// ── Playback state ──────────────────────────────────────────────────────────

pub struct PlaybackState {
    pub is_playing: bool,
    pub is_paused: bool,
    pub track_index: Option<usize>,
    pub playlist_name: Option<String>,
    pub track_duration: Duration,
    pub start_time: Option<Instant>,
    pub total_paused: Duration,
    pub pause_start: Option<Instant>,
}

impl PlaybackState {
    pub fn new() -> Self {
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

    pub fn elapsed(&self) -> Duration {
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

    pub fn reset(&mut self) {
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

// ── Response data types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct StatusData {
    pub playlist_count: usize,
    pub active_playlist: Option<String>,
    pub schedule_event_count: usize,
    pub crossfade_secs: f32,
    pub conflict_policy: String,
    pub silence_threshold: f32,
    pub silence_duration_secs: f32,
    pub intros_folder: Option<String>,
    pub recurring_intro_interval_secs: f32,
    pub recurring_intro_duck_volume: f32,
    pub now_playing_path: Option<String>,
    pub stream_output_enabled: bool,
    pub stream_output_url: String,
    pub recording_enabled: bool,
    pub recording_output_dir: Option<String>,
    pub indexed_locations: Vec<String>,
    pub favorite_folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaylistData {
    pub id: u32,
    pub name: String,
    pub track_count: usize,
    pub is_active: bool,
    pub current_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackData {
    pub index: usize,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub duration_secs: f64,
    pub duration_display: String,
    pub played_duration_secs: Option<f64>,
    pub start_time_display: Option<String>,
    pub has_intro: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigData {
    pub crossfade_secs: f32,
    pub silence_threshold: f32,
    pub silence_duration_secs: f32,
    pub intros_folder: Option<String>,
    pub recurring_intro_interval_secs: f32,
    pub recurring_intro_duck_volume: f32,
    pub conflict_policy: String,
    pub now_playing_path: Option<String>,
    pub stream_output_enabled: bool,
    pub stream_output_url: String,
    pub recording_enabled: bool,
    pub recording_output_dir: Option<String>,
    pub indexed_locations: Vec<String>,
    pub favorite_folders: Vec<String>,
    pub output_device_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransportData {
    pub is_playing: bool,
    pub is_paused: bool,
    pub elapsed_secs: f64,
    pub duration_secs: f64,
    pub track_index: Option<usize>,
    pub track_artist: Option<String>,
    pub track_title: Option<String>,
    pub next_artist: Option<String>,
    pub next_title: Option<String>,
    pub track_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScheduleEventData {
    pub id: u32,
    pub time: String,
    pub mode: String,
    pub file: String,
    pub priority: u8,
    pub enabled: bool,
    pub label: Option<String>,
    pub days: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdData {
    pub index: usize,
    pub name: String,
    pub enabled: bool,
    pub mp3_file: String,
    pub scheduled: bool,
    pub days: Vec<String>,
    pub hours: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RdsConfigData {
    pub ip: String,
    pub port: u16,
    pub default_message: String,
    pub messages: Vec<RdsMessageData>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RdsMessageData {
    pub index: usize,
    pub text: String,
    pub enabled: bool,
    pub duration: u32,
    pub scheduled: bool,
    pub days: Vec<String>,
    pub hours: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LectureConfigData {
    pub blacklist: Vec<String>,
    pub whitelist: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileBrowserEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileSearchResult {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaylistProfileData {
    pub name: String,
    pub playlist_names: Vec<String>,
    pub playlist_paths: Vec<Option<String>>,
}

// ── AppCore ─────────────────────────────────────────────────────────────────

pub struct AppCore {
    pub engine: Engine,
    pub playback: PlaybackState,
    pub logs: LogBuffer,
}

impl AppCore {
    /// Create a new AppCore by loading state from a file path.
    pub fn new(state_path: &Path) -> Self {
        AppCore {
            engine: Engine::load_from(state_path),
            playback: PlaybackState::new(),
            logs: LogBuffer::new(),
        }
    }

    /// Create a new AppCore with a fresh (empty) engine. For testing.
    pub fn new_test() -> Self {
        AppCore {
            engine: Engine::new(),
            playback: PlaybackState::new(),
            logs: LogBuffer::new(),
        }
    }

    // ── Status & Config (read-only) ─────────────────────────────────────

    pub fn get_status(&self) -> StatusData {
        StatusData {
            playlist_count: self.engine.playlists.len(),
            active_playlist: self.engine.active_playlist().map(|p| p.name.clone()),
            schedule_event_count: self.engine.schedule.events.len(),
            crossfade_secs: self.engine.crossfade_secs,
            conflict_policy: self.engine.conflict_policy.to_string(),
            silence_threshold: self.engine.silence_threshold,
            silence_duration_secs: self.engine.silence_duration_secs,
            intros_folder: self.engine.intros_folder.clone(),
            recurring_intro_interval_secs: self.engine.recurring_intro_interval_secs,
            recurring_intro_duck_volume: self.engine.recurring_intro_duck_volume,
            now_playing_path: self.engine.now_playing_path.clone(),
            stream_output_enabled: self.engine.stream_output.enabled,
            stream_output_url: self.engine.stream_output.endpoint_url.clone(),
            recording_enabled: self.engine.recording.enabled,
            recording_output_dir: self.engine.recording.output_dir.clone(),
            indexed_locations: self.engine.indexed_locations.clone(),
            favorite_folders: self.engine.favorite_folders.clone(),
        }
    }

    pub fn get_config(&self) -> ConfigData {
        ConfigData {
            crossfade_secs: self.engine.crossfade_secs,
            silence_threshold: self.engine.silence_threshold,
            silence_duration_secs: self.engine.silence_duration_secs,
            intros_folder: self.engine.intros_folder.clone(),
            recurring_intro_interval_secs: self.engine.recurring_intro_interval_secs,
            recurring_intro_duck_volume: self.engine.recurring_intro_duck_volume,
            conflict_policy: self.engine.conflict_policy.to_string(),
            now_playing_path: self.engine.now_playing_path.clone(),
            stream_output_enabled: self.engine.stream_output.enabled,
            stream_output_url: self.engine.stream_output.endpoint_url.clone(),
            recording_enabled: self.engine.recording.enabled,
            recording_output_dir: self.engine.recording.output_dir.clone(),
            indexed_locations: self.engine.indexed_locations.clone(),
            favorite_folders: self.engine.favorite_folders.clone(),
            output_device_name: self.engine.output_device_name.clone(),
        }
    }

    // ── Audio Device ────────────────────────────────────────────────────

    pub fn list_output_devices(&self) -> Vec<String> {
        crate::player::list_output_devices()
    }

    pub fn set_output_device(&mut self, name: Option<String>) -> Result<(), String> {
        self.engine.output_device_name = name;
        self.engine.save()
    }

    // ── Playlist CRUD ───────────────────────────────────────────────────

    pub fn get_playlists(&self) -> Vec<PlaylistData> {
        self.engine
            .playlists
            .iter()
            .map(|p| PlaylistData {
                id: p.id,
                name: p.name.clone(),
                track_count: p.track_count(),
                is_active: self.engine.active_playlist_id == Some(p.id),
                current_index: p.current_index,
            })
            .collect()
    }

    pub fn create_playlist(&mut self, name: String) -> Result<u32, String> {
        if self.engine.find_playlist(&name).is_some() {
            return Err(format!("Playlist '{}' already exists", name));
        }
        let id = self.engine.create_playlist(name);
        self.engine.save()?;
        Ok(id)
    }

    pub fn delete_playlist(&mut self, name: &str) -> Result<(), String> {
        let pos = self
            .engine
            .playlists
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("Playlist '{}' not found", name))?;
        let removed_id = self.engine.playlists[pos].id;
        self.engine.playlists.remove(pos);
        if self.engine.active_playlist_id == Some(removed_id) {
            self.engine.active_playlist_id = None;
        }
        self.engine.save()?;
        Ok(())
    }

    pub fn rename_playlist(&mut self, old_name: &str, new_name: String) -> Result<(), String> {
        if self.engine.find_playlist(&new_name).is_some() {
            return Err(format!("Playlist '{}' already exists", new_name));
        }
        let pl = self
            .engine
            .find_playlist_mut(old_name)
            .ok_or_else(|| format!("Playlist '{}' not found", old_name))?;
        pl.name = new_name;
        self.engine.save()?;
        Ok(())
    }

    pub fn set_active_playlist(&mut self, name: &str) -> Result<u32, String> {
        let id = self.engine.set_active(name)?;
        self.engine.save()?;
        Ok(id)
    }

    // ── Track operations ────────────────────────────────────────────────

    pub fn get_playlist_tracks(&self, name: &str) -> Result<Vec<TrackData>, String> {
        let intros_folder = self.engine.intros_folder.as_ref().map(Path::new);
        let pl = self
            .engine
            .find_playlist(name)
            .ok_or_else(|| format!("Playlist '{}' not found", name))?;
        Ok(pl
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let has_intro = intros_folder
                    .map(|folder| auto_intro::has_intro(folder, &t.artist))
                    .unwrap_or(false);
                TrackData {
                    index: i,
                    path: t.path.to_string_lossy().to_string(),
                    title: t.title.clone(),
                    artist: t.artist.clone(),
                    duration_secs: t.duration.as_secs_f64(),
                    duration_display: t.duration_display(),
                    played_duration_secs: t.played_duration.map(|d| d.as_secs_f64()),
                    start_time_display: t.played_duration_display(),
                    has_intro,
                }
            })
            .collect())
    }

    pub fn add_track(&mut self, playlist: &str, path: &str) -> Result<usize, String> {
        let intros_folder = self.engine.intros_folder.clone();
        let pl = self
            .engine
            .find_playlist_mut(playlist)
            .ok_or_else(|| format!("Playlist '{}' not found", playlist))?;
        let idx = pl.add_track(Path::new(path))?;
        if let Some(ref folder) = intros_folder {
            pl.tracks[idx].has_intro =
                auto_intro::has_intro(Path::new(folder), &pl.tracks[idx].artist);
        }
        self.engine.save()?;
        Ok(idx)
    }

    pub fn add_tracks(&mut self, playlist: &str, paths: &[String]) -> Result<usize, String> {
        let intros_folder = self.engine.intros_folder.clone();
        let pl = self
            .engine
            .find_playlist_mut(playlist)
            .ok_or_else(|| format!("Playlist '{}' not found", playlist))?;
        let mut count = 0;
        for path in paths {
            match pl.add_track(Path::new(path)) {
                Ok(idx) => {
                    if let Some(ref folder) = intros_folder {
                        pl.tracks[idx].has_intro =
                            auto_intro::has_intro(Path::new(folder), &pl.tracks[idx].artist);
                    }
                    count += 1;
                }
                Err(e) => eprintln!("Failed to add '{}': {}", path, e),
            }
        }
        self.engine.save()?;
        Ok(count)
    }

    pub fn remove_tracks(&mut self, playlist: &str, indices: &[usize]) -> Result<(), String> {
        let pl = self
            .engine
            .find_playlist_mut(playlist)
            .ok_or_else(|| format!("Playlist '{}' not found", playlist))?;
        let mut sorted = indices.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        for &idx in sorted.iter().rev() {
            pl.remove_track(idx)?;
        }
        self.engine.save()?;
        Ok(())
    }

    pub fn reorder_track(&mut self, playlist: &str, from: usize, to: usize) -> Result<(), String> {
        let pl = self
            .engine
            .find_playlist_mut(playlist)
            .ok_or_else(|| format!("Playlist '{}' not found", playlist))?;
        pl.reorder(from, to)?;
        self.engine.save()?;
        Ok(())
    }

    pub fn edit_track_metadata(
        &mut self,
        playlist: &str,
        track_index: usize,
        artist: Option<&str>,
        title: Option<&str>,
    ) -> Result<(), String> {
        self.engine
            .edit_track_metadata(playlist, track_index, artist, title)?;
        self.engine.save()?;
        Ok(())
    }

    pub fn copy_tracks(
        &self,
        from_playlist: &str,
        indices: &[usize],
    ) -> Result<Vec<crate::track::Track>, String> {
        self.engine.copy_tracks(from_playlist, indices)
    }

    pub fn paste_tracks(
        &mut self,
        to_playlist: &str,
        tracks: Vec<crate::track::Track>,
        at: Option<usize>,
    ) -> Result<(), String> {
        self.engine.paste_tracks(to_playlist, tracks, at)?;
        self.engine.save()?;
        Ok(())
    }

    // ── Transport state (read-only snapshot) ────────────────────────────
    //
    // Note: actual play/stop/pause/skip/seek require the Player, which is
    // NOT owned by AppCore yet. These methods provide the state snapshot
    // that Tauri's transport_status handler needs.

    pub fn get_transport_state(&self) -> TransportData {
        let elapsed = self.playback.elapsed();
        let (artist, title, next_artist, next_title, track_path) =
            if let (Some(idx), Some(pl_name)) =
                (self.playback.track_index, &self.playback.playlist_name)
            {
                if let Some(pl) = self.engine.find_playlist(pl_name) {
                    let current = if let Some(track) = pl.tracks.get(idx) {
                        (
                            Some(track.artist.clone()),
                            Some(track.title.clone()),
                            Some(track.path.to_string_lossy().to_string()),
                        )
                    } else {
                        (None, None, None)
                    };
                    let next = if let Some(next_track) = pl.tracks.get(idx + 1) {
                        (
                            Some(next_track.artist.clone()),
                            Some(next_track.title.clone()),
                        )
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

        TransportData {
            is_playing: self.playback.is_playing,
            is_paused: self.playback.is_paused,
            elapsed_secs: elapsed.as_secs_f64(),
            duration_secs: self.playback.track_duration.as_secs_f64(),
            track_index: self.playback.track_index,
            track_artist: artist,
            track_title: title,
            next_artist,
            next_title,
            track_path,
        }
    }

    /// Prepare transport state for playing a track. Returns the track path,
    /// duration, artist, title, playlist name, and resolved index.
    /// Does NOT actually play audio — the caller must handle Player interaction.
    pub fn prepare_play(
        &mut self,
        track_index: Option<usize>,
    ) -> Result<(PathBuf, Duration, String, String, String, usize), String> {
        let pl = self
            .engine
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
        self.engine.save().ok();

        // Update playback state
        self.playback.is_playing = true;
        self.playback.is_paused = false;
        self.playback.track_index = Some(idx);
        self.playback.playlist_name = Some(playlist_name.clone());
        self.playback.track_duration = track_duration;
        self.playback.start_time = Some(Instant::now());
        self.playback.total_paused = Duration::ZERO;
        self.playback.pause_start = None;

        self.logs.push(
            "info",
            format!("Playing: {} — {}", track_artist, track_title),
        );

        Ok((
            track_path,
            track_duration,
            track_artist,
            track_title,
            playlist_name,
            idx,
        ))
    }

    /// Update playback state after stopping.
    pub fn on_stop(&mut self) {
        self.playback.reset();
        self.logs.push("info", "Playback stopped".to_string());
    }

    /// Toggle pause state. Returns true if now paused, false if resumed.
    pub fn on_pause_toggle(&mut self) -> Result<bool, String> {
        if !self.playback.is_playing {
            return Err("Nothing is playing".to_string());
        }
        if self.playback.is_paused {
            self.playback.is_paused = false;
            if let Some(ps) = self.playback.pause_start.take() {
                self.playback.total_paused += ps.elapsed();
            }
            self.logs.push("info", "Playback resumed".to_string());
            Ok(false)
        } else {
            self.playback.is_paused = true;
            self.playback.pause_start = Some(Instant::now());
            self.logs.push("info", "Playback paused".to_string());
            Ok(true)
        }
    }

    /// Prepare for skip: advance to next track in active playlist.
    /// Returns the same tuple as prepare_play for the next track.
    /// Returns Err("__end_of_playlist__") if there is no next track.
    pub fn prepare_skip(
        &mut self,
    ) -> Result<(PathBuf, Duration, String, String, String, usize), String> {
        let pl = self
            .engine
            .active_playlist_mut()
            .ok_or_else(|| "No active playlist".to_string())?;

        let current = pl.current_index.unwrap_or(0);
        let next_idx = current + 1;
        if next_idx >= pl.tracks.len() {
            pl.current_index = None;
            self.engine.save().ok();
            self.playback.reset();
            self.logs
                .push("info", "Reached end of playlist".to_string());
            return Err("__end_of_playlist__".to_string());
        }

        let track_path = pl.tracks[next_idx].path.clone();
        let track_duration = pl.tracks[next_idx].duration;
        let track_artist = pl.tracks[next_idx].artist.clone();
        let track_title = pl.tracks[next_idx].title.clone();
        let playlist_name = pl.name.clone();
        pl.current_index = Some(next_idx);
        self.engine.save().ok();

        // Update playback state
        self.playback.is_playing = true;
        self.playback.is_paused = false;
        self.playback.track_index = Some(next_idx);
        self.playback.playlist_name = Some(playlist_name.clone());
        self.playback.track_duration = track_duration;
        self.playback.start_time = Some(Instant::now());
        self.playback.total_paused = Duration::ZERO;
        self.playback.pause_start = None;

        self.logs.push(
            "info",
            format!("Skipped to: {} — {}", track_artist, track_title),
        );

        Ok((
            track_path,
            track_duration,
            track_artist,
            track_title,
            playlist_name,
            next_idx,
        ))
    }

    /// Update timing after a seek operation.
    pub fn on_seek(&mut self, position_secs: f64) -> Result<(), String> {
        if !self.playback.is_playing {
            return Err("Nothing is playing".to_string());
        }
        let seek_pos = Duration::from_secs_f64(position_secs.max(0.0));
        let is_paused = self.playback.is_paused;
        self.playback.start_time = Some(Instant::now() - seek_pos);
        self.playback.total_paused = Duration::ZERO;
        self.playback.pause_start = if is_paused {
            Some(Instant::now())
        } else {
            None
        };
        Ok(())
    }

    // ── Waveform ────────────────────────────────────────────────────────

    pub fn get_waveform(path: &str) -> Result<Vec<f32>, String> {
        crate::waveform::generate_peaks_cached(Path::new(path))
    }

    // ── Schedule ────────────────────────────────────────────────────────

    pub fn get_schedule(&self) -> Vec<ScheduleEventData> {
        self.engine
            .schedule
            .events_by_time()
            .into_iter()
            .map(|e| ScheduleEventData {
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

    pub fn add_schedule_event(
        &mut self,
        time: &str,
        mode: &str,
        file: &str,
        priority: Option<u8>,
        label: Option<String>,
        days: Option<Vec<u8>>,
    ) -> Result<u32, String> {
        let parsed_time = parse_time(time)?;
        let parsed_mode = ScheduleMode::from_str_loose(mode)?;
        let pri = Priority(priority.unwrap_or(5));
        let days_vec = days.unwrap_or_default();

        let id = self.engine.schedule.add_event(
            parsed_time,
            parsed_mode,
            PathBuf::from(file),
            pri,
            label.clone(),
            days_vec,
        );
        self.engine.save()?;
        let display = label.unwrap_or_else(|| file.to_string());
        self.logs.push(
            "info",
            format!("Schedule event added: {} at {}", display, time),
        );
        Ok(id)
    }

    pub fn remove_schedule_event(&mut self, id: u32) -> Result<(), String> {
        self.engine.schedule.remove_event(id)?;
        self.engine.save()?;
        Ok(())
    }

    pub fn toggle_schedule_event(&mut self, id: u32) -> Result<bool, String> {
        let new_state = self.engine.schedule.toggle_event(id)?;
        self.engine.save()?;
        Ok(new_state)
    }

    // ── Config setters ──────────────────────────────────────────────────

    pub fn set_crossfade(&mut self, secs: f32) -> Result<(), String> {
        self.engine.crossfade_secs = secs;
        self.engine.save()?;
        Ok(())
    }

    pub fn set_silence_detection(
        &mut self,
        threshold: f32,
        duration_secs: f32,
    ) -> Result<(), String> {
        self.engine.silence_threshold = threshold;
        self.engine.silence_duration_secs = duration_secs;
        self.engine.save()?;
        Ok(())
    }

    pub fn set_intros_folder(&mut self, path: Option<String>) -> Result<(), String> {
        if let Some(ref p) = path {
            if !Path::new(p).is_dir() {
                return Err(format!("'{}' is not a valid directory", p));
            }
        }
        self.engine.intros_folder = path.clone();
        // Refresh has_intro flags on all tracks
        for pl in &mut self.engine.playlists {
            for track in &mut pl.tracks {
                track.has_intro = match &path {
                    Some(folder) => auto_intro::has_intro(Path::new(folder), &track.artist),
                    None => false,
                };
            }
        }
        self.engine.save()?;
        Ok(())
    }

    pub fn set_recurring_intro(
        &mut self,
        interval_secs: f32,
        duck_volume: f32,
    ) -> Result<(), String> {
        self.engine.recurring_intro_interval_secs = interval_secs;
        self.engine.recurring_intro_duck_volume = duck_volume;
        self.engine.save()?;
        Ok(())
    }

    pub fn set_conflict_policy(&mut self, policy: &str) -> Result<(), String> {
        let parsed = ConflictPolicy::from_str_loose(policy)?;
        self.engine.conflict_policy = parsed;
        self.engine.save()?;
        Ok(())
    }

    pub fn set_stream_output(&mut self, enabled: bool, endpoint_url: String) -> Result<(), String> {
        if enabled && endpoint_url.trim().is_empty() {
            return Err("Streaming endpoint URL is required when streaming is enabled".to_string());
        }
        self.engine.stream_output.enabled = enabled;
        self.engine.stream_output.endpoint_url = endpoint_url.trim().to_string();
        self.engine.save()
    }

    pub fn set_recording(
        &mut self,
        enabled: bool,
        output_dir: Option<String>,
    ) -> Result<(), String> {
        if enabled && output_dir.as_deref().unwrap_or("").trim().is_empty() {
            return Err(
                "Recording output directory is required when recording is enabled".to_string(),
            );
        }
        self.engine.recording.enabled = enabled;
        self.engine.recording.output_dir = output_dir.and_then(|p| {
            let trimmed = p.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
        self.engine.save()
    }

    pub fn set_indexed_locations(&mut self, locations: Vec<String>) -> Result<(), String> {
        self.engine.indexed_locations = locations
            .into_iter()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .collect();
        self.engine.indexed_locations.sort();
        self.engine.indexed_locations.dedup();
        self.engine.save()
    }

    pub fn set_favorite_folders(&mut self, folders: Vec<String>) -> Result<(), String> {
        self.engine.favorite_folders = folders
            .into_iter()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .collect();
        self.engine.favorite_folders.sort();
        self.engine.favorite_folders.dedup();
        self.engine.save()
    }

    pub fn set_nowplaying_path(&mut self, path: Option<String>) -> Result<(), String> {
        self.engine.now_playing_path = path;
        self.engine.save()?;
        Ok(())
    }

    pub fn list_directory(&self, path: Option<String>) -> Result<Vec<FileBrowserEntry>, String> {
        let target = path
            .map(PathBuf::from)
            .or_else(|| self.engine.indexed_locations.first().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."));

        let mut entries = Vec::new();
        let dir_entries = fs::read_dir(&target)
            .map_err(|e| format!("Failed to read directory '{}': {}", target.display(), e))?;

        for entry in dir_entries.flatten() {
            // Use target.join(file_name) instead of entry.path() to preserve
            // the user-provided drive letter on Windows mapped drives.
            // entry.path() resolves to \\?\UNC\... on mapped drives.
            let path = target.join(entry.file_name());
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let is_dir = file_type.is_dir();
            if !is_dir && !is_audio_file(&path) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            entries.push(FileBrowserEntry {
                path: path.to_string_lossy().to_string(),
                name,
                is_dir,
            });
        }

        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(entries)
    }

    pub fn search_indexed_files(&self, query: &str) -> Result<Vec<FileSearchResult>, String> {
        let trimmed = query.trim().to_lowercase();
        if trimmed.len() < 2 {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for root in &self.engine.indexed_locations {
            let root_path = Path::new(root);
            if !root_path.exists() {
                continue;
            }
            collect_matches(root_path, &trimmed, &mut results, 0);
            if results.len() >= 120 {
                break;
            }
        }

        Ok(results)
    }

    pub fn get_playlist_profiles(&self) -> Vec<PlaylistProfileData> {
        self.engine
            .playlist_profiles
            .iter()
            .map(|p| PlaylistProfileData {
                name: p.name.clone(),
                playlist_names: p.playlist_names.clone(),
                playlist_paths: p.playlist_paths.clone(),
            })
            .collect()
    }

    pub fn save_playlist_profile(&mut self, name: &str) -> Result<(), String> {
        let profile_name = name.trim();
        if profile_name.is_empty() {
            return Err("Profile name is required".to_string());
        }

        let playlist_names = self
            .engine
            .playlists
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();
        let playlist_paths = self
            .engine
            .playlists
            .iter()
            .map(|p| p.source_path.clone())
            .collect::<Vec<_>>();

        if let Some(existing) = self
            .engine
            .playlist_profiles
            .iter_mut()
            .find(|p| p.name.eq_ignore_ascii_case(profile_name))
        {
            existing.name = profile_name.to_string();
            existing.playlist_names = playlist_names;
            existing.playlist_paths = playlist_paths;
        } else {
            self.engine
                .playlist_profiles
                .push(crate::engine::PlaylistProfile {
                    name: profile_name.to_string(),
                    playlist_names,
                    playlist_paths,
                });
            self.engine
                .playlist_profiles
                .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }

        self.engine.save()
    }

    pub fn load_playlist_profile(&mut self, name: &str) -> Result<(), String> {
        let profile = self
            .engine
            .playlist_profiles
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .cloned()
            .ok_or_else(|| format!("Profile '{}' not found", name))?;

        self.engine.playlists.clear();
        self.engine.active_playlist_id = None;

        for (idx, playlist_name) in profile.playlist_names.into_iter().enumerate() {
            let id = self.engine.create_playlist(playlist_name);
            let source_path = profile.playlist_paths.get(idx).cloned().flatten();
            if let Some(playlist) = self.engine.playlists.iter_mut().find(|p| p.id == id) {
                playlist.source_path = source_path;
            }
        }

        if let Some(first_name) = self.engine.playlists.first().map(|p| p.name.clone()) {
            self.engine.set_active(&first_name)?;
        }

        self.playback.reset();
        self.engine.save()
    }

    pub fn import_m3u_playlist(&mut self, file_path: &str) -> Result<String, String> {
        let path = Path::new(file_path);
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read playlist '{}': {}", path.display(), e))?;

        let mut sources = Vec::new();
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let track_path = if Path::new(line).is_absolute() {
                PathBuf::from(line)
            } else {
                base_dir.join(line)
            };
            sources.push(track_path);
        }

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported")
            .trim();
        let base_name = if stem.is_empty() { "Imported" } else { stem };
        let mut name = base_name.to_string();
        let mut suffix = 2usize;
        while self.engine.find_playlist(&name).is_some() {
            name = format!("{} ({})", base_name, suffix);
            suffix += 1;
        }

        let id = self.engine.create_playlist(name.clone());
        {
            let playlist = self
                .engine
                .playlists
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or_else(|| "Failed to create imported playlist".to_string())?;
            playlist.source_path = Some(path.to_string_lossy().to_string());
            for src in sources {
                if let Ok(track) = crate::track::Track::from_path(&src) {
                    playlist.tracks.push(track);
                }
            }
        }
        self.engine.set_active(&name)?;
        self.engine.save()?;
        Ok(name)
    }

    pub fn export_playlist_to_m3u(
        &mut self,
        playlist_name: &str,
        file_path: Option<&str>,
    ) -> Result<String, String> {
        let target_path = if let Some(path) = file_path {
            PathBuf::from(path)
        } else {
            let pl = self
                .engine
                .find_playlist(playlist_name)
                .ok_or_else(|| format!("Playlist '{}' not found", playlist_name))?;
            let existing = pl
                .source_path
                .clone()
                .ok_or_else(|| "Playlist has no source file; choose Save As".to_string())?;
            PathBuf::from(existing)
        };

        let mut lines = vec!["#EXTM3U".to_string()];
        let tracks = self
            .engine
            .find_playlist(playlist_name)
            .ok_or_else(|| format!("Playlist '{}' not found", playlist_name))?
            .tracks
            .iter()
            .map(|t| t.path.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        lines.extend(tracks);
        fs::write(&target_path, lines.join("\n") + "\n").map_err(|e| {
            format!(
                "Failed to write playlist '{}': {}",
                target_path.display(),
                e
            )
        })?;

        if let Some(pl) = self.engine.find_playlist_mut(playlist_name) {
            pl.source_path = Some(target_path.to_string_lossy().to_string());
        }
        self.engine.save()?;
        Ok(target_path.to_string_lossy().to_string())
    }

    pub fn delete_playlist_profile(&mut self, name: &str) -> Result<(), String> {
        let profile_index = self
            .engine
            .playlist_profiles
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("Profile '{}' not found", name))?;
        self.engine.playlist_profiles.remove(profile_index);
        self.engine.save()
    }

    // ── Ads ─────────────────────────────────────────────────────────────

    pub fn get_ads(&self) -> Vec<AdData> {
        self.engine
            .ads
            .iter()
            .enumerate()
            .map(|(i, ad)| AdData {
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

    pub fn add_ad(&mut self, name: String, mp3_file: String) -> Result<usize, String> {
        let ad = AdConfig::new(name, PathBuf::from(mp3_file));
        let idx = self.engine.add_ad(ad);
        self.engine.save()?;
        Ok(idx)
    }

    pub fn remove_ad(&mut self, index: usize) -> Result<(), String> {
        self.engine.remove_ad(index)?;
        self.engine.save()?;
        Ok(())
    }

    pub fn toggle_ad(&mut self, index: usize) -> Result<bool, String> {
        let new_state = self.engine.toggle_ad(index)?;
        self.engine.save()?;
        Ok(new_state)
    }

    pub fn update_ad(
        &mut self,
        index: usize,
        name: String,
        enabled: bool,
        mp3_file: String,
        scheduled: bool,
        days: Vec<String>,
        hours: Vec<u8>,
    ) -> Result<(), String> {
        let len = self.engine.ads.len();
        let ad = self
            .engine
            .ads
            .get_mut(index)
            .ok_or_else(|| format!("Ad index {} out of range ({} ads)", index, len))?;
        ad.name = name;
        ad.enabled = enabled;
        ad.mp3_file = PathBuf::from(mp3_file);
        ad.scheduled = scheduled;
        ad.days = days;
        ad.hours = hours;
        self.engine.save()?;
        Ok(())
    }

    pub fn reorder_ad(&mut self, from: usize, to: usize) -> Result<(), String> {
        let len = self.engine.ads.len();
        if from >= len || to >= len {
            return Err(format!("Ad index out of range ({} ads)", len));
        }
        let ad = self.engine.ads.remove(from);
        self.engine.ads.insert(to, ad);
        self.engine.save()?;
        Ok(())
    }

    // ── Ad Statistics & Reports ──────────────────────────────────────────

    pub fn get_ad_stats(&self, start: Option<&str>, end: Option<&str>) -> AdStatistics {
        let logger = AdPlayLogger::new(Path::new("."));
        match (start, end) {
            (Some(s), Some(e)) => logger.get_ad_statistics_filtered(s, e),
            _ => logger.get_ad_statistics(),
        }
    }

    pub fn get_ad_daily_counts(&self, ad_name: &str) -> Vec<(String, usize)> {
        let logger = AdPlayLogger::new(Path::new("."));
        let counts = logger.get_daily_play_counts(ad_name);
        let mut entries: Vec<(String, usize)> = counts.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }

    pub fn get_ad_failures(&self) -> Vec<crate::ad_logger::AdFailure> {
        let logger = AdPlayLogger::new(Path::new("."));
        logger.get_failures()
    }

    pub fn generate_ad_report(
        &self,
        start: &str,
        end: &str,
        output_dir: &str,
        ad_name: Option<&str>,
        company_name: Option<&str>,
    ) -> Result<Vec<String>, String> {
        let logger = AdPlayLogger::new(Path::new("."));
        let reporter = AdReportGenerator::new(&logger);
        let out_path = Path::new(output_dir);

        if !out_path.is_dir() {
            return Err(format!("'{}' is not a valid directory", output_dir));
        }

        match ad_name {
            Some(name) => {
                match reporter.generate_single_report(name, start, end, company_name, out_path) {
                    Some(r) => Ok(vec![
                        r.csv_path.to_string_lossy().to_string(),
                        r.pdf_path.to_string_lossy().to_string(),
                    ]),
                    None => Ok(vec![]),
                }
            }
            None => {
                let results = reporter.generate_report(start, end, company_name, out_path);
                Ok(results
                    .into_iter()
                    .flat_map(|r| {
                        vec![
                            r.csv_path.to_string_lossy().to_string(),
                            r.pdf_path.to_string_lossy().to_string(),
                        ]
                    })
                    .collect())
            }
        }
    }

    // ── RDS ─────────────────────────────────────────────────────────────

    pub fn get_rds_config(&self) -> RdsConfigData {
        RdsConfigData {
            ip: self.engine.rds.ip.clone(),
            port: self.engine.rds.port,
            default_message: self.engine.rds.default_message.clone(),
            messages: self
                .engine
                .rds
                .messages
                .iter()
                .enumerate()
                .map(|(i, m)| RdsMessageData {
                    index: i,
                    text: m.text.clone(),
                    enabled: m.enabled,
                    duration: m.duration,
                    scheduled: m.scheduled.enabled,
                    days: m.scheduled.days.clone(),
                    hours: m.scheduled.hours.clone(),
                })
                .collect(),
        }
    }

    pub fn add_rds_message(&mut self, text: String) -> Result<usize, String> {
        let msg = RdsMessage::new(&text);
        self.engine.rds.messages.push(msg);
        let idx = self.engine.rds.messages.len() - 1;
        self.engine.save()?;
        Ok(idx)
    }

    pub fn remove_rds_message(&mut self, index: usize) -> Result<(), String> {
        let len = self.engine.rds.messages.len();
        if index >= len {
            return Err(format!(
                "RDS message index {} out of range ({} messages)",
                index, len
            ));
        }
        self.engine.rds.messages.remove(index);
        self.engine.save()?;
        Ok(())
    }

    pub fn toggle_rds_message(&mut self, index: usize) -> Result<bool, String> {
        let len = self.engine.rds.messages.len();
        let msg = self.engine.rds.messages.get_mut(index).ok_or_else(|| {
            format!(
                "RDS message index {} out of range ({} messages)",
                index, len
            )
        })?;
        msg.enabled = !msg.enabled;
        let new_state = msg.enabled;
        self.engine.save()?;
        Ok(new_state)
    }

    pub fn update_rds_message(
        &mut self,
        index: usize,
        text: String,
        enabled: bool,
        duration: u32,
        scheduled: bool,
        days: Vec<String>,
        hours: Vec<u8>,
    ) -> Result<(), String> {
        let len = self.engine.rds.messages.len();
        let msg = self.engine.rds.messages.get_mut(index).ok_or_else(|| {
            format!(
                "RDS message index {} out of range ({} messages)",
                index, len
            )
        })?;
        msg.text = text;
        msg.enabled = enabled;
        msg.duration = duration.clamp(1, 60);
        msg.scheduled = RdsSchedule {
            enabled: scheduled,
            days,
            hours,
        };
        self.engine.save()?;
        Ok(())
    }

    pub fn reorder_rds_message(&mut self, from: usize, to: usize) -> Result<(), String> {
        let len = self.engine.rds.messages.len();
        if from >= len || to >= len {
            return Err(format!("RDS message index out of range ({} messages)", len));
        }
        let msg = self.engine.rds.messages.remove(from);
        self.engine.rds.messages.insert(to, msg);
        self.engine.save()?;
        Ok(())
    }

    pub fn update_rds_settings(
        &mut self,
        ip: String,
        port: u16,
        default_message: String,
    ) -> Result<(), String> {
        self.engine.rds.ip = ip;
        self.engine.rds.port = port;
        self.engine.rds.default_message = default_message;
        self.engine.save()?;
        Ok(())
    }

    // ── Lecture Detector ────────────────────────────────────────────────

    pub fn get_lecture_config(&self) -> LectureConfigData {
        let mut blacklist: Vec<String> = self
            .engine
            .lecture_detector
            .blacklist
            .iter()
            .cloned()
            .collect();
        let mut whitelist: Vec<String> = self
            .engine
            .lecture_detector
            .whitelist
            .iter()
            .cloned()
            .collect();
        blacklist.sort();
        whitelist.sort();
        LectureConfigData {
            blacklist,
            whitelist,
        }
    }

    pub fn lecture_blacklist_add(&mut self, keyword: &str) -> Result<(), String> {
        self.engine.lecture_detector.add_blacklist(keyword);
        self.engine.save()?;
        Ok(())
    }

    pub fn lecture_blacklist_remove(&mut self, keyword: &str) -> Result<bool, String> {
        let removed = self.engine.lecture_detector.remove_blacklist(keyword);
        self.engine.save()?;
        Ok(removed)
    }

    pub fn lecture_whitelist_add(&mut self, keyword: &str) -> Result<(), String> {
        self.engine.lecture_detector.add_whitelist(keyword);
        self.engine.save()?;
        Ok(())
    }

    pub fn lecture_whitelist_remove(&mut self, keyword: &str) -> Result<bool, String> {
        let removed = self.engine.lecture_detector.remove_whitelist(keyword);
        self.engine.save()?;
        Ok(removed)
    }

    pub fn test_lecture(&self, artist: &str) -> bool {
        self.engine.lecture_detector.is_lecture(artist)
    }

    // ── Logs ────────────────────────────────────────────────────────────

    pub fn get_logs(&self, since_index: Option<usize>) -> Vec<LogEntry> {
        self.logs.get(since_index.unwrap_or(0))
    }

    pub fn clear_logs(&mut self) {
        self.logs.clear();
    }

    pub fn log(&mut self, level: &str, message: String) {
        self.logs.push(level, message);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

fn is_audio_file(path: &Path) -> bool {
    const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "ogg", "aac", "m4a"];
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn collect_matches(path: &Path, query: &str, out: &mut Vec<FileSearchResult>, depth: usize) {
    if depth > 5 || out.len() >= 120 {
        return;
    }
    let read = match fs::read_dir(path) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in read.flatten() {
        if out.len() >= 120 {
            return;
        }
        // Use path.join(file_name) instead of entry.path() to preserve
        // the user-provided drive letter on Windows mapped drives.
        let p = path.join(entry.file_name());
        let name = entry.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            collect_matches(&p, query, out, depth + 1);
            continue;
        }
        if !is_audio_file(&p) {
            continue;
        }
        let stem = Path::new(&name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&name)
            .to_lowercase();
        if stem.contains(query) {
            out.push(FileSearchResult {
                path: p.to_string_lossy().to_string(),
                name,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_core() -> AppCore {
        AppCore::new_test()
    }

    // -- Playlist CRUD --

    #[test]
    fn create_and_list_playlists() {
        let mut core = make_core();
        let id = core.create_playlist("Main".to_string()).unwrap();
        assert!(id > 0);

        let playlists = core.get_playlists();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "Main");
        assert_eq!(playlists[0].track_count, 0);
        assert!(!playlists[0].is_active);
    }

    #[test]
    fn create_duplicate_playlist_errors() {
        let mut core = make_core();
        core.create_playlist("Main".to_string()).unwrap();
        let result = core.create_playlist("main".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn delete_playlist() {
        let mut core = make_core();
        core.create_playlist("Temp".to_string()).unwrap();
        assert_eq!(core.get_playlists().len(), 1);
        core.delete_playlist("Temp").unwrap();
        assert_eq!(core.get_playlists().len(), 0);
    }

    #[test]
    fn delete_active_playlist_clears_active() {
        let mut core = make_core();
        core.create_playlist("Main".to_string()).unwrap();
        core.set_active_playlist("Main").unwrap();
        assert!(core.get_status().active_playlist.is_some());
        core.delete_playlist("Main").unwrap();
        assert!(core.get_status().active_playlist.is_none());
    }

    #[test]
    fn delete_nonexistent_playlist_errors() {
        let mut core = make_core();
        assert!(core.delete_playlist("Ghost").is_err());
    }

    #[test]
    fn rename_playlist() {
        let mut core = make_core();
        core.create_playlist("Old".to_string()).unwrap();
        core.rename_playlist("Old", "New".to_string()).unwrap();
        let playlists = core.get_playlists();
        assert_eq!(playlists[0].name, "New");
    }

    #[test]
    fn rename_to_existing_name_errors() {
        let mut core = make_core();
        core.create_playlist("A".to_string()).unwrap();
        core.create_playlist("B".to_string()).unwrap();
        assert!(core.rename_playlist("A", "B".to_string()).is_err());
    }

    #[test]
    fn set_active_playlist() {
        let mut core = make_core();
        core.create_playlist("Main".to_string()).unwrap();
        let id = core.set_active_playlist("Main").unwrap();
        assert!(id > 0);

        let status = core.get_status();
        assert_eq!(status.active_playlist, Some("Main".to_string()));

        let playlists = core.get_playlists();
        assert!(playlists[0].is_active);
    }

    #[test]
    fn set_active_nonexistent_errors() {
        let mut core = make_core();
        assert!(core.set_active_playlist("Ghost").is_err());
    }

    // -- Config --

    #[test]
    fn get_default_config() {
        let core = make_core();
        let config = core.get_config();
        assert_eq!(config.crossfade_secs, 0.0);
        assert_eq!(config.silence_duration_secs, 0.0);
        assert!(config.intros_folder.is_none());
        assert_eq!(config.conflict_policy, "schedule-wins");
        assert!(config.indexed_locations.is_empty());
        assert!(config.favorite_folders.is_empty());
    }

    #[test]
    fn set_crossfade() {
        let mut core = make_core();
        core.set_crossfade(3.5).unwrap();
        assert_eq!(core.get_config().crossfade_secs, 3.5);
    }

    #[test]
    fn set_silence_detection() {
        let mut core = make_core();
        core.set_silence_detection(0.02, 5.0).unwrap();
        let config = core.get_config();
        assert_eq!(config.silence_threshold, 0.02);
        assert_eq!(config.silence_duration_secs, 5.0);
    }

    #[test]
    fn set_recurring_intro() {
        let mut core = make_core();
        core.set_recurring_intro(900.0, 0.2).unwrap();
        let status = core.get_status();
        assert_eq!(status.recurring_intro_interval_secs, 900.0);
        assert_eq!(status.recurring_intro_duck_volume, 0.2);
    }

    #[test]
    fn set_conflict_policy() {
        let mut core = make_core();
        core.set_conflict_policy("manual-wins").unwrap();
        assert_eq!(core.get_config().conflict_policy, "manual-wins");
    }

    #[test]
    fn set_conflict_policy_invalid_errors() {
        let mut core = make_core();
        assert!(core.set_conflict_policy("bogus").is_err());
    }

    #[test]
    fn set_indexed_locations_deduplicates_and_trims() {
        let mut core = make_core();
        core.set_indexed_locations(vec![
            "  /music/a  ".to_string(),
            "/music/a".to_string(),
            "".to_string(),
            " /music/b".to_string(),
        ])
        .unwrap();
        let config = core.get_config();
        assert_eq!(config.indexed_locations, vec!["/music/a", "/music/b"]);
    }

    #[test]
    fn set_favorite_folders_deduplicates_and_trims() {
        let mut core = make_core();
        core.set_favorite_folders(vec![
            " /music/news ".to_string(),
            "/music/news".to_string(),
            " /music/ads".to_string(),
        ])
        .unwrap();
        let config = core.get_config();
        assert_eq!(config.favorite_folders, vec!["/music/ads", "/music/news"]);
    }

    #[test]
    fn set_nowplaying_path() {
        let mut core = make_core();
        core.set_nowplaying_path(Some("/tmp/np.xml".to_string()))
            .unwrap();
        assert_eq!(
            core.get_config().now_playing_path,
            Some("/tmp/np.xml".to_string())
        );
    }

    #[test]
    fn set_stream_output() {
        let mut core = make_core();
        core.set_stream_output(
            true,
            "icecast://source:pass@localhost:8000/live".to_string(),
        )
        .unwrap();
        let config = core.get_config();
        assert!(config.stream_output_enabled);
        assert_eq!(
            config.stream_output_url,
            "icecast://source:pass@localhost:8000/live"
        );
    }

    #[test]
    fn set_stream_output_requires_url_when_enabled() {
        let mut core = make_core();
        assert!(core.set_stream_output(true, "   ".to_string()).is_err());
    }

    #[test]
    fn set_recording() {
        let mut core = make_core();
        core.set_recording(true, Some("/tmp/records".to_string()))
            .unwrap();
        let config = core.get_config();
        assert!(config.recording_enabled);
        assert_eq!(
            config.recording_output_dir,
            Some("/tmp/records".to_string())
        );
    }

    #[test]
    fn set_recording_requires_output_dir_when_enabled() {
        let mut core = make_core();
        assert!(core.set_recording(true, None).is_err());
    }

    // -- Schedule --

    #[test]
    fn add_and_get_schedule() {
        let mut core = make_core();
        let id = core
            .add_schedule_event(
                "14:00",
                "stop",
                "news.mp3",
                Some(9),
                Some("News".to_string()),
                None,
            )
            .unwrap();
        assert!(id > 0);

        let events = core.get_schedule();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].mode, "stop");
        assert_eq!(events[0].priority, 9);
        assert_eq!(events[0].label, Some("News".to_string()));
    }

    #[test]
    fn add_schedule_event_bad_time_errors() {
        let mut core = make_core();
        assert!(core
            .add_schedule_event("25:00", "stop", "x.mp3", None, None, None)
            .is_err());
    }

    #[test]
    fn add_schedule_event_bad_mode_errors() {
        let mut core = make_core();
        assert!(core
            .add_schedule_event("14:00", "bogus", "x.mp3", None, None, None)
            .is_err());
    }

    #[test]
    fn remove_schedule_event() {
        let mut core = make_core();
        let id = core
            .add_schedule_event("12:00", "overlay", "jingle.mp3", None, None, None)
            .unwrap();
        core.remove_schedule_event(id).unwrap();
        assert!(core.get_schedule().is_empty());
    }

    #[test]
    fn toggle_schedule_event() {
        let mut core = make_core();
        let id = core
            .add_schedule_event("12:00", "insert", "promo.mp3", None, None, None)
            .unwrap();
        let disabled = core.toggle_schedule_event(id).unwrap();
        assert!(!disabled);
        let enabled = core.toggle_schedule_event(id).unwrap();
        assert!(enabled);
    }

    // -- Ads --

    #[test]
    fn add_and_get_ads() {
        let mut core = make_core();
        let idx = core
            .add_ad("Test Ad".to_string(), "ad.mp3".to_string())
            .unwrap();
        assert_eq!(idx, 0);

        let ads = core.get_ads();
        assert_eq!(ads.len(), 1);
        assert_eq!(ads[0].name, "Test Ad");
        assert!(ads[0].enabled);
    }

    #[test]
    fn remove_ad() {
        let mut core = make_core();
        core.add_ad("Ad1".to_string(), "ad1.mp3".to_string())
            .unwrap();
        core.remove_ad(0).unwrap();
        assert!(core.get_ads().is_empty());
    }

    #[test]
    fn remove_ad_out_of_range_errors() {
        let mut core = make_core();
        assert!(core.remove_ad(0).is_err());
    }

    #[test]
    fn toggle_ad() {
        let mut core = make_core();
        core.add_ad("Ad1".to_string(), "ad1.mp3".to_string())
            .unwrap();
        let disabled = core.toggle_ad(0).unwrap();
        assert!(!disabled);
        let enabled = core.toggle_ad(0).unwrap();
        assert!(enabled);
    }

    #[test]
    fn update_ad() {
        let mut core = make_core();
        core.add_ad("Original".to_string(), "old.mp3".to_string())
            .unwrap();
        core.update_ad(
            0,
            "Updated".to_string(),
            false,
            "new.mp3".to_string(),
            true,
            vec!["Monday".to_string()],
            vec![8, 9, 10],
        )
        .unwrap();

        let ads = core.get_ads();
        assert_eq!(ads[0].name, "Updated");
        assert!(!ads[0].enabled);
        assert_eq!(ads[0].mp3_file, "new.mp3");
        assert!(ads[0].scheduled);
        assert_eq!(ads[0].days, vec!["Monday".to_string()]);
        assert_eq!(ads[0].hours, vec![8, 9, 10]);
    }

    #[test]
    fn reorder_ad() {
        let mut core = make_core();
        core.add_ad("First".to_string(), "1.mp3".to_string())
            .unwrap();
        core.add_ad("Second".to_string(), "2.mp3".to_string())
            .unwrap();
        core.reorder_ad(0, 1).unwrap();

        let ads = core.get_ads();
        assert_eq!(ads[0].name, "Second");
        assert_eq!(ads[1].name, "First");
    }

    #[test]
    fn reorder_ad_out_of_range_errors() {
        let mut core = make_core();
        core.add_ad("Ad1".to_string(), "1.mp3".to_string()).unwrap();
        assert!(core.reorder_ad(0, 5).is_err());
    }

    // -- RDS --

    #[test]
    fn get_default_rds_config() {
        let core = make_core();
        let rds = core.get_rds_config();
        assert_eq!(rds.ip, "127.0.0.1");
        assert_eq!(rds.port, 10001);
        assert!(rds.messages.is_empty());
    }

    #[test]
    fn add_and_get_rds_message() {
        let mut core = make_core();
        let idx = core.add_rds_message("Test RDS".to_string()).unwrap();
        assert_eq!(idx, 0);

        let rds = core.get_rds_config();
        assert_eq!(rds.messages.len(), 1);
        assert_eq!(rds.messages[0].text, "Test RDS");
        // RdsMessage::new() defaults to enabled=false
        assert!(!rds.messages[0].enabled);
    }

    #[test]
    fn remove_rds_message() {
        let mut core = make_core();
        core.add_rds_message("Msg".to_string()).unwrap();
        core.remove_rds_message(0).unwrap();
        assert!(core.get_rds_config().messages.is_empty());
    }

    #[test]
    fn remove_rds_message_out_of_range_errors() {
        let mut core = make_core();
        assert!(core.remove_rds_message(0).is_err());
    }

    #[test]
    fn toggle_rds_message() {
        let mut core = make_core();
        core.add_rds_message("Msg".to_string()).unwrap();
        // RdsMessage::new() defaults to enabled=false, so first toggle enables
        let enabled = core.toggle_rds_message(0).unwrap();
        assert!(enabled);
        let disabled = core.toggle_rds_message(0).unwrap();
        assert!(!disabled);
    }

    #[test]
    fn update_rds_message() {
        let mut core = make_core();
        core.add_rds_message("Original".to_string()).unwrap();
        core.update_rds_message(
            0,
            "Updated".to_string(),
            false,
            30,
            true,
            vec!["Friday".to_string()],
            vec![18, 19, 20],
        )
        .unwrap();

        let rds = core.get_rds_config();
        let msg = &rds.messages[0];
        assert_eq!(msg.text, "Updated");
        assert!(!msg.enabled);
        assert_eq!(msg.duration, 30);
        assert!(msg.scheduled);
        assert_eq!(msg.days, vec!["Friday".to_string()]);
        assert_eq!(msg.hours, vec![18, 19, 20]);
    }

    #[test]
    fn reorder_rds_message() {
        let mut core = make_core();
        core.add_rds_message("First".to_string()).unwrap();
        core.add_rds_message("Second".to_string()).unwrap();
        core.reorder_rds_message(0, 1).unwrap();

        let rds = core.get_rds_config();
        assert_eq!(rds.messages[0].text, "Second");
        assert_eq!(rds.messages[1].text, "First");
    }

    #[test]
    fn update_rds_settings() {
        let mut core = make_core();
        core.update_rds_settings("10.0.0.1".to_string(), 5000, "Hello Radio".to_string())
            .unwrap();
        let rds = core.get_rds_config();
        assert_eq!(rds.ip, "10.0.0.1");
        assert_eq!(rds.port, 5000);
        assert_eq!(rds.default_message, "Hello Radio");
    }

    // -- Lecture Detector --

    #[test]
    fn lecture_detector_operations() {
        let mut core = make_core();
        // Default: starts-with-R heuristic
        assert!(core.test_lecture("Rabbi Shalom"));
        assert!(!core.test_lecture("The Beatles"));

        // Blacklist overrides
        core.lecture_blacklist_add("Rihanna").unwrap();
        assert!(!core.test_lecture("Rihanna"));

        // Whitelist
        core.lecture_whitelist_add("Special Speaker").unwrap();
        assert!(core.test_lecture("Special Speaker"));

        // Config read
        let config = core.get_lecture_config();
        assert!(config.blacklist.contains(&"rihanna".to_string()));
        assert!(config.whitelist.contains(&"special speaker".to_string()));

        // Remove
        assert!(core.lecture_blacklist_remove("Rihanna").unwrap());
        assert!(core.test_lecture("Rihanna")); // back to starts-with-R
    }

    // -- Logs --

    #[test]
    fn log_operations() {
        let mut core = make_core();
        core.log("info", "Test message".to_string());
        core.log("error", "Error message".to_string());

        let logs = core.get_logs(None);
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].level, "info");
        assert_eq!(logs[1].level, "error");

        let logs_since = core.get_logs(Some(1));
        assert_eq!(logs_since.len(), 1);
        assert_eq!(logs_since[0].level, "error");

        core.clear_logs();
        assert!(core.get_logs(None).is_empty());
    }

    // -- Status --

    #[test]
    fn get_status_reflects_state() {
        let mut core = make_core();
        let status = core.get_status();
        assert_eq!(status.playlist_count, 0);
        assert!(status.active_playlist.is_none());
        assert_eq!(status.schedule_event_count, 0);

        core.create_playlist("Main".to_string()).unwrap();
        core.set_active_playlist("Main").unwrap();
        core.add_schedule_event("12:00", "overlay", "jingle.mp3", None, None, None)
            .unwrap();

        let status = core.get_status();
        assert_eq!(status.playlist_count, 1);
        assert_eq!(status.active_playlist, Some("Main".to_string()));
        assert_eq!(status.schedule_event_count, 1);
    }

    // -- Transport state (no audio) --

    #[test]
    fn transport_state_defaults() {
        let core = make_core();
        let state = core.get_transport_state();
        assert!(!state.is_playing);
        assert!(!state.is_paused);
        assert_eq!(state.elapsed_secs, 0.0);
        assert!(state.track_index.is_none());
    }

    #[test]
    fn on_stop_resets_playback() {
        let mut core = make_core();
        core.playback.is_playing = true;
        core.playback.track_index = Some(3);
        core.on_stop();
        assert!(!core.playback.is_playing);
        assert!(core.playback.track_index.is_none());
    }

    #[test]
    fn on_pause_toggle_without_playing_errors() {
        let mut core = make_core();
        assert!(core.on_pause_toggle().is_err());
    }

    #[test]
    fn on_pause_toggle_pauses_and_resumes() {
        let mut core = make_core();
        core.playback.is_playing = true;
        core.playback.start_time = Some(Instant::now());

        let paused = core.on_pause_toggle().unwrap();
        assert!(paused);
        assert!(core.playback.is_paused);

        let resumed = core.on_pause_toggle().unwrap();
        assert!(!resumed);
        assert!(!core.playback.is_paused);
    }

    #[test]
    fn on_seek_updates_timing() {
        let mut core = make_core();
        core.playback.is_playing = true;
        core.playback.start_time = Some(Instant::now());

        core.on_seek(30.0).unwrap();
        let elapsed = core.playback.elapsed();
        // Should be close to 30 seconds (within tolerance for test timing)
        assert!(elapsed.as_secs_f64() >= 29.9 && elapsed.as_secs_f64() <= 30.5);
    }

    #[test]
    fn on_seek_without_playing_errors() {
        let mut core = make_core();
        assert!(core.on_seek(10.0).is_err());
    }

    // -- Copy/Paste --

    #[test]
    fn copy_paste_tracks() {
        let mut core = make_core();
        core.create_playlist("Src".to_string()).unwrap();
        core.create_playlist("Dest".to_string()).unwrap();

        // Manually add test tracks (can't use add_track without real files)
        let track = crate::track::Track {
            path: "test.mp3".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            duration: Duration::from_secs(180),
            played_duration: None,
            has_intro: false,
        };
        core.engine
            .find_playlist_mut("Src")
            .unwrap()
            .tracks
            .push(track);

        let copied = core.copy_tracks("Src", &[0]).unwrap();
        assert_eq!(copied.len(), 1);
        assert_eq!(copied[0].title, "Song");

        core.paste_tracks("Dest", copied, None).unwrap();
        let tracks = core.engine.find_playlist("Dest").unwrap();
        assert_eq!(tracks.track_count(), 1);
    }
    #[test]
    fn search_ignores_single_character_query() {
        let core = make_core();
        assert!(core.search_indexed_files("a").unwrap().is_empty());
    }

    #[test]
    fn search_matches_filename_without_extension() {
        let temp = tempfile::tempdir().unwrap();
        let audio_path = temp.path().join("MySong.mp3");
        fs::write(&audio_path, b"not real audio").unwrap();

        let mut core = make_core();
        core.engine
            .indexed_locations
            .push(temp.path().to_string_lossy().to_string());

        let matches = core.search_indexed_files("mysong").unwrap();
        assert!(matches.iter().any(|m| m.name == "MySong.mp3"));
    }

    #[test]
    fn export_and_import_m3u_playlist_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("sample.m3u8");
        fs::write(
            &source,
            "#EXTM3U
track-a.mp3
",
        )
        .unwrap();

        let mut core = make_core();
        let imported = core.import_m3u_playlist(&source.to_string_lossy()).unwrap();
        assert_eq!(imported, "sample");

        let out = temp.path().join("out.m3u");
        core.export_playlist_to_m3u(&imported, Some(&out.to_string_lossy()))
            .unwrap();
        let saved = fs::read_to_string(&out).unwrap();
        assert!(saved.contains("#EXTM3U"));

        core.save_playlist_profile("WithPath").unwrap();
        let profiles = core.get_playlist_profiles();
        assert_eq!(profiles[0].playlist_paths.len(), 1);
        assert_eq!(
            profiles[0].playlist_paths[0],
            Some(out.to_string_lossy().to_string())
        );
    }
    #[test]
    fn playlist_profiles_roundtrip() {
        let mut core = make_core();
        core.create_playlist("News".to_string()).unwrap();
        core.create_playlist("Music".to_string()).unwrap();

        core.save_playlist_profile("Morning").unwrap();
        assert_eq!(core.get_playlist_profiles().len(), 1);

        core.delete_playlist("News").unwrap();
        core.delete_playlist("Music").unwrap();
        assert!(core.get_playlists().is_empty());

        core.load_playlist_profile("Morning").unwrap();
        let playlists = core.get_playlists();
        assert_eq!(playlists.len(), 2);
        assert!(playlists.iter().any(|p| p.name == "News"));
        assert!(playlists.iter().any(|p| p.name == "Music"));

        core.delete_playlist_profile("Morning").unwrap();
        assert!(core.get_playlist_profiles().is_empty());
    }
}
