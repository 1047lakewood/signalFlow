use crate::level_monitor::{LevelMonitor, LevelSource};
use crate::silence::{SilenceDetector, SilenceMonitor};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::{Duration, Instant};

/// Runtime audio player wrapping rodio. Not serializable — created fresh per session.
pub struct Player {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    sink: Sink,
}

impl Player {
    /// Initialize audio output and create a playback sink.
    pub fn new() -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| format!("Failed to open audio output: {}", e))?;
        let sink = Sink::try_new(&handle)
            .map_err(|e| format!("Failed to create audio sink: {}", e))?;
        Ok(Player {
            _stream: stream,
            stream_handle: handle,
            sink,
        })
    }

    /// Create a new independent sink on the same audio output.
    pub fn create_sink(&self) -> Result<Sink, String> {
        Sink::try_new(&self.stream_handle)
            .map_err(|e| format!("Failed to create sink: {}", e))
    }

    /// Decode and append an audio file to the default sink, starting playback.
    pub fn play_file(&self, path: &Path) -> Result<(), String> {
        let file = File::open(path)
            .map_err(|e| format!("Cannot open '{}': {}", path.display(), e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Cannot decode '{}': {}", path.display(), e))?;
        self.sink.append(source);
        self.sink.play();
        Ok(())
    }

    /// Decode and append an audio file to the default sink with level monitoring.
    /// The `LevelMonitor` is updated with the current RMS level as audio plays.
    pub fn play_file_with_level(&self, path: &Path, monitor: LevelMonitor) -> Result<(), String> {
        let file = File::open(path)
            .map_err(|e| format!("Cannot open '{}': {}", path.display(), e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Cannot decode '{}': {}", path.display(), e))?;
        let wrapped = LevelSource::new(source.convert_samples::<f32>(), monitor);
        self.sink.append(wrapped);
        self.sink.play();
        Ok(())
    }

    /// Play an audio file on a new sink, returning ownership of that sink.
    pub fn play_file_new_sink(&self, path: &Path) -> Result<Sink, String> {
        let sink = self.create_sink()?;
        let file = File::open(path)
            .map_err(|e| format!("Cannot open '{}': {}", path.display(), e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Cannot decode '{}': {}", path.display(), e))?;
        sink.append(source);
        sink.play();
        Ok(sink)
    }

    /// Play an audio file on a new sink with a fade-in applied.
    pub fn play_file_new_sink_fadein(
        &self,
        path: &Path,
        fade: Duration,
    ) -> Result<Sink, String> {
        let sink = self.create_sink()?;
        let file = File::open(path)
            .map_err(|e| format!("Cannot open '{}': {}", path.display(), e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Cannot decode '{}': {}", path.display(), e))?;
        sink.append(source.fade_in(fade));
        sink.play();
        Ok(sink)
    }

    /// Play an audio file on a new sink with silence detection, returning the sink and monitor.
    pub fn play_file_new_sink_monitored(
        &self,
        path: &Path,
        threshold: f32,
        silence_duration: Duration,
    ) -> Result<(Sink, SilenceMonitor), String> {
        let sink = self.create_sink()?;
        let file = File::open(path)
            .map_err(|e| format!("Cannot open '{}': {}", path.display(), e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Cannot decode '{}': {}", path.display(), e))?;
        let monitor = SilenceMonitor::new();
        let wrapped = SilenceDetector::new(
            source.convert_samples::<f32>(),
            threshold,
            silence_duration,
            monitor.clone(),
        );
        sink.append(wrapped);
        sink.play();
        Ok((sink, monitor))
    }

    /// Play an audio file on a new sink with fade-in and silence detection.
    pub fn play_file_new_sink_fadein_monitored(
        &self,
        path: &Path,
        fade: Duration,
        threshold: f32,
        silence_duration: Duration,
    ) -> Result<(Sink, SilenceMonitor), String> {
        let sink = self.create_sink()?;
        let file = File::open(path)
            .map_err(|e| format!("Cannot open '{}': {}", path.display(), e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Cannot decode '{}': {}", path.display(), e))?;
        let monitor = SilenceMonitor::new();
        let wrapped = SilenceDetector::new(
            source.convert_samples::<f32>(),
            threshold,
            silence_duration,
            monitor.clone(),
        );
        sink.append(wrapped.fade_in(fade));
        sink.play();
        Ok((sink, monitor))
    }

    /// Stop playback and clear the sink.
    pub fn stop(&self) {
        self.sink.stop();
    }

    /// Pause playback (can be resumed).
    pub fn pause(&self) {
        self.sink.pause();
    }

    /// Resume paused playback.
    pub fn resume(&self) {
        self.sink.play();
    }

    /// True when the sink has finished all queued audio.
    pub fn is_empty(&self) -> bool {
        self.sink.empty()
    }

    /// Skip the currently playing source.
    pub fn skip_one(&self) {
        self.sink.skip_one();
    }

    /// Attempt to seek to a position in the current source.
    pub fn try_seek(&self, position: Duration) -> Result<(), String> {
        self.sink
            .try_seek(position)
            .map_err(|e| format!("Seek failed: {}", e))
    }

    /// Returns true if the sink is paused.
    pub fn is_paused(&self) -> bool {
        self.sink.is_paused()
    }

    /// Play an audio file as an overlay on top of current audio.
    /// Creates a new independent sink and blocks until playback finishes.
    pub fn play_overlay(&self, path: &Path) -> Result<(), String> {
        let sink = self.play_file_new_sink(path)?;
        while !sink.empty() {
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    /// Stop current audio, then play a file on a new sink (hard break).
    /// Stops the default sink first, plays the file, and blocks until finished.
    pub fn play_stop_mode(&self, path: &Path) -> Result<(), String> {
        self.sink.stop();
        let sink = self.play_file_new_sink(path)?;
        while !sink.empty() {
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }
}

/// Returns true if crossfading should occur for this track transition.
pub fn should_crossfade(crossfade_secs: f32, track_duration: Duration, has_next: bool) -> bool {
    crossfade_secs > 0.0
        && has_next
        && track_duration > Duration::from_secs_f32(crossfade_secs * 2.0)
}

/// Perform a linear fade-out on a sink over the given duration.
/// Blocks the calling thread for the fade duration.
fn fade_out_sink(sink: &Sink, duration: Duration) {
    let fade_secs = duration.as_secs_f32();
    let steps = (fade_secs * 20.0).max(1.0) as usize; // ~50ms per step
    let step_duration = duration / steps as u32;

    for step in 1..=steps {
        let volume = 1.0 - (step as f32 / steps as f32);
        sink.set_volume(volume);
        std::thread::sleep(step_duration);
    }
    sink.set_volume(0.0);
}

/// Configuration for silence detection during playback.
#[derive(Clone, Copy)]
pub struct SilenceConfig {
    pub threshold: f32,
    pub duration_secs: f32,
}

impl SilenceConfig {
    /// Returns true if silence detection is enabled.
    pub fn enabled(&self) -> bool {
        self.duration_secs > 0.0 && self.threshold > 0.0
    }

    /// Silence duration as a `Duration`.
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f32(self.duration_secs.max(0.0))
    }

    /// Disabled silence config.
    pub fn disabled() -> Self {
        SilenceConfig {
            threshold: 0.0,
            duration_secs: 0.0,
        }
    }
}

/// Configuration for recurring intro overlays during playback.
#[derive(Clone, Copy)]
pub struct RecurringIntroConfig {
    /// Interval in seconds between recurring intro overlays (0 = disabled).
    pub interval_secs: f32,
    /// Volume level to duck the main track to during overlay (0.0–1.0).
    pub duck_volume: f32,
}

impl RecurringIntroConfig {
    /// Returns true if recurring intros are enabled.
    pub fn enabled(&self) -> bool {
        self.interval_secs > 0.0
    }

    /// Interval as a `Duration`.
    pub fn interval(&self) -> Duration {
        Duration::from_secs_f32(self.interval_secs.max(0.0))
    }

    /// Disabled recurring intro config.
    pub fn disabled() -> Self {
        RecurringIntroConfig {
            interval_secs: 0.0,
            duck_volume: 0.3,
        }
    }
}

/// Result of playing through a playlist.
pub struct PlaybackResult {
    /// Index of the last track that was started.
    pub last_index: usize,
    /// Played durations for each track that was played: (track_index, duration).
    pub played_durations: Vec<(usize, Duration)>,
}

/// Play through a playlist starting at `start_index`, auto-advancing.
/// Supports crossfading when `crossfade_secs > 0.0`.
/// Supports silence detection when `silence.enabled()`.
/// Supports auto-intros when `intros_folder` is provided.
/// Blocks until all tracks finish or the process is interrupted.
/// Returns a `PlaybackResult` with the last index and per-track played durations.
pub fn play_playlist(
    player: &Player,
    tracks: &[crate::track::Track],
    start_index: usize,
    crossfade_secs: f32,
    silence: SilenceConfig,
    intros_folder: Option<&Path>,
    recurring_intro: RecurringIntroConfig,
) -> PlaybackResult {
    let crossfade_dur = Duration::from_secs_f32(crossfade_secs.max(0.0));
    let mut current = start_index;
    let mut current_sink: Option<Sink> = None;
    let mut current_monitor: Option<SilenceMonitor> = None;
    let mut current_start_time: Option<Instant> = None;
    let mut played_durations: Vec<(usize, Duration)> = Vec::new();
    let mut last_intro_artist: Option<String> = None;
    let mut last_recurring_intro_time: Option<Instant>;

    while current < tracks.len() {
        let track = &tracks[current];
        println!(
            "Now playing [{}/{}]: {} — {} [{}]",
            current + 1,
            tracks.len(),
            track.artist,
            track.title,
            track.duration_display()
        );

        // Play artist intro if configured and not a consecutive same-artist track
        if current_sink.is_none() {
            if let Some(intros_dir) = intros_folder {
                let same_artist = last_intro_artist
                    .as_ref()
                    .map_or(false, |a| a.eq_ignore_ascii_case(&track.artist));
                if !same_artist {
                    if let Some(intro_path) =
                        crate::auto_intro::find_intro(intros_dir, &track.artist)
                    {
                        println!("  Playing intro for {}...", track.artist);
                        match player.play_file_new_sink(&intro_path) {
                            Ok(intro_sink) => {
                                while !intro_sink.empty() {
                                    std::thread::sleep(Duration::from_millis(100));
                                }
                                last_intro_artist = Some(track.artist.clone());
                            }
                            Err(e) => {
                                eprintln!("  Intro error: {} — skipping intro", e);
                            }
                        }
                    }
                }
            }
        }

        // Start playback if not already playing via crossfade
        let (sink, monitor, start_time) = if let Some(s) = current_sink.take() {
            (s, current_monitor.take(), current_start_time.take().unwrap_or_else(Instant::now))
        } else {
            match start_track(player, &track.path, &silence) {
                Ok(pair) => (pair.0, pair.1, Instant::now()),
                Err(e) => {
                    eprintln!("  Error: {} — skipping", e);
                    current += 1;
                    continue;
                }
            }
        };

        // Reset recurring intro timer for each new track
        last_recurring_intro_time = if recurring_intro.enabled() && intros_folder.is_some() {
            Some(Instant::now())
        } else {
            None
        };

        let track_duration = track.duration;
        let next_index = current + 1;
        let do_crossfade =
            should_crossfade(crossfade_secs, track_duration, next_index < tracks.len());

        let mut silence_skipped = false;

        if do_crossfade {
            let crossfade_start = track_duration - crossfade_dur;

            // Wait until crossfade point, track ends, or silence detected
            loop {
                if start_time.elapsed() >= crossfade_start || sink.empty() {
                    break;
                }
                if check_silence(&monitor) {
                    println!("  Silence detected — skipping to next track");
                    sink.stop();
                    silence_skipped = true;
                    break;
                }
                // Check for recurring intro overlay
                maybe_play_recurring_intro(
                    player,
                    &sink,
                    &track.artist,
                    intros_folder,
                    &recurring_intro,
                    &mut last_recurring_intro_time,
                );
                std::thread::sleep(Duration::from_millis(50));
            }

            // Record played duration for this track
            played_durations.push((current, start_time.elapsed()));

            if !silence_skipped && !sink.empty() {
                let next_track = &tracks[next_index];
                let crossfade_result = if silence.enabled() {
                    player
                        .play_file_new_sink_fadein_monitored(
                            &next_track.path,
                            crossfade_dur,
                            silence.threshold,
                            silence.duration(),
                        )
                        .map(|(s, m)| (s, Some(m)))
                } else {
                    player
                        .play_file_new_sink_fadein(&next_track.path, crossfade_dur)
                        .map(|s| (s, None))
                };

                match crossfade_result {
                    Ok((next_sink, next_monitor)) => {
                        fade_out_sink(&sink, crossfade_dur);
                        sink.stop();
                        current_sink = Some(next_sink);
                        current_monitor = next_monitor;
                        current_start_time = Some(Instant::now());
                        current += 1;
                        continue;
                    }
                    Err(e) => {
                        eprintln!("  Crossfade error: {} — playing sequentially", e);
                    }
                }
            }
        } else {
            // Wait for track to finish (if not already silence-skipped)
            if !silence_skipped {
                loop {
                    if sink.empty() {
                        break;
                    }
                    if check_silence(&monitor) {
                        println!("  Silence detected — skipping to next track");
                        sink.stop();
                        break;
                    }
                    // Check for recurring intro overlay
                    maybe_play_recurring_intro(
                        player,
                        &sink,
                        &track.artist,
                        intros_folder,
                        &recurring_intro,
                        &mut last_recurring_intro_time,
                    );
                    std::thread::sleep(Duration::from_millis(100));
                }
            }

            // Record played duration for this track
            played_durations.push((current, start_time.elapsed()));
        }

        current += 1;
    }

    println!("Playlist finished.");
    PlaybackResult {
        last_index: current.saturating_sub(1),
        played_durations,
    }
}

/// Check if it's time to play a recurring intro overlay, and play it if so.
/// Ducks the main sink volume during the intro, then restores it.
fn maybe_play_recurring_intro(
    player: &Player,
    main_sink: &Sink,
    artist: &str,
    intros_folder: Option<&Path>,
    config: &RecurringIntroConfig,
    last_time: &mut Option<Instant>,
) {
    if !config.enabled() {
        return;
    }
    let intros_dir = match intros_folder {
        Some(d) => d,
        None => return,
    };
    let last = match last_time {
        Some(t) => *t,
        None => return,
    };
    if last.elapsed() < config.interval() {
        return;
    }

    // Time to play a recurring intro overlay
    if let Some(intro_path) = crate::auto_intro::find_intro(intros_dir, artist) {
        println!("  Recurring intro overlay for {}...", artist);
        match player.play_file_new_sink(&intro_path) {
            Ok(overlay_sink) => {
                // Duck main track volume
                let original_volume = 1.0_f32;
                main_sink.set_volume(config.duck_volume);

                // Wait for overlay to finish
                while !overlay_sink.empty() {
                    std::thread::sleep(Duration::from_millis(50));
                }

                // Restore main track volume
                main_sink.set_volume(original_volume);
            }
            Err(e) => {
                eprintln!("  Recurring intro error: {} — skipping", e);
            }
        }
    }

    // Reset timer regardless of whether intro was found/played
    *last_time = Some(Instant::now());
}

/// Start a track, optionally with silence monitoring.
fn start_track(
    player: &Player,
    path: &Path,
    silence: &SilenceConfig,
) -> Result<(Sink, Option<SilenceMonitor>), String> {
    if silence.enabled() {
        player
            .play_file_new_sink_monitored(path, silence.threshold, silence.duration())
            .map(|(s, m)| (s, Some(m)))
    } else {
        player.play_file_new_sink(path).map(|s| (s, None))
    }
}

/// Check if a silence monitor has triggered.
fn check_silence(monitor: &Option<SilenceMonitor>) -> bool {
    monitor.as_ref().map_or(false, |m| m.is_silent())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_creation_succeeds_or_fails_gracefully() {
        let result = Player::new();
        match result {
            Ok(p) => {
                assert!(p.is_empty());
                assert!(!p.is_paused());
            }
            Err(e) => {
                assert!(e.contains("Failed to open audio output"));
            }
        }
    }

    #[test]
    fn play_file_rejects_missing_file() {
        if let Ok(player) = Player::new() {
            let result = player.play_file(Path::new("nonexistent_audio.mp3"));
            assert!(result.is_err());
        }
    }

    #[test]
    fn create_sink_works() {
        if let Ok(player) = Player::new() {
            let sink = player.create_sink();
            assert!(sink.is_ok());
            if let Ok(s) = sink {
                assert!(s.empty());
            }
        }
    }

    #[test]
    fn silence_config_enabled() {
        let cfg = SilenceConfig { threshold: 0.01, duration_secs: 3.0 };
        assert!(cfg.enabled());
        assert_eq!(cfg.duration(), Duration::from_secs(3));
    }

    #[test]
    fn silence_config_disabled_when_zero_duration() {
        let cfg = SilenceConfig { threshold: 0.01, duration_secs: 0.0 };
        assert!(!cfg.enabled());
    }

    #[test]
    fn silence_config_disabled_when_zero_threshold() {
        let cfg = SilenceConfig { threshold: 0.0, duration_secs: 3.0 };
        assert!(!cfg.enabled());
    }

    #[test]
    fn silence_config_disabled_constructor() {
        let cfg = SilenceConfig::disabled();
        assert!(!cfg.enabled());
    }

    #[test]
    fn play_overlay_rejects_missing_file() {
        if let Ok(player) = Player::new() {
            let result = player.play_overlay(Path::new("nonexistent_overlay.mp3"));
            assert!(result.is_err());
        }
    }

    #[test]
    fn play_stop_mode_rejects_missing_file() {
        if let Ok(player) = Player::new() {
            let result = player.play_stop_mode(Path::new("nonexistent_stop.mp3"));
            assert!(result.is_err());
        }
    }

    #[test]
    fn recurring_intro_config_enabled() {
        let cfg = RecurringIntroConfig { interval_secs: 900.0, duck_volume: 0.3 };
        assert!(cfg.enabled());
        assert_eq!(cfg.interval(), Duration::from_secs(900));
    }

    #[test]
    fn recurring_intro_config_disabled_when_zero() {
        let cfg = RecurringIntroConfig { interval_secs: 0.0, duck_volume: 0.3 };
        assert!(!cfg.enabled());
    }

    #[test]
    fn recurring_intro_config_disabled_constructor() {
        let cfg = RecurringIntroConfig::disabled();
        assert!(!cfg.enabled());
        assert_eq!(cfg.duck_volume, 0.3);
    }

    #[test]
    fn should_crossfade_basic_cases() {
        // Disabled when crossfade_secs is 0
        assert!(!should_crossfade(0.0, Duration::from_secs(300), true));

        // Disabled when no next track
        assert!(!should_crossfade(3.0, Duration::from_secs(300), false));

        // Disabled when track too short (must be > 2x crossfade)
        assert!(!should_crossfade(3.0, Duration::from_secs(5), true));

        // Enabled for normal case
        assert!(should_crossfade(3.0, Duration::from_secs(300), true));

        // Edge: track exactly 2x crossfade — too short
        assert!(!should_crossfade(3.0, Duration::from_secs(6), true));

        // Edge: track slightly longer than 2x crossfade
        assert!(should_crossfade(3.0, Duration::from_secs(7), true));
    }
}
