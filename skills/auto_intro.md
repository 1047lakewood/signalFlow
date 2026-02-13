# Auto-Intro System — Design Doc (DONE, incl. Recurring Overlay + GUI Config)

> Maintenance note (2026-02-13): treat this as a living design record. Verify behavior against current code and tests before implementation decisions.

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
- GUI displays blue dot (●) on tracks with `has_intro: true` in the status column

## Playback Integration (DONE)
- `play_playlist()` accepts `intros_folder: Option<&Path>`
- Before each track, call `find_intro()` for the track's artist
- If intro found: play intro on its own sink, wait for it to finish
- If not found: play track normally
- Consecutive tracks by the same artist: only play intro before the first one in the run
- Note: crossfade-into-song (fade intro tail into track start) is not yet implemented

## Recurring Intro Overlay (DONE)
- While a track plays, re-play its artist intro every N seconds as an overlay
- Main track volume is ducked (lowered) during the overlay, then restored
- Timer resets when a new track starts — each track gets its own cycle
- Only applies to the currently playing track; skips if no intro found
- Config: `Engine.recurring_intro_interval_secs: f32` (0 = disabled, default 0)
- Config: `Engine.recurring_intro_duck_volume: f32` (0.0–1.0, default 0.3)
- Both fields `#[serde(default)]` for backward compat
- `play_playlist()` accepts `RecurringIntroConfig` parameter
- `maybe_play_recurring_intro()` helper checks timing, plays overlay, ducks volume
- Integrated into both crossfade and sequential wait loops
- CLI: `config intros recurring set <interval> [--duck <vol>]`
- CLI: `config intros recurring off`
- IPC: `set_recurring_intro(interval_secs, duck_volume)` Tauri command
- IPC: `get_config` / `get_status` responses include recurring intro fields

## CLI (DONE)
- `config intros set <path>` — set intros folder
- `config intros off` — clear intros folder
- `config intros recurring set <interval> [--duck <vol>]` — enable recurring overlay
- `config intros recurring off` — disable recurring overlay
- `config show` — display intros folder + recurring intro settings
- `play` — uses configured intros_folder and recurring intro automatically
