# Auto-Intro System — Design Doc (DONE)

## Purpose
Automatically play an artist intro/jingle before tracks by a matching artist. Common in radio automation — e.g., before "Adele - Hello.mp3", play "Adele.mp3" from the intros folder.

## Config (DONE)
- `Engine.intros_folder: Option<String>` — path to folder containing intro files
- Persisted in state JSON, `#[serde(default)]`
- CLI: `config intros set <path>` / `config intros off`

## Matching Logic — `src/auto_intro.rs` (DONE)
- `find_intro(intros_folder: &Path, artist: &str) -> Option<PathBuf>`
- Case-insensitive filename match: looks for `Artist.*` (any audio extension)
- Supported extensions: `.mp3`, `.wav`, `.flac`, `.ogg`, `.aac`, `.m4a`
- Returns first match found

## Track Flag (DONE)
- `Track.has_intro: bool` — set dynamically when intros_folder is configured
- `#[serde(default)]` for backward compat
- Used by future GUI for dot indicator

## Playback Integration (DONE)
- `play_playlist()` accepts `intros_folder: Option<&Path>`
- Before each track, call `find_intro()` for the track's artist
- If intro found: play intro on its own sink, wait for it to finish
- If not found: play track normally
- Consecutive tracks by the same artist: only play intro before the first one in the run
- Note: crossfade-into-song (fade intro tail into track start) is not yet implemented

## CLI (DONE)
- `config intros set <path>` — set intros folder
- `config intros off` — clear intros folder
- `config show` — display intros folder setting
- `play` — uses configured intros_folder automatically
