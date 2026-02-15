use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::prelude::TagExt;
use lofty::tag::{Accessor, Tag};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    #[serde(with = "duration_serde")]
    pub duration: Duration,
    /// Actual playback time (set after track finishes playing).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "option_duration_serde")]
    pub played_duration: Option<Duration>,
    /// Whether an intro file exists for this track's artist.
    #[serde(default)]
    pub has_intro: bool,
}

impl Track {
    /// Create a Track by reading metadata from an audio file.
    pub fn from_path(path: &Path) -> Result<Self, String> {
        // Avoid canonicalize — it resolves mapped drives to UNC paths on Windows
        // (e.g. G:\Music → \\NAS\share\Music), losing the drive letter the user expects.
        let path = std::path::absolute(path)
            .map_err(|e| format!("Invalid path '{}': {}", path.display(), e))?;

        let tagged_file = lofty::read_from_path(&path)
            .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;

        let properties = tagged_file.properties();
        let duration = properties.duration();

        let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

        let tag_title = tag.and_then(|t| t.title().map(|s| s.to_string()));
        let tag_artist = tag.and_then(|t| t.artist().map(|s| s.to_string()));

        let (title, artist) = match (tag_title, tag_artist) {
            (Some(t), Some(a)) => (t, a),
            (Some(t), None) => (t, filename_fallback_artist(&path)),
            (None, Some(a)) => (filename_fallback_title(&path), a),
            (None, None) => {
                let (a, t) = parse_filename(&path);
                (t, a)
            }
        };

        Ok(Track {
            path,
            title,
            artist,
            duration,
            played_duration: None,
            has_intro: false,
        })
    }

    /// Format calculated duration as MM:SS.
    pub fn duration_display(&self) -> String {
        format_duration(self.duration)
    }

    /// Format played duration as MM:SS, if available.
    pub fn played_duration_display(&self) -> Option<String> {
        self.played_duration.map(format_duration)
    }

    /// Edit track metadata (artist and/or title) and persist changes to the audio file's tags.
    /// Updates the in-memory fields and writes the new values to the file's embedded tags via lofty.
    pub fn write_tags(
        &mut self,
        new_artist: Option<&str>,
        new_title: Option<&str>,
    ) -> Result<(), String> {
        if new_artist.is_none() && new_title.is_none() {
            return Err("Nothing to edit: provide --artist and/or --title".to_string());
        }

        let mut tagged_file = lofty::read_from_path(&self.path)
            .map_err(|e| format!("Failed to read '{}': {}", self.path.display(), e))?;

        let tag = match tagged_file.primary_tag_mut() {
            Some(t) => t,
            None => match tagged_file.first_tag_mut() {
                Some(t) => t,
                None => {
                    let tag_type = tagged_file.primary_tag_type();
                    tagged_file.insert_tag(Tag::new(tag_type));
                    tagged_file.primary_tag_mut().unwrap()
                }
            },
        };

        if let Some(artist) = new_artist {
            tag.set_artist(artist.to_string());
            self.artist = artist.to_string();
        }

        if let Some(title) = new_title {
            tag.set_title(title.to_string());
            self.title = title.to_string();
        }

        tag.save_to_path(&self.path, WriteOptions::default())
            .map_err(|e| format!("Failed to write tags to '{}': {}", self.path.display(), e))?;

        Ok(())
    }
}

/// Format a Duration as M:SS.
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Parse "Artist - Title" from a filename. Falls back gracefully.
fn parse_filename(path: &Path) -> (String, String) {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    if let Some((artist, title)) = stem.split_once(" - ") {
        let artist = artist.trim();
        let title = title.trim();
        if !artist.is_empty() && !title.is_empty() {
            return (artist.to_string(), title.to_string());
        }
    }

    ("Unknown".to_string(), stem)
}

/// Extract artist from filename when tag is missing.
fn filename_fallback_artist(path: &Path) -> String {
    let (artist, _) = parse_filename(path);
    artist
}

/// Extract title from filename when tag is missing.
fn filename_fallback_title(path: &Path) -> String {
    let (_, title) = parse_filename(path);
    title
}

mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    #[derive(Serialize, Deserialize)]
    struct DurationRepr {
        secs: u64,
        nanos: u32,
    }

    pub fn serialize<S: Serializer>(dur: &Duration, s: S) -> Result<S::Ok, S::Error> {
        DurationRepr {
            secs: dur.as_secs(),
            nanos: dur.subsec_nanos(),
        }
        .serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let repr = DurationRepr::deserialize(d)?;
        Ok(Duration::new(repr.secs, repr.nanos))
    }
}

mod option_duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    #[derive(Serialize, Deserialize)]
    struct DurationRepr {
        secs: u64,
        nanos: u32,
    }

    pub fn serialize<S: Serializer>(dur: &Option<Duration>, s: S) -> Result<S::Ok, S::Error> {
        match dur {
            Some(d) => DurationRepr {
                secs: d.as_secs(),
                nanos: d.subsec_nanos(),
            }
            .serialize(s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Duration>, D::Error> {
        let opt = Option::<DurationRepr>::deserialize(d)?;
        Ok(opt.map(|repr| Duration::new(repr.secs, repr.nanos)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_track(title: &str, artist: &str) -> Track {
        Track {
            path: PathBuf::from("test.mp3"),
            title: title.to_string(),
            artist: artist.to_string(),
            duration: Duration::new(60, 0),
            played_duration: None,
            has_intro: false,
        }
    }

    #[test]
    fn duration_display_formats_correctly() {
        let track = Track {
            path: PathBuf::from("test.mp3"),
            title: "Test".to_string(),
            artist: "Artist".to_string(),
            duration: Duration::new(185, 0), // 3:05
            played_duration: None,
            has_intro: false,
        };
        assert_eq!(track.duration_display(), "3:05");
    }

    #[test]
    fn played_duration_display_none_when_unset() {
        let track = make_track("Test", "Artist");
        assert!(track.played_duration_display().is_none());
    }

    #[test]
    fn played_duration_display_shows_value() {
        let mut track = make_track("Test", "Artist");
        track.played_duration = Some(Duration::new(45, 0));
        assert_eq!(track.played_duration_display(), Some("0:45".to_string()));
    }

    #[test]
    fn from_path_rejects_missing_file() {
        let result = Track::from_path(Path::new("nonexistent.mp3"));
        assert!(result.is_err());
    }

    #[test]
    fn parse_filename_artist_dash_title() {
        let (artist, title) = parse_filename(Path::new("Cool Band - Great Song.mp3"));
        assert_eq!(artist, "Cool Band");
        assert_eq!(title, "Great Song");
    }

    #[test]
    fn parse_filename_no_dash() {
        let (artist, title) = parse_filename(Path::new("just_a_filename.mp3"));
        assert_eq!(artist, "Unknown");
        assert_eq!(title, "just_a_filename");
    }

    #[test]
    fn parse_filename_empty_parts() {
        let (artist, title) = parse_filename(Path::new(" - .mp3"));
        assert_eq!(artist, "Unknown");
        assert_eq!(title, " - ");
    }

    #[test]
    fn played_duration_survives_serialization() {
        let mut track = make_track("Test", "Artist");
        track.played_duration = Some(Duration::new(120, 500_000_000));
        let json = serde_json::to_string(&track).unwrap();
        let loaded: Track = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.played_duration, Some(Duration::new(120, 500_000_000)));
    }

    #[test]
    fn has_intro_defaults_to_false() {
        let track = make_track("Test", "Artist");
        assert!(!track.has_intro);
    }

    #[test]
    fn has_intro_survives_serialization() {
        let mut track = make_track("Test", "Artist");
        track.has_intro = true;
        let json = serde_json::to_string(&track).unwrap();
        let loaded: Track = serde_json::from_str(&json).unwrap();
        assert!(loaded.has_intro);
    }

    #[test]
    fn has_intro_defaults_when_missing_from_json() {
        let json = r#"{"path":"test.mp3","title":"T","artist":"A","duration":{"secs":60,"nanos":0}}"#;
        let track: Track = serde_json::from_str(json).unwrap();
        assert!(!track.has_intro);
    }

    #[test]
    fn played_duration_defaults_when_missing() {
        let json = r#"{"path":"test.mp3","title":"T","artist":"A","duration":{"secs":60,"nanos":0}}"#;
        let track: Track = serde_json::from_str(json).unwrap();
        assert!(track.played_duration.is_none());
    }

    #[test]
    fn write_tags_rejects_no_changes() {
        let mut track = make_track("Test", "Artist");
        let result = track.write_tags(None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Nothing to edit"));
    }

    #[test]
    fn write_tags_rejects_missing_file() {
        let mut track = make_track("Test", "Artist");
        track.path = PathBuf::from("nonexistent_file.mp3");
        let result = track.write_tags(Some("New Artist"), None);
        assert!(result.is_err());
    }

    #[test]
    fn write_tags_updates_in_memory_fields_on_missing_file() {
        // When file doesn't exist, write_tags errors before updating fields
        let mut track = make_track("Old Title", "Old Artist");
        track.path = PathBuf::from("nonexistent.mp3");
        let _ = track.write_tags(Some("New Artist"), Some("New Title"));
        // Fields should NOT have changed since the file read failed before we got to set them
        assert_eq!(track.artist, "Old Artist");
        assert_eq!(track.title, "Old Title");
    }
}
