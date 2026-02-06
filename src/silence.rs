use rodio::Source;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Shared flag indicating whether silence has been detected.
#[derive(Clone)]
pub struct SilenceMonitor {
    detected: Arc<AtomicBool>,
}

impl SilenceMonitor {
    pub fn new() -> Self {
        SilenceMonitor {
            detected: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns true if silence has been detected.
    pub fn is_silent(&self) -> bool {
        self.detected.load(Ordering::Relaxed)
    }

    fn set_silent(&self) {
        self.detected.store(true, Ordering::Relaxed);
    }
}

/// A Source wrapper that monitors audio levels and flags silence.
/// Passes all samples through unchanged.
pub struct SilenceDetector<S> {
    inner: S,
    monitor: SilenceMonitor,
    threshold: f32,
    silence_samples_needed: u64,
    silent_samples: u64,
    window_size: usize,
    window_sum_sq: f64,
    window_pos: usize,
}

impl<S> SilenceDetector<S>
where
    S: Source<Item = f32>,
{
    /// Wrap a source with silence detection.
    ///
    /// - `threshold`: RMS level below which audio is considered silent (e.g., 0.01)
    /// - `silence_duration`: how long silence must persist before flagging
    /// - `monitor`: shared flag that gets set when silence is detected
    pub fn new(
        source: S,
        threshold: f32,
        silence_duration: Duration,
        monitor: SilenceMonitor,
    ) -> Self {
        let sample_rate = source.sample_rate() as u64;
        let channels = source.channels() as u64;
        let samples_per_sec = sample_rate * channels;
        let silence_samples_needed = (silence_duration.as_secs_f64() * samples_per_sec as f64) as u64;
        // ~100ms analysis window
        let window_size = (samples_per_sec as f64 * 0.1).max(1.0) as usize;

        SilenceDetector {
            inner: source,
            monitor,
            threshold,
            silence_samples_needed,
            silent_samples: 0,
            window_size,
            window_sum_sq: 0.0,
            window_pos: 0,
        }
    }
}

impl<S> Iterator for SilenceDetector<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;

        // Accumulate for RMS
        self.window_sum_sq += (sample as f64) * (sample as f64);
        self.window_pos += 1;

        if self.window_pos >= self.window_size {
            let rms = (self.window_sum_sq / self.window_size as f64).sqrt() as f32;
            if rms < self.threshold {
                self.silent_samples += self.window_size as u64;
            } else {
                self.silent_samples = 0;
            }

            if self.silent_samples >= self.silence_samples_needed && self.silence_samples_needed > 0
            {
                self.monitor.set_silent();
            }

            self.window_sum_sq = 0.0;
            self.window_pos = 0;
        }

        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<S> Source for SilenceDetector<S>
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

        fn silent(num_samples: usize, sample_rate: u32) -> Self {
            TestSource::new(vec![0.0; num_samples], sample_rate, 1)
        }

        fn loud_then_silent(loud_samples: usize, silent_samples: usize, sample_rate: u32) -> Self {
            let mut samples = vec![0.5; loud_samples];
            samples.extend(vec![0.0; silent_samples]);
            TestSource::new(samples, sample_rate, 1)
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
    fn monitor_starts_not_silent() {
        let monitor = SilenceMonitor::new();
        assert!(!monitor.is_silent());
    }

    #[test]
    fn detects_silence_in_silent_source() {
        // 1000 Hz sample rate, 1 channel. Silence duration = 0.5s = 500 samples.
        // Provide 1000 silent samples (1 second), threshold 0.01, duration 0.5s.
        let source = TestSource::silent(1000, 1000);
        let monitor = SilenceMonitor::new();
        let detector = SilenceDetector::new(
            source,
            0.01,
            Duration::from_millis(500),
            monitor.clone(),
        );

        // Consume all samples
        let collected: Vec<f32> = detector.collect();
        assert_eq!(collected.len(), 1000);
        assert!(monitor.is_silent(), "Should detect silence after 1s of silence with 0.5s threshold");
    }

    #[test]
    fn does_not_flag_loud_source() {
        // Source with all loud samples
        let source = TestSource::new(vec![0.5; 1000], 1000, 1);
        let monitor = SilenceMonitor::new();
        let detector = SilenceDetector::new(
            source,
            0.01,
            Duration::from_millis(500),
            monitor.clone(),
        );

        let _: Vec<f32> = detector.collect();
        assert!(!monitor.is_silent(), "Should not flag loud audio as silent");
    }

    #[test]
    fn resets_on_loud_after_brief_silence() {
        // 500 silent samples, then 500 loud samples (at 1000Hz, 1ch)
        // Silence duration threshold = 0.8s = 800 samples
        // The 500 silent samples are only 0.5s, so shouldn't trigger
        let mut samples = vec![0.0; 500];
        samples.extend(vec![0.5; 500]);
        let source = TestSource::new(samples, 1000, 1);
        let monitor = SilenceMonitor::new();
        let detector = SilenceDetector::new(
            source,
            0.01,
            Duration::from_millis(800),
            monitor.clone(),
        );

        let _: Vec<f32> = detector.collect();
        assert!(!monitor.is_silent(), "Brief silence followed by audio should not trigger");
    }

    #[test]
    fn passes_through_samples_unchanged() {
        let original = vec![0.1, 0.2, -0.3, 0.4, 0.5];
        let source = TestSource::new(original.clone(), 1000, 1);
        let monitor = SilenceMonitor::new();
        let detector = SilenceDetector::new(
            source,
            0.01,
            Duration::from_secs(1),
            monitor,
        );

        let output: Vec<f32> = detector.collect();
        assert_eq!(output, original);
    }

    #[test]
    fn disabled_when_duration_zero() {
        let source = TestSource::silent(1000, 1000);
        let monitor = SilenceMonitor::new();
        let detector = SilenceDetector::new(
            source,
            0.01,
            Duration::ZERO,
            monitor.clone(),
        );

        let _: Vec<f32> = detector.collect();
        assert!(!monitor.is_silent(), "Should not trigger when duration is zero (disabled)");
    }

    #[test]
    fn detects_silence_after_loud_section() {
        // 200 loud samples, then 800 silent samples at 1000Hz
        // Silence threshold = 0.5s = 500 samples
        let source = TestSource::loud_then_silent(200, 800, 1000);
        let monitor = SilenceMonitor::new();
        let detector = SilenceDetector::new(
            source,
            0.01,
            Duration::from_millis(500),
            monitor.clone(),
        );

        let _: Vec<f32> = detector.collect();
        assert!(monitor.is_silent(), "Should detect silence in the trailing silent section");
    }
}
