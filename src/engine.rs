use crate::playlist::Playlist;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const STATE_FILE: &str = "signalflow_state.json";

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
        }
    }

    /// Load engine state from JSON, or create a new instance if not found.
    pub fn load() -> Self {
        let path = Path::new(STATE_FILE);
        if path.exists() {
            match fs::read_to_string(path) {
                Ok(data) => match serde_json::from_str(&data) {
                    Ok(engine) => return engine,
                    Err(e) => eprintln!("Warning: corrupt state file, starting fresh: {}", e),
                },
                Err(e) => eprintln!("Warning: could not read state file: {}", e),
            }
        }
        Engine::new()
    }

    /// Persist current state to JSON.
    pub fn save(&self) -> Result<(), String> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("Serialize error: {}", e))?;
        fs::write(STATE_FILE, json).map_err(|e| format!("Write error: {}", e))?;
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
    pub fn copy_tracks(&self, from_name: &str, indices: &[usize]) -> Result<Vec<crate::track::Track>, String> {
        let pl = self
            .find_playlist(from_name)
            .ok_or_else(|| format!("Playlist '{}' not found", from_name))?;
        let mut tracks = Vec::new();
        for &idx in indices {
            if idx >= pl.tracks.len() {
                return Err(format!(
                    "Index {} out of range (playlist '{}' has {} tracks)",
                    idx, from_name, pl.tracks.len()
                ));
            }
            tracks.push(pl.tracks[idx].clone());
        }
        Ok(tracks)
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

    fn make_track(name: &str) -> crate::track::Track {
        crate::track::Track {
            path: format!("{}.mp3", name).into(),
            title: name.into(),
            artist: "X".into(),
            duration: std::time::Duration::new(60, 0),
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
        assert!(engine.paste_tracks("Ghost", vec![make_track("A")], None).is_err());
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
        });
        assert_eq!(engine.active_playlist().unwrap().track_count(), 1);
    }
}
