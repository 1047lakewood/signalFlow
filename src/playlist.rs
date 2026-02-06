use crate::track::Track;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: u32,
    pub name: String,
    pub tracks: Vec<Track>,
    pub current_index: Option<usize>,
}

impl Playlist {
    pub fn new(id: u32, name: String) -> Self {
        Playlist {
            id,
            name,
            tracks: Vec::new(),
            current_index: None,
        }
    }

    /// Add a track by parsing metadata from a file path.
    pub fn add_track(&mut self, path: &Path) -> Result<usize, String> {
        let track = Track::from_path(path)?;
        self.tracks.push(track);
        Ok(self.tracks.len() - 1)
    }

    /// Remove a track by index. Returns the removed track.
    pub fn remove_track(&mut self, index: usize) -> Result<Track, String> {
        if index >= self.tracks.len() {
            return Err(format!(
                "Index {} out of range (playlist has {} tracks)",
                index,
                self.tracks.len()
            ));
        }
        let track = self.tracks.remove(index);
        // Adjust current_index if needed
        if let Some(ci) = self.current_index {
            if index < ci {
                self.current_index = Some(ci - 1);
            } else if index == ci {
                self.current_index = None;
            }
        }
        Ok(track)
    }

    /// Move a track from one position to another.
    pub fn reorder(&mut self, from: usize, to: usize) -> Result<(), String> {
        if from >= self.tracks.len() || to >= self.tracks.len() {
            return Err(format!(
                "Index out of range (playlist has {} tracks)",
                self.tracks.len()
            ));
        }
        let track = self.tracks.remove(from);
        self.tracks.insert(to, track);
        Ok(())
    }

    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_playlist_is_empty() {
        let pl = Playlist::new(1, "Test".to_string());
        assert_eq!(pl.track_count(), 0);
        assert!(pl.current_index.is_none());
    }

    #[test]
    fn remove_adjusts_current_index() {
        let mut pl = Playlist::new(1, "Test".to_string());
        // Manually add tracks for unit testing without real files
        pl.tracks.push(crate::track::Track {
            path: "a.mp3".into(),
            title: "A".into(),
            artist: "X".into(),
            duration: std::time::Duration::new(60, 0),
        });
        pl.tracks.push(crate::track::Track {
            path: "b.mp3".into(),
            title: "B".into(),
            artist: "X".into(),
            duration: std::time::Duration::new(60, 0),
        });
        pl.tracks.push(crate::track::Track {
            path: "c.mp3".into(),
            title: "C".into(),
            artist: "X".into(),
            duration: std::time::Duration::new(60, 0),
        });
        pl.current_index = Some(2);

        // Remove track before current -> index shifts down
        pl.remove_track(0).unwrap();
        assert_eq!(pl.current_index, Some(1));
    }
}
