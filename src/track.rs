use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
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
}

impl Track {
    /// Create a Track by reading metadata from an audio file.
    pub fn from_path(path: &Path) -> Result<Self, String> {
        let path = path
            .canonicalize()
            .map_err(|e| format!("Invalid path '{}': {}", path.display(), e))?;

        let tagged_file = lofty::read_from_path(&path)
            .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;

        let properties = tagged_file.properties();
        let duration = properties.duration();

        let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

        let title = tag
            .and_then(|t| t.title().map(|s| s.to_string()))
            .unwrap_or_else(|| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown".to_string())
            });

        let artist = tag
            .and_then(|t| t.artist().map(|s| s.to_string()))
            .unwrap_or_else(|| "Unknown".to_string());

        Ok(Track {
            path,
            title,
            artist,
            duration,
        })
    }

    /// Format duration as MM:SS.
    pub fn duration_display(&self) -> String {
        let secs = self.duration.as_secs();
        format!("{}:{:02}", secs / 60, secs % 60)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_display_formats_correctly() {
        let track = Track {
            path: PathBuf::from("test.mp3"),
            title: "Test".to_string(),
            artist: "Artist".to_string(),
            duration: Duration::new(185, 0), // 3:05
        };
        assert_eq!(track.duration_display(), "3:05");
    }

    #[test]
    fn from_path_rejects_missing_file() {
        let result = Track::from_path(Path::new("nonexistent.mp3"));
        assert!(result.is_err());
    }
}
