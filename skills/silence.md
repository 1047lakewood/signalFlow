# Silence Detection — Design Doc (DONE)

> Maintenance note (2026-02-13): treat this as a living design record. Verify behavior against current code and tests before implementation decisions.

## Goal
Auto-skip tracks when audio signal drops below a configurable threshold for a configurable duration. Prevents dead air in radio automation.

## Architecture

### SilenceMonitor (shared state)
- `Arc<AtomicBool>` — `silence_detected` flag
- Created fresh per track, checked by playback loop

### SilenceDetector<S: Source> (source wrapper)
- Wraps any `Source<Item = f32>` — rodio's `Decoder` outputs f32
- Passes all samples through unchanged (transparent to audio output)
- Measures RMS amplitude over a rolling window (e.g., 100ms chunks)
- Tracks how long continuous silence has lasted
- Sets `silence_detected` flag when silence exceeds configured duration
- Resets silence timer when signal rises above threshold

### Config (Engine fields)
- `silence_threshold: f32` — RMS threshold (default 0.01, ~-40dB)
- `silence_duration_secs: f32` — seconds of continuous silence before skip (default 0, disabled)
- Both `#[serde(default)]` for backward compatibility

### Playback integration
- `play_playlist()` gains `silence_threshold` and `silence_duration_secs` params
- Each track's source gets wrapped in `SilenceDetector` when silence detection is enabled
- Poll loop checks `SilenceMonitor::is_silent()` alongside `sink.empty()`
- On silence detection: print message, stop current sink, advance to next track

### CLI
- `config silence set <threshold> <duration>` — enable with params
- `config silence off` — disable (sets duration to 0)
- `config show` — includes silence settings
- `play --silence-threshold <f32>` / `--silence-duration <f32>` — per-session override
- `status` — shows silence config

## RMS Calculation
- Window: sample_rate * channels * 0.1 samples (~100ms)
- RMS = sqrt(sum_of_squares / window_size)
- Compare RMS against threshold each window
- Track consecutive silent windows to measure silence duration

## Edge Cases
- Track shorter than silence duration: plays normally, no false trigger
- Silence at very start of track: still triggers after configured duration
- Crossfade overlap: silence detector only monitors the current track's source
- Disabled by default (silence_duration_secs = 0)
