use rodio::{Decoder, Source};
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{BufReader, Read as _, Write as _};
use std::path::{Path, PathBuf};

/// Number of amplitude peaks to generate for the waveform overview.
const DEFAULT_NUM_PEAKS: usize = 200;

/// Generate a waveform overview from an audio file.
/// Returns a vector of peak amplitudes (0.0–1.0) representing the audio shape.
///
/// Streams samples without collecting them all into memory, keeping usage ~1KB
/// regardless of track length.
pub fn generate_peaks(path: &Path, num_peaks: usize) -> Result<Vec<f32>, String> {
    let file = File::open(path)
        .map_err(|e| format!("Cannot open '{}': {}", path.display(), e))?;
    let source = Decoder::new(BufReader::new(file))
        .map_err(|e| format!("Cannot decode '{}': {}", path.display(), e))?;

    let channels = source.channels() as usize;
    let sample_rate = source.sample_rate() as usize;

    // Compute peaks in ~100ms chunks by streaming through samples.
    // Each chunk = sample_rate/10 frames × channels samples.
    let frames_per_chunk = (sample_rate / 10).max(1);
    let samples_per_chunk = frames_per_chunk * channels.max(1);

    let mut chunk_peaks: Vec<f32> = Vec::new();
    let mut chunk_max: f32 = 0.0;
    let mut sample_count: usize = 0;

    for sample in source.convert_samples::<f32>() {
        let amp = sample.abs();
        if amp > chunk_max {
            chunk_max = amp;
        }
        sample_count += 1;
        if sample_count >= samples_per_chunk {
            chunk_peaks.push(chunk_max);
            chunk_max = 0.0;
            sample_count = 0;
        }
    }
    // Flush the last partial chunk
    if sample_count > 0 {
        chunk_peaks.push(chunk_max);
    }

    if chunk_peaks.is_empty() {
        return Ok(vec![0.0; num_peaks]);
    }

    // Downsample chunk_peaks → num_peaks output peaks (take max within each bucket)
    let peaks = downsample_peaks(&chunk_peaks, num_peaks);

    // Normalize so the loudest peak is 1.0
    let global_max = peaks.iter().cloned().fold(0.0_f32, f32::max);
    if global_max > 0.0 {
        Ok(peaks.into_iter().map(|p| p / global_max).collect())
    } else {
        Ok(peaks)
    }
}

/// Downsample raw chunk peaks into `num_peaks` output buckets by taking
/// the max value within each bucket's range.
fn downsample_peaks(chunk_peaks: &[f32], num_peaks: usize) -> Vec<f32> {
    if chunk_peaks.len() <= num_peaks {
        // Fewer chunks than requested peaks — pad with zeros
        let mut out = chunk_peaks.to_vec();
        out.resize(num_peaks, 0.0);
        return out;
    }

    let chunks_per_peak = chunk_peaks.len() as f64 / num_peaks as f64;
    let mut peaks = Vec::with_capacity(num_peaks);

    for i in 0..num_peaks {
        let start = (i as f64 * chunks_per_peak) as usize;
        let end = (((i + 1) as f64 * chunks_per_peak) as usize).min(chunk_peaks.len());
        let max_val = chunk_peaks[start..end]
            .iter()
            .cloned()
            .fold(0.0_f32, f32::max);
        peaks.push(max_val);
    }

    peaks
}

/// Generate waveform peaks with the default number of buckets.
pub fn generate_peaks_default(path: &Path) -> Result<Vec<f32>, String> {
    generate_peaks(path, DEFAULT_NUM_PEAKS)
}

// ── Disk cache ──────────────────────────────────────────────────────────────

/// Return cache directory: `<data_local_dir>/signalFlow/waveform_cache/`
fn cache_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("signalFlow").join("waveform_cache"))
}

/// Build a cache key from file path + size + modification time.
fn cache_key(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.to_string_lossy().as_ref().hash(&mut hasher);
    size.hash(&mut hasher);
    mtime.hash(&mut hasher);
    Some(format!("{:016x}", hasher.finish()))
}

/// Try to load cached peaks from disk.
fn load_cached(path: &Path) -> Option<Vec<f32>> {
    let dir = cache_dir()?;
    let key = cache_key(path)?;
    let cache_path = dir.join(&key);

    let mut file = File::open(&cache_path).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;

    // Expect exactly DEFAULT_NUM_PEAKS * 4 bytes (little-endian f32)
    if bytes.len() != DEFAULT_NUM_PEAKS * 4 {
        return None;
    }

    let peaks: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    Some(peaks)
}

/// Write peaks to disk cache.
fn save_cache(path: &Path, peaks: &[f32]) {
    let Some(dir) = cache_dir() else { return };
    let Some(key) = cache_key(path) else { return };

    if fs::create_dir_all(&dir).is_err() {
        return;
    }

    let cache_path = dir.join(&key);
    let Ok(mut file) = File::create(&cache_path) else {
        return;
    };

    let bytes: Vec<u8> = peaks.iter().flat_map(|f| f.to_le_bytes()).collect();
    let _ = file.write_all(&bytes);
}

/// Generate waveform peaks with disk caching.
/// Checks cache first; computes and caches on miss.
pub fn generate_peaks_cached(path: &Path) -> Result<Vec<f32>, String> {
    // Cache hit?
    if let Some(peaks) = load_cached(path) {
        return Ok(peaks);
    }

    // Cache miss — compute
    let peaks = generate_peaks_default(path)?;

    // Store for next time (best-effort, ignore errors)
    save_cache(path, &peaks);

    Ok(peaks)
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

    #[test]
    fn downsample_exact() {
        let input = vec![0.5; 200];
        let result = downsample_peaks(&input, 200);
        assert_eq!(result.len(), 200);
        assert!((result[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn downsample_fewer_chunks_than_peaks() {
        let input = vec![1.0, 0.5];
        let result = downsample_peaks(&input, 200);
        assert_eq!(result.len(), 200);
        assert!((result[0] - 1.0).abs() < 1e-6);
        assert!((result[1] - 0.5).abs() < 1e-6);
        // Remaining should be zero-padded
        assert!((result[2] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn downsample_many_to_few() {
        // 1000 chunks → 10 peaks, each bucket of 100 chunks
        let mut input = vec![0.0; 1000];
        input[50] = 0.8; // bucket 0
        input[150] = 0.6; // bucket 1
        let result = downsample_peaks(&input, 10);
        assert_eq!(result.len(), 10);
        assert!((result[0] - 0.8).abs() < 1e-6);
        assert!((result[1] - 0.6).abs() < 1e-6);
    }

    #[test]
    fn cache_key_deterministic() {
        // cache_key returns None for nonexistent files
        assert!(cache_key(Path::new("nonexistent.mp3")).is_none());
    }
}
