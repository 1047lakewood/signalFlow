use crate::track::Track;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub tracks: Vec<Track>,
    pub current_index: Option<usize>,
}

impl Playlist {
    pub fn new(id: u32, name: String) -> Self {
        Playlist {
            id,
            name,
            source_path: None,
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

    /// Insert tracks at a specific position, or append if `at` is None.
    pub fn insert_tracks(&mut self, tracks: Vec<Track>, at: Option<usize>) -> Result<(), String> {
        match at {
            Some(pos) => {
                if pos > self.tracks.len() {
                    return Err(format!(
                        "Insert position {} out of range (playlist has {} tracks)",
                        pos,
                        self.tracks.len()
                    ));
                }
                for (i, track) in tracks.into_iter().enumerate() {
                    self.tracks.insert(pos + i, track);
                }
            }
            None => {
                self.tracks.extend(tracks);
            }
        }
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
    fn insert_tracks_appends_when_no_position() {
        let mut pl = Playlist::new(1, "Test".to_string());
        pl.tracks.push(make_track("A"));
        pl.insert_tracks(vec![make_track("B"), make_track("C")], None)
            .unwrap();
        assert_eq!(pl.track_count(), 3);
        assert_eq!(pl.tracks[1].title, "B");
        assert_eq!(pl.tracks[2].title, "C");
    }

    #[test]
    fn insert_tracks_at_beginning() {
        let mut pl = Playlist::new(1, "Test".to_string());
        pl.tracks.push(make_track("A"));
        pl.insert_tracks(vec![make_track("B"), make_track("C")], Some(0))
            .unwrap();
        assert_eq!(pl.track_count(), 3);
        assert_eq!(pl.tracks[0].title, "B");
        assert_eq!(pl.tracks[1].title, "C");
        assert_eq!(pl.tracks[2].title, "A");
    }

    #[test]
    fn insert_tracks_at_middle() {
        let mut pl = Playlist::new(1, "Test".to_string());
        pl.tracks.push(make_track("A"));
        pl.tracks.push(make_track("D"));
        pl.insert_tracks(vec![make_track("B"), make_track("C")], Some(1))
            .unwrap();
        assert_eq!(pl.track_count(), 4);
        assert_eq!(pl.tracks[0].title, "A");
        assert_eq!(pl.tracks[1].title, "B");
        assert_eq!(pl.tracks[2].title, "C");
        assert_eq!(pl.tracks[3].title, "D");
    }

    #[test]
    fn insert_tracks_at_end() {
        let mut pl = Playlist::new(1, "Test".to_string());
        pl.tracks.push(make_track("A"));
        pl.insert_tracks(vec![make_track("B")], Some(1)).unwrap();
        assert_eq!(pl.track_count(), 2);
        assert_eq!(pl.tracks[1].title, "B");
    }

    #[test]
    fn insert_tracks_out_of_range_errors() {
        let mut pl = Playlist::new(1, "Test".to_string());
        let result = pl.insert_tracks(vec![make_track("A")], Some(5));
        assert!(result.is_err());
    }

    #[test]
    fn remove_adjusts_current_index() {
        let mut pl = Playlist::new(1, "Test".to_string());
        // Manually add tracks for unit testing without real files
        pl.tracks.push(make_track("A"));
        pl.tracks.push(make_track("B"));
        pl.tracks.push(make_track("C"));
        pl.current_index = Some(2);

        // Remove track before current -> index shifts down
        pl.remove_track(0).unwrap();
        assert_eq!(pl.current_index, Some(1));
    }
}
