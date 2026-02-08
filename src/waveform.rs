use rodio::{Decoder, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Number of amplitude peaks to generate for the waveform overview.
const DEFAULT_NUM_PEAKS: usize = 200;

/// Generate a waveform overview from an audio file.
/// Returns a vector of peak amplitudes (0.0–1.0) representing the audio shape.
/// Each value is the maximum absolute sample value within its time bucket.
pub fn generate_peaks(path: &Path, num_peaks: usize) -> Result<Vec<f32>, String> {
    let file = File::open(path)
        .map_err(|e| format!("Cannot open '{}': {}", path.display(), e))?;
    let source = Decoder::new(BufReader::new(file))
        .map_err(|e| format!("Cannot decode '{}': {}", path.display(), e))?;

    let channels = source.channels() as usize;

    // Collect all samples as f32
    let samples: Vec<f32> = source.convert_samples::<f32>().collect();

    if samples.is_empty() {
        return Ok(vec![0.0; num_peaks]);
    }

    // Total number of frames (one frame = all channels at one time point)
    let total_frames = samples.len() / channels.max(1);
    if total_frames == 0 {
        return Ok(vec![0.0; num_peaks]);
    }

    let frames_per_peak = (total_frames as f64 / num_peaks as f64).max(1.0);
    let mut peaks = Vec::with_capacity(num_peaks);

    for i in 0..num_peaks {
        let start_frame = (i as f64 * frames_per_peak) as usize;
        let end_frame = (((i + 1) as f64 * frames_per_peak) as usize).min(total_frames);

        let mut max_amp: f32 = 0.0;
        for frame in start_frame..end_frame {
            for ch in 0..channels {
                let idx = frame * channels + ch;
                if idx < samples.len() {
                    let amp = samples[idx].abs();
                    if amp > max_amp {
                        max_amp = amp;
                    }
                }
            }
        }
        peaks.push(max_amp);
    }

    // Normalize so the loudest peak is 1.0
    let global_max = peaks.iter().cloned().fold(0.0_f32, f32::max);
    if global_max > 0.0 {
        for p in &mut peaks {
            *p /= global_max;
        }
    }

    Ok(peaks)
}

/// Generate waveform peaks with the default number of buckets.
pub fn generate_peaks_default(path: &Path) -> Result<Vec<f32>, String> {
    generate_peaks(path, DEFAULT_NUM_PEAKS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_peaks_rejects_missing_file() {
        let result = generate_peaks(Path::new("nonexistent.mp3"), 100);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot open"));
    }

    #[test]
    fn default_peaks_count() {
        assert_eq!(DEFAULT_NUM_PEAKS, 200);
    }
}
