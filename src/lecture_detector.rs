use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Classifies tracks as "lecture" or "music" based on artist name rules.
///
/// Classification priority: Blacklist > Whitelist > starts-with-'R'.
/// - Blacklist: never a lecture, even if starts with 'R'.
/// - Whitelist: always a lecture.
/// - Starts with 'R': treated as lecture by default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LectureDetector {
    /// Lowercase artist names that are NEVER lectures.
    #[serde(default)]
    pub blacklist: HashSet<String>,
    /// Lowercase artist names that are ALWAYS lectures.
    #[serde(default)]
    pub whitelist: HashSet<String>,
}

impl Default for LectureDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl LectureDetector {
    pub fn new() -> Self {
        LectureDetector {
            blacklist: HashSet::new(),
            whitelist: HashSet::new(),
        }
    }

    /// Classify whether an artist name represents a lecture.
    ///
    /// Rules applied in order:
    /// 1. Empty artist -> false
    /// 2. Blacklisted (case-insensitive) -> false
    /// 3. Whitelisted (case-insensitive) -> true
    /// 4. Starts with 'R' or 'r' -> true
    /// 5. Otherwise -> false
    pub fn is_lecture(&self, artist: &str) -> bool {
        if artist.is_empty() {
            return false;
        }
        let lower = artist.to_lowercase();
        if self.blacklist.contains(&lower) {
            return false;
        }
        if self.whitelist.contains(&lower) {
            return true;
        }
        lower.starts_with('r')
    }

    /// Add an artist to the blacklist (stored lowercase).
    pub fn add_blacklist(&mut self, artist: &str) {
        self.blacklist.insert(artist.to_lowercase());
    }

    /// Remove an artist from the blacklist.
    pub fn remove_blacklist(&mut self, artist: &str) -> bool {
        self.blacklist.remove(&artist.to_lowercase())
    }

    /// Add an artist to the whitelist (stored lowercase).
    pub fn add_whitelist(&mut self, artist: &str) {
        self.whitelist.insert(artist.to_lowercase());
    }

    /// Remove an artist from the whitelist.
    pub fn remove_whitelist(&mut self, artist: &str) -> bool {
        self.whitelist.remove(&artist.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_artist_is_not_lecture() {
        let ld = LectureDetector::new();
        assert!(!ld.is_lecture(""));
    }

    #[test]
    fn starts_with_r_is_lecture() {
        let ld = LectureDetector::new();
        assert!(ld.is_lecture("Rabbi Shalom"));
        assert!(ld.is_lecture("rav moshe"));
        assert!(ld.is_lecture("Rambam"));
    }

    #[test]
    fn does_not_start_with_r_is_not_lecture() {
        let ld = LectureDetector::new();
        assert!(!ld.is_lecture("The Beatles"));
        assert!(!ld.is_lecture("Adele"));
        assert!(!ld.is_lecture("Mozart"));
    }

    #[test]
    fn blacklist_overrides_starts_with_r() {
        let mut ld = LectureDetector::new();
        ld.add_blacklist("Rihanna");
        assert!(!ld.is_lecture("Rihanna"));
        assert!(!ld.is_lecture("rihanna"));
        assert!(!ld.is_lecture("RIHANNA"));
    }

    #[test]
    fn whitelist_forces_lecture() {
        let mut ld = LectureDetector::new();
        ld.add_whitelist("Special Speaker");
        assert!(ld.is_lecture("Special Speaker"));
        assert!(ld.is_lecture("special speaker"));
    }

    #[test]
    fn blacklist_overrides_whitelist() {
        let mut ld = LectureDetector::new();
        ld.add_whitelist("Rihanna");
        ld.add_blacklist("Rihanna");
        // Blacklist wins
        assert!(!ld.is_lecture("Rihanna"));
    }

    #[test]
    fn add_remove_blacklist() {
        let mut ld = LectureDetector::new();
        ld.add_blacklist("Rihanna");
        assert!(ld.blacklist.contains("rihanna"));
        assert!(ld.remove_blacklist("RIHANNA"));
        assert!(!ld.blacklist.contains("rihanna"));
    }

    #[test]
    fn add_remove_whitelist() {
        let mut ld = LectureDetector::new();
        ld.add_whitelist("Speaker One");
        assert!(ld.whitelist.contains("speaker one"));
        assert!(ld.remove_whitelist("Speaker One"));
        assert!(!ld.whitelist.contains("speaker one"));
    }

    #[test]
    fn serialization_roundtrip() {
        let mut ld = LectureDetector::new();
        ld.add_blacklist("Rihanna");
        ld.add_whitelist("Rabbi Moshe");
        let json = serde_json::to_string(&ld).unwrap();
        let loaded: LectureDetector = serde_json::from_str(&json).unwrap();
        assert!(loaded.blacklist.contains("rihanna"));
        assert!(loaded.whitelist.contains("rabbi moshe"));
    }

    #[test]
    fn defaults_when_missing_from_json() {
        let json = "{}";
        let ld: LectureDetector = serde_json::from_str(json).unwrap();
        assert!(ld.blacklist.is_empty());
        assert!(ld.whitelist.is_empty());
    }
}
