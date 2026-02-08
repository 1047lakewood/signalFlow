use crate::engine::Engine;
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Snapshot of current playback state for XML export.
#[derive(Debug, Clone)]
pub struct NowPlaying {
    pub playlist_name: Option<String>,
    pub current_artist: Option<String>,
    pub current_title: Option<String>,
    pub current_duration: Option<Duration>,
    pub current_elapsed: Option<Duration>,
    pub current_remaining: Option<Duration>,
    pub next_artist: Option<String>,
    pub next_title: Option<String>,
    pub next_duration: Option<Duration>,
    pub state: PlaybackState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
}

impl std::fmt::Display for PlaybackState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaybackState::Stopped => write!(f, "stopped"),
            PlaybackState::Playing => write!(f, "playing"),
        }
    }
}

impl NowPlaying {
    /// Build a NowPlaying snapshot from the current engine state.
    /// `elapsed` is optionally provided by the caller (from live playback tracking).
    pub fn from_engine(engine: &Engine, elapsed: Option<Duration>) -> Self {
        let pl = engine.active_playlist();

        let (playlist_name, current_track, next_track, state) = match pl {
            Some(pl) => {
                let name = Some(pl.name.clone());
                match pl.current_index {
                    Some(idx) => {
                        let current = pl.tracks.get(idx);
                        let next = pl.tracks.get(idx + 1);
                        let state = if current.is_some() {
                            PlaybackState::Playing
                        } else {
                            PlaybackState::Stopped
                        };
                        (name, current, next, state)
                    }
                    None => (name, None, pl.tracks.first(), PlaybackState::Stopped),
                }
            }
            None => (None, None, None, PlaybackState::Stopped),
        };

        let (current_artist, current_title, current_duration) = match current_track {
            Some(t) => (
                Some(t.artist.clone()),
                Some(t.title.clone()),
                Some(t.duration),
            ),
            None => (None, None, None),
        };

        let current_elapsed = elapsed;
        let current_remaining = match (current_duration, elapsed) {
            (Some(dur), Some(el)) if dur > el => Some(dur - el),
            (Some(_), Some(_)) => Some(Duration::ZERO),
            _ => None,
        };

        let (next_artist, next_title, next_duration) = match next_track {
            Some(t) => (
                Some(t.artist.clone()),
                Some(t.title.clone()),
                Some(t.duration),
            ),
            None => (None, None, None),
        };

        NowPlaying {
            playlist_name,
            current_artist,
            current_title,
            current_duration,
            current_elapsed,
            current_remaining,
            next_artist,
            next_title,
            next_duration,
            state,
        }
    }

    /// Render this snapshot as an XML string.
    pub fn to_xml(&self) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<nowplaying>\n");

        xml.push_str(&format!(
            "  <state>{}</state>\n",
            self.state
        ));

        xml.push_str(&format!(
            "  <playlist>{}</playlist>\n",
            xml_escape(self.playlist_name.as_deref().unwrap_or(""))
        ));

        xml.push_str("  <current>\n");
        xml.push_str(&format!(
            "    <artist>{}</artist>\n",
            xml_escape(self.current_artist.as_deref().unwrap_or(""))
        ));
        xml.push_str(&format!(
            "    <title>{}</title>\n",
            xml_escape(self.current_title.as_deref().unwrap_or(""))
        ));
        xml.push_str(&format!(
            "    <duration>{}</duration>\n",
            self.current_duration.map(format_secs).unwrap_or_default()
        ));
        xml.push_str(&format!(
            "    <elapsed>{}</elapsed>\n",
            self.current_elapsed.map(format_secs).unwrap_or_default()
        ));
        xml.push_str(&format!(
            "    <remaining>{}</remaining>\n",
            self.current_remaining.map(format_secs).unwrap_or_default()
        ));
        xml.push_str("  </current>\n");

        xml.push_str("  <next>\n");
        xml.push_str(&format!(
            "    <artist>{}</artist>\n",
            xml_escape(self.next_artist.as_deref().unwrap_or(""))
        ));
        xml.push_str(&format!(
            "    <title>{}</title>\n",
            xml_escape(self.next_title.as_deref().unwrap_or(""))
        ));
        xml.push_str(&format!(
            "    <duration>{}</duration>\n",
            self.next_duration.map(format_secs).unwrap_or_default()
        ));
        xml.push_str("  </next>\n");

        xml.push_str("</nowplaying>\n");
        xml
    }

    /// Write the XML snapshot to a file.
    pub fn write_xml(&self, path: &Path) -> Result<(), String> {
        let xml = self.to_xml();
        fs::write(path, &xml).map_err(|e| format!("Failed to write XML to '{}': {}", path.display(), e))
    }
}

/// Format Duration as integer seconds string.
fn format_secs(d: Duration) -> String {
    d.as_secs().to_string()
}

/// Escape XML special characters.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use crate::track::Track;
    use std::path::PathBuf;

    fn make_track(artist: &str, title: &str, secs: u64) -> Track {
        Track {
            path: PathBuf::from("test.mp3"),
            title: title.to_string(),
            artist: artist.to_string(),
            duration: Duration::new(secs, 0),
            played_duration: None,
            has_intro: false,
        }
    }

    #[test]
    fn from_engine_no_active_playlist() {
        let engine = Engine::new();
        let np = NowPlaying::from_engine(&engine, None);
        assert!(np.playlist_name.is_none());
        assert!(np.current_artist.is_none());
        assert!(np.next_artist.is_none());
        assert_eq!(np.state, PlaybackState::Stopped);
    }

    #[test]
    fn from_engine_with_current_track() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        engine.set_active("Main").unwrap();
        let pl = engine.active_playlist_mut().unwrap();
        pl.tracks.push(make_track("Artist A", "Song A", 200));
        pl.tracks.push(make_track("Artist B", "Song B", 180));
        pl.current_index = Some(0);

        let np = NowPlaying::from_engine(&engine, Some(Duration::new(45, 0)));
        assert_eq!(np.playlist_name.as_deref(), Some("Main"));
        assert_eq!(np.current_artist.as_deref(), Some("Artist A"));
        assert_eq!(np.current_title.as_deref(), Some("Song A"));
        assert_eq!(np.current_duration, Some(Duration::new(200, 0)));
        assert_eq!(np.current_elapsed, Some(Duration::new(45, 0)));
        assert_eq!(np.current_remaining, Some(Duration::new(155, 0)));
        assert_eq!(np.next_artist.as_deref(), Some("Artist B"));
        assert_eq!(np.next_title.as_deref(), Some("Song B"));
        assert_eq!(np.state, PlaybackState::Playing);
    }

    #[test]
    fn from_engine_at_last_track_no_next() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        engine.set_active("Main").unwrap();
        let pl = engine.active_playlist_mut().unwrap();
        pl.tracks.push(make_track("Solo", "Only Song", 120));
        pl.current_index = Some(0);

        let np = NowPlaying::from_engine(&engine, None);
        assert_eq!(np.current_artist.as_deref(), Some("Solo"));
        assert!(np.next_artist.is_none());
    }

    #[test]
    fn from_engine_no_current_index_stopped() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        engine.set_active("Main").unwrap();
        let pl = engine.active_playlist_mut().unwrap();
        pl.tracks.push(make_track("A", "B", 60));
        // current_index is None

        let np = NowPlaying::from_engine(&engine, None);
        assert_eq!(np.state, PlaybackState::Stopped);
        assert!(np.current_artist.is_none());
        // next should be the first track
        assert_eq!(np.next_artist.as_deref(), Some("A"));
    }

    #[test]
    fn remaining_clamps_to_zero() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        engine.set_active("Main").unwrap();
        let pl = engine.active_playlist_mut().unwrap();
        pl.tracks.push(make_track("A", "B", 60));
        pl.current_index = Some(0);

        // elapsed exceeds duration
        let np = NowPlaying::from_engine(&engine, Some(Duration::new(999, 0)));
        assert_eq!(np.current_remaining, Some(Duration::ZERO));
    }

    #[test]
    fn to_xml_contains_expected_elements() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        engine.set_active("Main").unwrap();
        let pl = engine.active_playlist_mut().unwrap();
        pl.tracks.push(make_track("Test Artist", "Test Song", 180));
        pl.tracks.push(make_track("Next Artist", "Next Song", 200));
        pl.current_index = Some(0);

        let np = NowPlaying::from_engine(&engine, Some(Duration::new(30, 0)));
        let xml = np.to_xml();
        assert!(xml.contains("<state>playing</state>"));
        assert!(xml.contains("<playlist>Main</playlist>"));
        assert!(xml.contains("<artist>Test Artist</artist>"));
        assert!(xml.contains("<title>Test Song</title>"));
        assert!(xml.contains("<duration>180</duration>"));
        assert!(xml.contains("<elapsed>30</elapsed>"));
        assert!(xml.contains("<remaining>150</remaining>"));
        assert!(xml.contains("<artist>Next Artist</artist>"));
    }

    #[test]
    fn to_xml_escapes_special_characters() {
        let mut engine = Engine::new();
        engine.create_playlist("Rock & Roll".to_string());
        engine.set_active("Rock & Roll").unwrap();
        let pl = engine.active_playlist_mut().unwrap();
        pl.tracks.push(make_track("AC/DC", "<TNT>", 60));
        pl.current_index = Some(0);

        let np = NowPlaying::from_engine(&engine, None);
        let xml = np.to_xml();
        assert!(xml.contains("Rock &amp; Roll"));
        assert!(xml.contains("&lt;TNT&gt;"));
    }

    #[test]
    fn to_xml_stopped_state() {
        let engine = Engine::new();
        let np = NowPlaying::from_engine(&engine, None);
        let xml = np.to_xml();
        assert!(xml.contains("<state>stopped</state>"));
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    }

    #[test]
    fn write_xml_to_tempfile() {
        let engine = Engine::new();
        let np = NowPlaying::from_engine(&engine, None);
        let dir = std::env::temp_dir();
        let path = dir.join("signalflow_test_nowplaying.xml");
        np.write_xml(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<nowplaying>"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn xml_escape_handles_all_chars() {
        assert_eq!(xml_escape("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&apos;f");
    }

    #[test]
    fn playback_state_display() {
        assert_eq!(format!("{}", PlaybackState::Stopped), "stopped");
        assert_eq!(format!("{}", PlaybackState::Playing), "playing");
    }
}
