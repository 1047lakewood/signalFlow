use crate::ad_scheduler::{AdConfig, AdInserterSettings};
use crate::lecture_detector::LectureDetector;
use crate::playlist::Playlist;
use crate::rds::RdsConfig;
use crate::scheduler::{ConflictPolicy, Schedule};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const STATE_FILE: &str = "signalflow_state.json";

fn default_duck_volume() -> f32 {
    0.3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistProfile {
    pub name: String,
    pub playlist_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOutputConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub endpoint_url: String,
}

impl Default for StreamOutputConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint_url: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub output_dir: Option<String>,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            output_dir: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Engine {
    pub playlists: Vec<Playlist>,
    pub active_playlist_id: Option<u32>,
    next_id: u32,
    #[serde(default)]
    pub crossfade_secs: f32,
    /// RMS threshold below which audio is considered silent (e.g., 0.01).
    #[serde(default)]
    pub silence_threshold: f32,
    /// Seconds of continuous silence before auto-skip (0 = disabled).
    #[serde(default)]
    pub silence_duration_secs: f32,
    /// Path to folder containing artist intro files (None = disabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intros_folder: Option<String>,
    /// Scheduled events (timed triggers for overlay, stop, insert).
    #[serde(default)]
    pub schedule: Schedule,
    /// How to resolve conflicts between manual playback and scheduled events.
    #[serde(default)]
    pub conflict_policy: ConflictPolicy,
    /// Interval in seconds for recurring intro overlays (0 = disabled).
    /// When > 0, re-plays the artist intro every N seconds during track playback.
    #[serde(default)]
    pub recurring_intro_interval_secs: f32,
    /// Volume level for main track during recurring intro overlay (0.0–1.0, default 0.3).
    #[serde(default = "default_duck_volume")]
    pub recurring_intro_duck_volume: f32,
    /// Path for now-playing XML export (None = disabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now_playing_path: Option<String>,
    /// Ad definitions for the ad scheduler/inserter system.
    #[serde(default)]
    pub ads: Vec<AdConfig>,
    /// Ad inserter service settings (output path, station ID).
    #[serde(default)]
    pub ad_inserter: AdInserterSettings,
    /// Lecture detector with blacklist/whitelist (shared between ad scheduler and RDS).
    #[serde(default)]
    pub lecture_detector: LectureDetector,
    /// RDS (Radio Data System) message rotation configuration.
    #[serde(default)]
    pub rds: RdsConfig,
    /// Internet stream relay output (ffmpeg-based sidecar).
    #[serde(default)]
    pub stream_output: StreamOutputConfig,
    /// Daily playback recording output settings.
    #[serde(default)]
    pub recording: RecordingConfig,
    /// Folders indexed by the file browser search.
    #[serde(default)]
    pub indexed_locations: Vec<String>,
    /// User-pinned favorite folders shown in the file browser pane.
    #[serde(default)]
    pub favorite_folders: Vec<String>,
    /// Saved named profiles of open playlists.
    #[serde(default)]
    pub playlist_profiles: Vec<PlaylistProfile>,
    /// Runtime-only: path to the state file. Not serialized.
    #[serde(skip)]
    state_path: Option<PathBuf>,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            playlists: Vec::new(),
            active_playlist_id: None,
            next_id: 1,
            crossfade_secs: 0.0,
            silence_threshold: 0.01,
            silence_duration_secs: 0.0,
            intros_folder: None,
            recurring_intro_interval_secs: 0.0,
            recurring_intro_duck_volume: 0.3,
            schedule: Schedule::new(),
            conflict_policy: ConflictPolicy::default(),
            now_playing_path: None,
            ads: Vec::new(),
            ad_inserter: AdInserterSettings::default(),
            lecture_detector: LectureDetector::new(),
            rds: RdsConfig::default(),
            stream_output: StreamOutputConfig::default(),
            recording: RecordingConfig::default(),
            indexed_locations: Vec::new(),
            favorite_folders: Vec::new(),
            playlist_profiles: Vec::new(),
            state_path: None,
        }
    }

    /// Returns the state file path, or None if in-memory mode (tests).
    pub fn state_path(&self) -> Option<&Path> {
        self.state_path.as_deref()
    }

    /// Load engine state from the default state file (CWD).
    pub fn load() -> Self {
        Self::load_from(Path::new(STATE_FILE))
    }

    /// Load engine state from a specific path.
    pub fn load_from(path: &Path) -> Self {
        if path.exists() {
            match fs::read_to_string(path) {
                Ok(data) => match serde_json::from_str::<Engine>(&data) {
                    Ok(mut engine) => {
                        engine.state_path = Some(path.to_path_buf());
                        engine.migrate_unc_paths();
                        return engine;
                    }
                    Err(e) => eprintln!("Warning: corrupt state file, starting fresh: {}", e),
                },
                Err(e) => eprintln!("Warning: could not read state file: {}", e),
            }
        }
        let mut engine = Engine::new();
        engine.state_path = Some(path.to_path_buf());
        engine
    }

    /// Rewrite `\\?\UNC\...` paths in all playlist tracks to use mapped drive
    /// letters where possible. On Windows, `DirEntry::path()` returns verbatim
    /// UNC paths for files on mapped network drives, which is confusing in the UI.
    fn migrate_unc_paths(&mut self) {
        let mapping = build_unc_to_drive_map();
        if mapping.is_empty() {
            return;
        }
        let mut changed = false;
        for pl in &mut self.playlists {
            for track in &mut pl.tracks {
                let path_str = track.path.to_string_lossy().to_string();
                if let Some(rewritten) = rewrite_unc_path(&path_str, &mapping) {
                    track.path = PathBuf::from(rewritten);
                    changed = true;
                }
            }
        }
        if changed {
            let _ = self.save();
        }
    }

    /// Persist current state to JSON.
    /// When `state_path` is None (in-memory / test mode), this is a no-op.
    pub fn save(&self) -> Result<(), String> {
        let path = match &self.state_path {
            Some(p) => p.as_path(),
            None => return Ok(()), // In-memory mode — skip file I/O
        };
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| format!("Create dir error: {}", e))?;
            }
        }
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("Serialize error: {}", e))?;
        fs::write(path, json).map_err(|e| format!("Write error: {}", e))?;
        Ok(())
    }

    /// Create a new playlist with the given name. Returns its ID.
    pub fn create_playlist(&mut self, name: String) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.playlists.push(Playlist::new(id, name));
        id
    }

    /// Find a playlist by name (case-insensitive).
    pub fn find_playlist(&self, name: &str) -> Option<&Playlist> {
        self.playlists
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Find a playlist by name (case-insensitive), mutable.
    pub fn find_playlist_mut(&mut self, name: &str) -> Option<&mut Playlist> {
        self.playlists
            .iter_mut()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Set the active playlist by name. Returns the playlist ID or an error.
    pub fn set_active(&mut self, name: &str) -> Result<u32, String> {
        let id = self
            .find_playlist(name)
            .map(|p| p.id)
            .ok_or_else(|| format!("Playlist '{}' not found", name))?;
        self.active_playlist_id = Some(id);
        Ok(id)
    }

    /// Get the active playlist, if any.
    pub fn active_playlist(&self) -> Option<&Playlist> {
        self.active_playlist_id
            .and_then(|id| self.playlists.iter().find(|p| p.id == id))
    }

    /// Get the active playlist mutably, if any.
    pub fn active_playlist_mut(&mut self) -> Option<&mut Playlist> {
        self.active_playlist_id
            .and_then(|id| self.playlists.iter_mut().find(|p| p.id == id))
    }

    /// Copy tracks from a playlist by indices (0-based). Returns cloned tracks.
    pub fn copy_tracks(
        &self,
        from_name: &str,
        indices: &[usize],
    ) -> Result<Vec<crate::track::Track>, String> {
        let pl = self
            .find_playlist(from_name)
            .ok_or_else(|| format!("Playlist '{}' not found", from_name))?;
        let mut tracks = Vec::new();
        for &idx in indices {
            if idx >= pl.tracks.len() {
                return Err(format!(
                    "Index {} out of range (playlist '{}' has {} tracks)",
                    idx,
                    from_name,
                    pl.tracks.len()
                ));
            }
            tracks.push(pl.tracks[idx].clone());
        }
        Ok(tracks)
    }

    /// Insert a track as the next item in the active playlist (after current_index).
    /// Used by the scheduler's Insert mode to queue a file as the next track.
    /// Returns the insertion position (0-based) or an error.
    pub fn insert_next_track(&mut self, path: &std::path::Path) -> Result<usize, String> {
        let track = crate::track::Track::from_path(path)?;
        let pl = self
            .active_playlist_mut()
            .ok_or_else(|| "No active playlist".to_string())?;
        let insert_pos = match pl.current_index {
            Some(i) => i + 1,
            None => 0,
        };
        pl.insert_tracks(vec![track], Some(insert_pos))?;
        Ok(insert_pos)
    }

    /// Edit metadata (artist/title) for a track in a playlist. Writes changes to file tags.
    /// `playlist_name` is case-insensitive. `track_index` is 0-based.
    pub fn edit_track_metadata(
        &mut self,
        playlist_name: &str,
        track_index: usize,
        new_artist: Option<&str>,
        new_title: Option<&str>,
    ) -> Result<(), String> {
        let pl = self
            .find_playlist_mut(playlist_name)
            .ok_or_else(|| format!("Playlist '{}' not found", playlist_name))?;
        let track_count = pl.tracks.len();
        let track = pl.tracks.get_mut(track_index).ok_or_else(|| {
            format!(
                "Track index {} out of range (playlist '{}' has {} tracks)",
                track_index, playlist_name, track_count
            )
        })?;
        track.write_tags(new_artist, new_title)
    }

    // --- Ad management ---

    /// Add a new ad configuration. Returns its index (0-based).
    pub fn add_ad(&mut self, ad: AdConfig) -> usize {
        self.ads.push(ad);
        self.ads.len() - 1
    }

    /// Remove an ad by index (0-based). Returns the removed ad.
    pub fn remove_ad(&mut self, index: usize) -> Result<AdConfig, String> {
        if index >= self.ads.len() {
            return Err(format!(
                "Ad index {} out of range ({} ads)",
                index,
                self.ads.len()
            ));
        }
        Ok(self.ads.remove(index))
    }

    /// Find an ad by name (case-insensitive).
    pub fn find_ad(&self, name: &str) -> Option<(usize, &AdConfig)> {
        self.ads
            .iter()
            .enumerate()
            .find(|(_, a)| a.name.eq_ignore_ascii_case(name))
    }

    /// Toggle an ad's enabled state. Returns the new state.
    pub fn toggle_ad(&mut self, index: usize) -> Result<bool, String> {
        let len = self.ads.len();
        let ad = self
            .ads
            .get_mut(index)
            .ok_or_else(|| format!("Ad index {} out of range ({} ads)", index, len))?;
        ad.enabled = !ad.enabled;
        Ok(ad.enabled)
    }

    /// Get the path of the currently playing track from the active playlist.
    pub fn current_track_path(&self) -> Option<&Path> {
        let pl = self.active_playlist()?;
        let idx = pl.current_index?;
        let track = pl.tracks.get(idx)?;
        Some(&track.path)
    }

    /// Get the current track's artist and title from the active playlist.
    pub fn current_track_info(&self) -> Option<(&str, &str)> {
        let pl = self.active_playlist()?;
        let idx = pl.current_index?;
        let track = pl.tracks.get(idx)?;
        Some((&track.artist, &track.title))
    }

    /// Get the next track's artist from the active playlist.
    pub fn next_track_artist(&self) -> Option<&str> {
        let pl = self.active_playlist()?;
        let idx = pl.current_index?;
        let next = pl.tracks.get(idx + 1)?;
        Some(&next.artist)
    }

    /// Check if there is a next track in the active playlist.
    pub fn has_next_track(&self) -> bool {
        self.active_playlist()
            .and_then(|pl| pl.current_index.map(|idx| idx + 1 < pl.tracks.len()))
            .unwrap_or(false)
    }

    /// Paste (insert) tracks into a playlist at a position, or append.
    pub fn paste_tracks(
        &mut self,
        to_name: &str,
        tracks: Vec<crate::track::Track>,
        at: Option<usize>,
    ) -> Result<(), String> {
        let pl = self
            .find_playlist_mut(to_name)
            .ok_or_else(|| format!("Playlist '{}' not found", to_name))?;
        pl.insert_tracks(tracks, at)
    }
}

/// Build a map of UNC share roots to drive letters by querying Windows drive mappings.
/// Returns e.g. `[("\\\\RadioNAS\\104.7", "G:")]`. Empty on non-Windows or if no mapped drives.
fn build_unc_to_drive_map() -> Vec<(String, String)> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        let mut mapping = Vec::new();
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:", letter as char);
            let wide_drive: Vec<u16> = drive.encode_utf16().chain(std::iter::once(0)).collect();
            let mut buf = vec![0u16; 1024];
            let mut buf_len: u32 = buf.len() as u32;
            // WNetGetConnectionW returns the remote UNC name for a mapped drive letter.
            let result = unsafe {
                windows_sys::Win32::NetworkManagement::WNet::WNetGetConnectionW(
                    wide_drive.as_ptr(),
                    buf.as_mut_ptr(),
                    &mut buf_len,
                )
            };
            // NO_ERROR = 0 means success
            if result != 0 {
                continue;
            }
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf_len as usize);
            let unc = OsString::from_wide(&buf[..end])
                .to_string_lossy()
                .to_string();
            if unc.starts_with("\\\\") {
                mapping.push((unc, drive));
            }
        }
        mapping
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

/// If `path_str` starts with `\\?\UNC\` or `\\`, try to replace the UNC share root
/// with a mapped drive letter.
fn rewrite_unc_path(path_str: &str, mapping: &[(String, String)]) -> Option<String> {
    // Strip verbatim prefix first: \\?\UNC\server\share → \\server\share
    let normalized = if path_str.starts_with(r"\\?\UNC\") {
        format!(r"\\{}", &path_str[8..])
    } else if path_str.starts_with(r"\\") {
        path_str.to_string()
    } else {
        return None;
    };

    // Try each known UNC→drive mapping (case-insensitive prefix match)
    let norm_lower = normalized.to_lowercase();
    for (unc_root, drive) in mapping {
        let root_lower = unc_root.to_lowercase();
        if norm_lower.starts_with(&root_lower) {
            let rest = &normalized[unc_root.len()..];
            // rest is either empty, or starts with '\' (subfolder)
            return Some(format!("{}{}", drive, rest));
        }
    }
    None
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_playlist_assigns_unique_ids() {
        let mut engine = Engine::new();
        let id1 = engine.create_playlist("A".to_string());
        let id2 = engine.create_playlist("B".to_string());
        assert_ne!(id1, id2);
        assert_eq!(engine.playlists.len(), 2);
    }

    #[test]
    fn find_playlist_case_insensitive() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        assert!(engine.find_playlist("main").is_some());
        assert!(engine.find_playlist("MAIN").is_some());
        assert!(engine.find_playlist("nope").is_none());
    }

    #[test]
    fn set_active_and_retrieve() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        engine.set_active("Main").unwrap();
        let active = engine.active_playlist().unwrap();
        assert_eq!(active.name, "Main");
    }

    #[test]
    fn set_active_nonexistent_errors() {
        let mut engine = Engine::new();
        assert!(engine.set_active("ghost").is_err());
    }

    #[test]
    fn crossfade_secs_defaults_to_zero() {
        let engine = Engine::new();
        assert_eq!(engine.crossfade_secs, 0.0);
    }

    #[test]
    fn crossfade_secs_survives_serialization() {
        let mut engine = Engine::new();
        engine.crossfade_secs = 3.5;
        let json = serde_json::to_string(&engine).unwrap();
        let loaded: Engine = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.crossfade_secs, 3.5);
    }

    #[test]
    fn crossfade_secs_defaults_when_missing_from_json() {
        // Simulate loading an old state file without crossfade_secs
        let json = r#"{"playlists":[],"active_playlist_id":null,"next_id":1}"#;
        let engine: Engine = serde_json::from_str(json).unwrap();
        assert_eq!(engine.crossfade_secs, 0.0);
    }

    #[test]
    fn silence_fields_default_correctly() {
        let engine = Engine::new();
        assert_eq!(engine.silence_threshold, 0.01);
        assert_eq!(engine.silence_duration_secs, 0.0);
    }

    #[test]
    fn silence_fields_survive_serialization() {
        let mut engine = Engine::new();
        engine.silence_threshold = 0.005;
        engine.silence_duration_secs = 5.0;
        let json = serde_json::to_string(&engine).unwrap();
        let loaded: Engine = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.silence_threshold, 0.005);
        assert_eq!(loaded.silence_duration_secs, 5.0);
    }

    #[test]
    fn silence_fields_default_when_missing_from_json() {
        let json = r#"{"playlists":[],"active_playlist_id":null,"next_id":1}"#;
        let engine: Engine = serde_json::from_str(json).unwrap();
        assert_eq!(engine.silence_threshold, 0.0);
        assert_eq!(engine.silence_duration_secs, 0.0);
    }

    #[test]
    fn stream_and_recording_defaults() {
        let engine = Engine::new();
        assert!(!engine.stream_output.enabled);
        assert!(engine.stream_output.endpoint_url.is_empty());
        assert!(!engine.recording.enabled);
        assert!(engine.recording.output_dir.is_none());
    }

    #[test]
    fn stream_and_recording_survive_serialization() {
        let mut engine = Engine::new();
        engine.stream_output.enabled = true;
        engine.stream_output.endpoint_url = "icecast://example/live".to_string();
        engine.recording.enabled = true;
        engine.recording.output_dir = Some("/tmp/records".to_string());

        let json = serde_json::to_string(&engine).unwrap();
        let loaded: Engine = serde_json::from_str(&json).unwrap();
        assert!(loaded.stream_output.enabled);
        assert_eq!(loaded.stream_output.endpoint_url, "icecast://example/live");
        assert!(loaded.recording.enabled);
        assert_eq!(loaded.recording.output_dir.as_deref(), Some("/tmp/records"));
    }

    fn make_track(name: &str) -> crate::track::Track {
        crate::track::Track {
            path: format!("{}.mp3", name).into(),
            title: name.into(),
            artist: "X".into(),
            duration: std::time::Duration::new(60, 0),
            played_duration: None,
            has_intro: false,
        }
    }

    #[test]
    fn copy_tracks_returns_clones() {
        let mut engine = Engine::new();
        engine.create_playlist("Src".to_string());
        let pl = engine.find_playlist_mut("Src").unwrap();
        pl.tracks.push(make_track("A"));
        pl.tracks.push(make_track("B"));
        pl.tracks.push(make_track("C"));

        let copied = engine.copy_tracks("Src", &[0, 2]).unwrap();
        assert_eq!(copied.len(), 2);
        assert_eq!(copied[0].title, "A");
        assert_eq!(copied[1].title, "C");
    }

    #[test]
    fn copy_tracks_bad_name_errors() {
        let engine = Engine::new();
        assert!(engine.copy_tracks("Ghost", &[0]).is_err());
    }

    #[test]
    fn copy_tracks_bad_index_errors() {
        let mut engine = Engine::new();
        engine.create_playlist("Src".to_string());
        let pl = engine.find_playlist_mut("Src").unwrap();
        pl.tracks.push(make_track("A"));

        assert!(engine.copy_tracks("Src", &[5]).is_err());
    }

    #[test]
    fn paste_tracks_appends() {
        let mut engine = Engine::new();
        engine.create_playlist("Dest".to_string());
        let pl = engine.find_playlist_mut("Dest").unwrap();
        pl.tracks.push(make_track("A"));

        engine
            .paste_tracks("Dest", vec![make_track("B")], None)
            .unwrap();
        assert_eq!(engine.find_playlist("Dest").unwrap().track_count(), 2);
        assert_eq!(engine.find_playlist("Dest").unwrap().tracks[1].title, "B");
    }

    #[test]
    fn paste_tracks_at_position() {
        let mut engine = Engine::new();
        engine.create_playlist("Dest".to_string());
        let pl = engine.find_playlist_mut("Dest").unwrap();
        pl.tracks.push(make_track("A"));
        pl.tracks.push(make_track("C"));

        engine
            .paste_tracks("Dest", vec![make_track("B")], Some(1))
            .unwrap();
        let dest = engine.find_playlist("Dest").unwrap();
        assert_eq!(dest.track_count(), 3);
        assert_eq!(dest.tracks[1].title, "B");
        assert_eq!(dest.tracks[2].title, "C");
    }

    #[test]
    fn paste_tracks_bad_name_errors() {
        let mut engine = Engine::new();
        assert!(
            engine
                .paste_tracks("Ghost", vec![make_track("A")], None)
                .is_err()
        );
    }

    #[test]
    fn copy_paste_across_playlists() {
        let mut engine = Engine::new();
        engine.create_playlist("Src".to_string());
        engine.create_playlist("Dest".to_string());
        let pl = engine.find_playlist_mut("Src").unwrap();
        pl.tracks.push(make_track("A"));
        pl.tracks.push(make_track("B"));

        let copied = engine.copy_tracks("Src", &[0, 1]).unwrap();
        engine.paste_tracks("Dest", copied, None).unwrap();

        assert_eq!(engine.find_playlist("Dest").unwrap().track_count(), 2);
        // Source unchanged
        assert_eq!(engine.find_playlist("Src").unwrap().track_count(), 2);
    }

    #[test]
    fn insert_next_track_at_beginning_when_no_current() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        engine.set_active("Main").unwrap();
        let pl = engine.active_playlist_mut().unwrap();
        pl.tracks.push(make_track("A"));
        pl.tracks.push(make_track("B"));
        // current_index is None, so insert at position 0
        let pl = engine.active_playlist_mut().unwrap();
        pl.current_index = None;
        let tracks_to_insert = vec![make_track("X")];
        pl.insert_tracks(tracks_to_insert, Some(0)).unwrap();
        let pl = engine.active_playlist().unwrap();
        assert_eq!(pl.tracks[0].title, "X");
        assert_eq!(pl.tracks[1].title, "A");
        assert_eq!(pl.tracks[2].title, "B");
    }

    #[test]
    fn insert_next_track_after_current_index() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        engine.set_active("Main").unwrap();
        let pl = engine.active_playlist_mut().unwrap();
        pl.tracks.push(make_track("A"));
        pl.tracks.push(make_track("B"));
        pl.tracks.push(make_track("C"));
        pl.current_index = Some(1); // currently at "B"
        let tracks_to_insert = vec![make_track("X")];
        pl.insert_tracks(tracks_to_insert, Some(2)).unwrap();
        let pl = engine.active_playlist().unwrap();
        assert_eq!(pl.tracks[0].title, "A");
        assert_eq!(pl.tracks[1].title, "B");
        assert_eq!(pl.tracks[2].title, "X");
        assert_eq!(pl.tracks[3].title, "C");
    }

    #[test]
    fn insert_next_track_no_active_playlist_errors() {
        let mut engine = Engine::new();
        // No active playlist set
        let result = engine.active_playlist_mut();
        assert!(result.is_none());
    }

    #[test]
    fn recurring_intro_defaults() {
        let engine = Engine::new();
        assert_eq!(engine.recurring_intro_interval_secs, 0.0);
        assert_eq!(engine.recurring_intro_duck_volume, 0.3);
    }

    #[test]
    fn recurring_intro_survives_serialization() {
        let mut engine = Engine::new();
        engine.recurring_intro_interval_secs = 900.0;
        engine.recurring_intro_duck_volume = 0.2;
        let json = serde_json::to_string(&engine).unwrap();
        let loaded: Engine = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.recurring_intro_interval_secs, 900.0);
        assert_eq!(loaded.recurring_intro_duck_volume, 0.2);
    }

    #[test]
    fn recurring_intro_defaults_when_missing_from_json() {
        let json = r#"{"playlists":[],"active_playlist_id":null,"next_id":1}"#;
        let engine: Engine = serde_json::from_str(json).unwrap();
        assert_eq!(engine.recurring_intro_interval_secs, 0.0);
        assert_eq!(engine.recurring_intro_duck_volume, 0.3);
    }

    #[test]
    fn intros_folder_defaults_to_none() {
        let engine = Engine::new();
        assert!(engine.intros_folder.is_none());
    }

    #[test]
    fn intros_folder_survives_serialization() {
        let mut engine = Engine::new();
        engine.intros_folder = Some("C:\\intros".to_string());
        let json = serde_json::to_string(&engine).unwrap();
        let loaded: Engine = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.intros_folder, Some("C:\\intros".to_string()));
    }

    #[test]
    fn intros_folder_defaults_when_missing_from_json() {
        let json = r#"{"playlists":[],"active_playlist_id":null,"next_id":1}"#;
        let engine: Engine = serde_json::from_str(json).unwrap();
        assert!(engine.intros_folder.is_none());
    }

    #[test]
    fn conflict_policy_defaults_to_schedule_wins() {
        let engine = Engine::new();
        assert_eq!(engine.conflict_policy, ConflictPolicy::ScheduleWins);
    }

    #[test]
    fn conflict_policy_survives_serialization() {
        let mut engine = Engine::new();
        engine.conflict_policy = ConflictPolicy::ManualWins;
        let json = serde_json::to_string(&engine).unwrap();
        let loaded: Engine = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.conflict_policy, ConflictPolicy::ManualWins);
    }

    #[test]
    fn conflict_policy_defaults_when_missing_from_json() {
        let json = r#"{"playlists":[],"active_playlist_id":null,"next_id":1}"#;
        let engine: Engine = serde_json::from_str(json).unwrap();
        assert_eq!(engine.conflict_policy, ConflictPolicy::ScheduleWins);
    }

    #[test]
    fn edit_track_metadata_bad_playlist_errors() {
        let mut engine = Engine::new();
        let result = engine.edit_track_metadata("Ghost", 0, Some("Artist"), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn edit_track_metadata_bad_index_errors() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        let pl = engine.find_playlist_mut("Main").unwrap();
        pl.tracks.push(make_track("A"));
        let result = engine.edit_track_metadata("Main", 5, Some("Artist"), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of range"));
    }

    #[test]
    fn edit_track_metadata_no_changes_errors() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        let pl = engine.find_playlist_mut("Main").unwrap();
        pl.tracks.push(make_track("A"));
        let result = engine.edit_track_metadata("Main", 0, None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Nothing to edit"));
    }

    #[test]
    fn active_playlist_mut_allows_modification() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        engine.set_active("Main").unwrap();
        let active = engine.active_playlist_mut().unwrap();
        active.tracks.push(crate::track::Track {
            path: "test.mp3".into(),
            title: "Test".into(),
            artist: "Artist".into(),
            duration: std::time::Duration::new(60, 0),
            played_duration: None,
            has_intro: false,
        });
        assert_eq!(engine.active_playlist().unwrap().track_count(), 1);
    }

    #[test]
    fn rds_config_defaults() {
        let engine = Engine::new();
        assert_eq!(engine.rds.ip, "127.0.0.1");
        assert_eq!(engine.rds.port, 10001);
        assert!(engine.rds.messages.is_empty());
    }

    #[test]
    fn rds_config_survives_serialization() {
        let mut engine = Engine::new();
        engine.rds.ip = "10.0.0.1".to_string();
        engine.rds.port = 5000;
        engine
            .rds
            .messages
            .push(crate::rds::RdsMessage::new("Test"));
        let json = serde_json::to_string(&engine).unwrap();
        let loaded: Engine = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.rds.ip, "10.0.0.1");
        assert_eq!(loaded.rds.port, 5000);
        assert_eq!(loaded.rds.messages.len(), 1);
    }

    #[test]
    fn rds_config_defaults_when_missing_from_json() {
        let json = r#"{"playlists":[],"active_playlist_id":null,"next_id":1}"#;
        let engine: Engine = serde_json::from_str(json).unwrap();
        assert_eq!(engine.rds.ip, "127.0.0.1");
        assert_eq!(engine.rds.port, 10001);
        assert!(engine.rds.messages.is_empty());
    }
}
