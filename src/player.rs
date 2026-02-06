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

/// Play through a playlist starting at `start_index`, auto-advancing.
/// Supports crossfading when `crossfade_secs > 0.0`.
/// Supports silence detection when `silence.enabled()`.
/// Blocks until all tracks finish or the process is interrupted.
/// Returns the index of the last track that was started.
pub fn play_playlist(
    player: &Player,
    tracks: &[crate::track::Track],
    start_index: usize,
    crossfade_secs: f32,
    silence: SilenceConfig,
) -> usize {
    let crossfade_dur = Duration::from_secs_f32(crossfade_secs.max(0.0));
    let mut current = start_index;
    let mut current_sink: Option<Sink> = None;
    let mut current_monitor: Option<SilenceMonitor> = None;

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

        // Start playback if not already playing via crossfade
        let (sink, monitor) = if let Some(s) = current_sink.take() {
            (s, current_monitor.take())
        } else {
            match start_track(player, &track.path, &silence) {
                Ok(pair) => pair,
                Err(e) => {
                    eprintln!("  Error: {} — skipping", e);
                    current += 1;
                    continue;
                }
            }
        };

        let start_time = Instant::now();
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
                std::thread::sleep(Duration::from_millis(50));
            }

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
                        current += 1;
                        continue;
                    }
                    Err(e) => {
                        eprintln!("  Crossfade error: {} — playing sequentially", e);
                    }
                }
            }
        }

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
                std::thread::sleep(Duration::from_millis(100));
            }
        }

        current += 1;
    }

    println!("Playlist finished.");
    current.saturating_sub(1)
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
