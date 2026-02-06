use std::path::{Path, PathBuf};

/// Supported audio extensions for intro files.
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "ogg", "aac", "m4a"];

/// Search for artist intro files in the given folder.
///
/// Matches files whose stem starts with the artist name (case-insensitive),
/// where any remaining characters are non-alphabetic (digits, spaces, punctuation).
/// This allows variants like `Artist.mp3`, `Artist2.mp3`, `Artist 3.mp3`.
/// If multiple matches are found, one is picked at random.
pub fn find_intro(intros_folder: &Path, artist: &str) -> Option<PathBuf> {
    let candidates = find_all_intros(intros_folder, artist);
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() == 1 {
        return Some(candidates.into_iter().next().unwrap());
    }
    let idx = fastrand::usize(..candidates.len());
    Some(candidates.into_iter().nth(idx).unwrap())
}

/// Return all matching intro files for an artist.
pub fn find_all_intros(intros_folder: &Path, artist: &str) -> Vec<PathBuf> {
    if artist.is_empty() || artist == "Unknown" {
        return Vec::new();
    }

    let entries = match std::fs::read_dir(intros_folder) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let artist_lower = artist.to_lowercase();

    let mut matches = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let stem = match path.file_stem() {
            Some(s) => s.to_string_lossy().to_lowercase(),
            None => continue,
        };

        let ext = match path.extension() {
            Some(e) => e.to_string_lossy().to_lowercase(),
            None => continue,
        };

        if !AUDIO_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        if stem == artist_lower {
            matches.push(path);
        } else if stem.starts_with(&artist_lower) {
            // Allow suffix that contains no alphabetic characters (digits, spaces, punctuation)
            let suffix = &stem[artist_lower.len()..];
            if !suffix.chars().any(|c| c.is_alphabetic()) {
                matches.push(path);
            }
        }
    }

    matches
}

/// Check whether an intro exists for a given artist (without returning the path).
pub fn has_intro(intros_folder: &Path, artist: &str) -> bool {
    !find_all_intros(intros_folder, artist).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_temp_intros_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn find_intro_returns_none_for_empty_folder() {
        let dir = create_temp_intros_dir();
        assert!(find_intro(dir.path(), "Adele").is_none());
    }

    #[test]
    fn find_intro_returns_none_for_unknown_artist() {
        let dir = create_temp_intros_dir();
        assert!(find_intro(dir.path(), "Unknown").is_none());
    }

    #[test]
    fn find_intro_returns_none_for_empty_artist() {
        let dir = create_temp_intros_dir();
        assert!(find_intro(dir.path(), "").is_none());
    }

    #[test]
    fn find_intro_matches_case_insensitive() {
        let dir = create_temp_intros_dir();
        let intro_path = dir.path().join("adele.mp3");
        fs::write(&intro_path, b"fake audio").unwrap();

        let result = find_intro(dir.path(), "Adele");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), intro_path);
    }

    #[test]
    fn find_intro_matches_different_extensions() {
        let dir = create_temp_intros_dir();
        let intro_path = dir.path().join("Beatles.wav");
        fs::write(&intro_path, b"fake audio").unwrap();

        assert!(find_intro(dir.path(), "Beatles").is_some());
    }

    #[test]
    fn find_intro_ignores_non_audio_extensions() {
        let dir = create_temp_intros_dir();
        fs::write(dir.path().join("Adele.txt"), b"not audio").unwrap();
        fs::write(dir.path().join("Adele.jpg"), b"not audio").unwrap();

        assert!(find_intro(dir.path(), "Adele").is_none());
    }

    #[test]
    fn find_intro_ignores_alphabetic_suffix() {
        let dir = create_temp_intros_dir();
        fs::write(dir.path().join("Adele Remix.mp3"), b"fake").unwrap();

        assert!(find_intro(dir.path(), "Adele").is_none());
    }

    #[test]
    fn find_intro_matches_numeric_suffix() {
        let dir = create_temp_intros_dir();
        fs::write(dir.path().join("Adele2.mp3"), b"fake").unwrap();
        fs::write(dir.path().join("Adele 3.mp3"), b"fake").unwrap();

        let all = find_all_intros(dir.path(), "Adele");
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn find_all_intros_collects_all_variants() {
        let dir = create_temp_intros_dir();
        fs::write(dir.path().join("R Avigor Miller.mp3"), b"fake").unwrap();
        fs::write(dir.path().join("R Avigor Miller2.mp3"), b"fake").unwrap();
        fs::write(dir.path().join("R Avigor Miller 3.mp3"), b"fake").unwrap();

        let all = find_all_intros(dir.path(), "R Avigor Miller");
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn find_intro_picks_from_multiple() {
        let dir = create_temp_intros_dir();
        fs::write(dir.path().join("Adele.mp3"), b"fake").unwrap();
        fs::write(dir.path().join("Adele2.mp3"), b"fake").unwrap();

        // Run multiple times — should always return a valid path
        for _ in 0..10 {
            let result = find_intro(dir.path(), "Adele");
            assert!(result.is_some());
            let path = result.unwrap();
            let stem = path.file_stem().unwrap().to_string_lossy().to_lowercase();
            assert!(stem.starts_with("adele"));
        }
    }

    #[test]
    fn has_intro_returns_bool() {
        let dir = create_temp_intros_dir();
        fs::write(dir.path().join("Adele.mp3"), b"fake").unwrap();

        assert!(has_intro(dir.path(), "Adele"));
        assert!(!has_intro(dir.path(), "Nobody"));
    }

    #[test]
    fn find_intro_nonexistent_folder() {
        assert!(find_intro(Path::new("Z:\\nonexistent_folder_xyz"), "Adele").is_none());
    }
}
