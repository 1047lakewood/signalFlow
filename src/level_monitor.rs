use rodio::Source;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Shared atomic storing the current audio RMS level as f32 bits.
/// Updated by `LevelSource` on the audio thread, read by IPC on the main thread.
#[derive(Clone)]
pub struct LevelMonitor {
    level: Arc<AtomicU32>,
}

impl LevelMonitor {
    pub fn new() -> Self {
        LevelMonitor {
            level: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Get the current RMS level (0.0–1.0+).
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    fn set_level(&self, rms: f32) {
        self.level.store(rms.to_bits(), Ordering::Relaxed);
    }

    /// Reset the level to zero (e.g. on stop).
    pub fn reset(&self) {
        self.set_level(0.0);
    }
}

/// A Source wrapper that measures RMS amplitude and updates a `LevelMonitor`.
/// Passes all samples through unchanged.
pub struct LevelSource<S> {
    inner: S,
    monitor: LevelMonitor,
    window_size: usize,
    window_sum_sq: f64,
    window_pos: usize,
}

impl<S> LevelSource<S>
where
    S: Source<Item = f32>,
{
    /// Wrap a source with level monitoring.
    /// RMS is computed over ~50ms windows and stored in `monitor`.
    pub fn new(source: S, monitor: LevelMonitor) -> Self {
        let sample_rate = source.sample_rate() as usize;
        let channels = source.channels() as usize;
        let samples_per_sec = sample_rate * channels;
        // ~50ms analysis window for responsive metering
        let window_size = (samples_per_sec as f64 * 0.05).max(1.0) as usize;

        LevelSource {
            inner: source,
            monitor,
            window_size,
            window_sum_sq: 0.0,
            window_pos: 0,
        }
    }
}

impl<S> Iterator for LevelSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;

        self.window_sum_sq += (sample as f64) * (sample as f64);
        self.window_pos += 1;

        if self.window_pos >= self.window_size {
            let rms = (self.window_sum_sq / self.window_size as f64).sqrt() as f32;
            self.monitor.set_level(rms);
            self.window_sum_sq = 0.0;
            self.window_pos = 0;
        }

        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<S> Source for LevelSource<S>
where
    S: Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test source that produces a fixed sequence of samples.
    struct TestSource {
        samples: Vec<f32>,
        pos: usize,
        sample_rate: u32,
        channels: u16,
    }

    impl TestSource {
        fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
            TestSource {
                samples,
                pos: 0,
                sample_rate,
                channels,
            }
        }
    }

    impl Iterator for TestSource {
        type Item = f32;
        fn next(&mut self) -> Option<f32> {
            if self.pos < self.samples.len() {
                let s = self.samples[self.pos];
                self.pos += 1;
                Some(s)
            } else {
                None
            }
        }
    }

    impl Source for TestSource {
        fn current_frame_len(&self) -> Option<usize> {
            Some(self.samples.len() - self.pos)
        }
        fn channels(&self) -> u16 {
            self.channels
        }
        fn sample_rate(&self) -> u32 {
            self.sample_rate
        }
        fn total_duration(&self) -> Option<Duration> {
            None
        }
    }

    #[test]
    fn monitor_starts_at_zero() {
        let monitor = LevelMonitor::new();
        assert_eq!(monitor.level(), 0.0);
    }

    #[test]
    fn monitor_reset_sets_zero() {
        let monitor = LevelMonitor::new();
        monitor.set_level(0.5);
        assert!(monitor.level() > 0.0);
        monitor.reset();
        assert_eq!(monitor.level(), 0.0);
    }

    #[test]
    fn level_source_passes_samples_unchanged() {
        let original = vec![0.1, 0.2, -0.3, 0.4, 0.5];
        let source = TestSource::new(original.clone(), 1000, 1);
        let monitor = LevelMonitor::new();
        let wrapped = LevelSource::new(source, monitor);
        let output: Vec<f32> = wrapped.collect();
        assert_eq!(output, original);
    }

    #[test]
    fn level_source_measures_loud_audio() {
        // 1000 Hz, 1 channel. Window = ~50ms = 50 samples.
        // Provide 100 samples of 0.5 amplitude. After first window (50 samples),
        // RMS should be ~0.5.
        let source = TestSource::new(vec![0.5; 100], 1000, 1);
        let monitor = LevelMonitor::new();
        let wrapped = LevelSource::new(source, monitor.clone());
        let _: Vec<f32> = wrapped.collect();
        let level = monitor.level();
        assert!(level > 0.4, "Expected RMS ~0.5, got {}", level);
        assert!(level < 0.6, "Expected RMS ~0.5, got {}", level);
    }

    #[test]
    fn level_source_measures_silence() {
        let source = TestSource::new(vec![0.0; 100], 1000, 1);
        let monitor = LevelMonitor::new();
        let wrapped = LevelSource::new(source, monitor.clone());
        let _: Vec<f32> = wrapped.collect();
        assert_eq!(monitor.level(), 0.0);
    }

    #[test]
    fn level_source_preserves_source_properties() {
        let source = TestSource::new(vec![0.0; 50], 44100, 2);
        let monitor = LevelMonitor::new();
        let wrapped = LevelSource::new(source, monitor);
        assert_eq!(wrapped.sample_rate(), 44100);
        assert_eq!(wrapped.channels(), 2);
    }
}
