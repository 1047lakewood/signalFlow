use std::path::{Path, PathBuf};

/// Supported audio extensions for intro files.
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "ogg", "aac", "m4a"];

/// Search for an artist intro file in the given folder.
///
/// Looks for files named `<artist>.<ext>` (case-insensitive) where ext is
/// any supported audio format. Returns the first match found.
pub fn find_intro(intros_folder: &Path, artist: &str) -> Option<PathBuf> {
    if artist.is_empty() || artist == "Unknown" {
        return None;
    }

    let entries = std::fs::read_dir(intros_folder).ok()?;
    let artist_lower = artist.to_lowercase();

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

        if stem == artist_lower && AUDIO_EXTENSIONS.contains(&ext.as_str()) {
            return Some(path);
        }
    }

    None
}

/// Check whether an intro exists for a given artist (without returning the path).
pub fn has_intro(intros_folder: &Path, artist: &str) -> bool {
    find_intro(intros_folder, artist).is_some()
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
    fn find_intro_ignores_partial_matches() {
        let dir = create_temp_intros_dir();
        fs::write(dir.path().join("Adele Remix.mp3"), b"fake").unwrap();

        assert!(find_intro(dir.path(), "Adele").is_none());
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
