//! Audio editor operations — ffmpeg filter chain builder + silence detection.
//!
//! All edits are non-destructive (stored as an operation list in the frontend).
//! On export, this module builds a single ffmpeg invocation with an `-af` filter chain.

use std::path::Path;
use std::process::Command;

// ── Operation types ──────────────────────────────────────────────────────────

/// A time-range to cut (remove) from the output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CutRegion {
    pub start_secs: f64,
    pub end_secs: f64,
}

/// All non-destructive edit operations to apply at export time.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EditorOperations {
    /// Trim start in seconds (0.0 = beginning of file).
    pub trim_start_secs: f64,
    /// Trim end in seconds (0.0 = use file duration).
    pub trim_end_secs: f64,
    /// Volume adjustment in dB (0.0 = no change, positive = louder).
    pub volume_db: f64,
    /// Playback speed multiplier (1.0 = original, 0.5–2.0 range per atempo step).
    pub speed: f64,
    /// Pitch shift in semitones (0.0 = no change, ±12 range).
    pub pitch_semitones: f64,
    /// Fade-in duration in seconds (0.0 = no fade).
    pub fade_in_secs: f64,
    /// Fade-out duration in seconds (0.0 = no fade).
    pub fade_out_secs: f64,
    /// Apply EBU R128 loudness normalization.
    pub normalize: bool,
    /// Regions to remove from the audio (silence-gapped cuts via aselect).
    pub cuts: Vec<CutRegion>,
    /// Total output duration in seconds — used to place the fade-out correctly.
    /// Set by the exporter after computing trimmed length.
    pub total_duration_secs: f64,
}

/// A detected silence region in an audio file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SilenceRegion {
    pub start_secs: f64,
    pub end_secs: f64,
}

// ── ffmpeg filter chain builder ──────────────────────────────────────────────

/// Build the complete ffmpeg argument list for the given operations.
/// Returns a `Vec<String>` ready for `Command::new("ffmpeg").args(...)`.
pub fn build_ffmpeg_args(
    input_path: &str,
    output_path: &str,
    ops: &EditorOperations,
    format: &str, // "mp3" or "wav"
    quality: u8,  // mp3: 0 (best) – 9 (worst); ignored for wav
) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    args.push("-y".into());
    args.push("-i".into());
    args.push(input_path.to_string());

    // Input trim via -ss / -to (applied before filters for efficiency)
    if ops.trim_start_secs > 0.001 {
        args.push("-ss".into());
        args.push(format!("{:.6}", ops.trim_start_secs));
    }
    if ops.trim_end_secs > 0.001 {
        args.push("-to".into());
        args.push(format!("{:.6}", ops.trim_end_secs));
    }

    // Build audio filter chain
    let mut filters: Vec<String> = Vec::new();

    // Cut regions via aselect (keep everything OUTSIDE cut ranges)
    if !ops.cuts.is_empty() {
        let conditions: Vec<String> = ops
            .cuts
            .iter()
            .map(|c| format!("not(between(t,{:.6},{:.6}))", c.start_secs, c.end_secs))
            .collect();
        filters.push(format!("aselect='{}'", conditions.join("*")));
        filters.push("asetpts=N/SR/TB".into());
    }

    // Volume
    if ops.volume_db.abs() > 0.01 {
        filters.push(format!("volume={:.2}dB", ops.volume_db));
    }

    // Speed (atempo; chain multiple for values outside [0.5, 2.0])
    if (ops.speed - 1.0).abs() > 0.01 && ops.speed > 0.0 {
        filters.extend(build_atempo_chain(ops.speed));
    }

    // Pitch shift — asetrate changes perceived pitch, aresample corrects playback rate
    if ops.pitch_semitones.abs() > 0.01 {
        let ratio = 2.0_f64.powf(ops.pitch_semitones / 12.0);
        filters.push(format!("asetrate=44100*{:.6}", ratio));
        filters.push("aresample=44100".into());
    }

    // Fade in
    if ops.fade_in_secs > 0.001 {
        filters.push(format!("afade=t=in:st=0:d={:.3}", ops.fade_in_secs));
    }

    // Fade out — requires knowing output start time = total_duration - fade_out
    if ops.fade_out_secs > 0.001 && ops.total_duration_secs > ops.fade_out_secs {
        let st = ops.total_duration_secs - ops.fade_out_secs;
        filters.push(format!("afade=t=out:st={:.3}:d={:.3}", st, ops.fade_out_secs));
    }

    // Loudness normalize (EBU R128)
    if ops.normalize {
        filters.push("loudnorm".into());
    }

    if !filters.is_empty() {
        args.push("-af".into());
        args.push(filters.join(","));
    }

    // Output format
    match format {
        "wav" => {
            args.push("-acodec".into());
            args.push("pcm_s16le".into());
        }
        _ => {
            args.push("-q:a".into());
            args.push(quality.to_string());
        }
    }

    args.push(output_path.to_string());
    args
}

/// Build an atempo filter chain, chaining multiple steps when speed is outside [0.5, 2.0].
fn build_atempo_chain(speed: f64) -> Vec<String> {
    if speed <= 0.0 {
        return vec!["atempo=1.0".into()];
    }
    let mut remaining = speed;
    let mut chain = Vec::new();
    while remaining > 2.0 + f64::EPSILON {
        chain.push("atempo=2.0".into());
        remaining /= 2.0;
    }
    while remaining < 0.5 - f64::EPSILON {
        chain.push("atempo=0.5".into());
        remaining /= 0.5;
    }
    chain.push(format!("atempo={:.6}", remaining));
    chain
}

/// Run ffmpeg with the given argument list. Returns `Ok(())` on success.
pub fn run_ffmpeg(args: &[String]) -> Result<(), String> {
    let status = Command::new("ffmpeg")
        .args(args)
        .status()
        .map_err(|e| format!("Failed to launch ffmpeg: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "ffmpeg exited with status {}",
            status.code().unwrap_or(-1)
        ))
    }
}

// ── Silence detection ────────────────────────────────────────────────────────

/// Scan an audio file for silence regions using `ffmpeg silencedetect`.
/// `threshold_db` is the noise threshold (e.g. `-40.0`).
/// `min_duration_secs` is the minimum silence length to report.
pub fn detect_silence_regions(
    path: &Path,
    threshold_db: f64,
    min_duration_secs: f64,
) -> Result<Vec<SilenceRegion>, String> {
    let output = Command::new("ffmpeg")
        .args([
            "-i",
            path.to_str().unwrap_or(""),
            "-af",
            &format!(
                "silencedetect=n={:.1}dB:d={:.3}",
                threshold_db, min_duration_secs
            ),
            "-f",
            "null",
            "-",
        ])
        .output()
        .map_err(|e| format!("Failed to launch ffmpeg silencedetect: {e}"))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_silence_output(&stderr)
}

fn parse_silence_output(stderr: &str) -> Result<Vec<SilenceRegion>, String> {
    let mut regions: Vec<SilenceRegion> = Vec::new();
    let mut current_start: Option<f64> = None;

    for line in stderr.lines() {
        if line.contains("silence_start:") {
            if let Some(pos) = line.rfind("silence_start:") {
                let tail = line[pos + "silence_start:".len()..].trim();
                if let Ok(t) = tail.split_whitespace().next().unwrap_or("").parse::<f64>() {
                    current_start = Some(t);
                }
            }
        } else if line.contains("silence_end:") {
            if let Some(pos) = line.rfind("silence_end:") {
                let tail = line[pos + "silence_end:".len()..].trim();
                let val_str = tail.split(|c: char| c == '|' || c.is_whitespace())
                    .next()
                    .unwrap_or("");
                if let Ok(t) = val_str.parse::<f64>() {
                    if let Some(start) = current_start.take() {
                        regions.push(SilenceRegion {
                            start_secs: start,
                            end_secs: t,
                        });
                    }
                }
            }
        }
    }

    // Trailing silence (file ends during silence)
    if let Some(start) = current_start {
        regions.push(SilenceRegion {
            start_secs: start,
            end_secs: f64::MAX,
        });
    }

    Ok(regions)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn no_ops() -> EditorOperations {
        EditorOperations {
            trim_start_secs: 0.0,
            trim_end_secs: 0.0,
            volume_db: 0.0,
            speed: 1.0,
            pitch_semitones: 0.0,
            fade_in_secs: 0.0,
            fade_out_secs: 0.0,
            normalize: false,
            cuts: vec![],
            total_duration_secs: 0.0,
        }
    }

    #[test]
    fn build_args_passthrough() {
        let ops = no_ops();
        let args = build_ffmpeg_args("in.mp3", "out.mp3", &ops, "mp3", 2);
        assert!(!args.contains(&"-af".to_string()));
        assert!(args.contains(&"-y".to_string()));
        assert!(args.contains(&"in.mp3".to_string()));
        assert!(args.contains(&"out.mp3".to_string()));
    }

    #[test]
    fn build_args_with_volume() {
        let mut ops = no_ops();
        ops.volume_db = 3.0;
        let args = build_ffmpeg_args("in.mp3", "out.mp3", &ops, "mp3", 2);
        let af_pos = args.iter().position(|a| a == "-af").expect("-af present");
        assert!(args[af_pos + 1].contains("volume="));
        assert!(args[af_pos + 1].contains("3.00dB"));
    }

    #[test]
    fn build_args_trim() {
        let mut ops = no_ops();
        ops.trim_start_secs = 5.0;
        ops.trim_end_secs = 60.0;
        let args = build_ffmpeg_args("in.mp3", "out.mp3", &ops, "mp3", 2);
        let ss_pos = args.iter().position(|a| a == "-ss").expect("-ss present");
        assert!(args[ss_pos + 1].starts_with("5."));
        let to_pos = args.iter().position(|a| a == "-to").expect("-to present");
        assert!(args[to_pos + 1].starts_with("60."));
    }

    #[test]
    fn build_args_wav_format() {
        let ops = no_ops();
        let args = build_ffmpeg_args("in.mp3", "out.wav", &ops, "wav", 0);
        assert!(args.contains(&"pcm_s16le".to_string()));
        assert!(!args.contains(&"-q:a".to_string()));
    }

    #[test]
    fn atempo_chain_fast() {
        let chain = build_atempo_chain(4.0);
        // 4.0 → atempo=2.0, then atempo=2.0
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0], "atempo=2.0");
    }

    #[test]
    fn atempo_chain_slow() {
        let chain = build_atempo_chain(0.25);
        // 0.25 → atempo=0.5, then atempo=0.5
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0], "atempo=0.5");
    }

    #[test]
    fn atempo_chain_normal() {
        let chain = build_atempo_chain(1.5);
        assert_eq!(chain.len(), 1);
        assert!(chain[0].contains("1.5"));
    }

    #[test]
    fn parse_silence_basic() {
        let stderr = "\
[silencedetect @ 0xabc] silence_start: 1.234\n\
[silencedetect @ 0xabc] silence_end: 2.567 | silence_duration: 1.333\n";
        let regions = parse_silence_output(stderr).unwrap();
        assert_eq!(regions.len(), 1);
        assert!((regions[0].start_secs - 1.234).abs() < 0.001);
        assert!((regions[0].end_secs - 2.567).abs() < 0.001);
    }

    #[test]
    fn parse_silence_multiple() {
        let stderr = "\
[silencedetect @ 0xabc] silence_start: 0.0\n\
[silencedetect @ 0xabc] silence_end: 0.5 | silence_duration: 0.5\n\
[silencedetect @ 0xabc] silence_start: 10.0\n\
[silencedetect @ 0xabc] silence_end: 12.0 | silence_duration: 2.0\n";
        let regions = parse_silence_output(stderr).unwrap();
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn build_args_normalize() {
        let mut ops = no_ops();
        ops.normalize = true;
        let args = build_ffmpeg_args("in.mp3", "out.mp3", &ops, "mp3", 2);
        let af_pos = args.iter().position(|a| a == "-af").unwrap();
        assert!(args[af_pos + 1].contains("loudnorm"));
    }

    #[test]
    fn build_args_cuts() {
        let mut ops = no_ops();
        ops.cuts = vec![CutRegion { start_secs: 5.0, end_secs: 10.0 }];
        let args = build_ffmpeg_args("in.mp3", "out.mp3", &ops, "mp3", 2);
        let af_pos = args.iter().position(|a| a == "-af").unwrap();
        assert!(args[af_pos + 1].contains("aselect"));
        assert!(args[af_pos + 1].contains("asetpts"));
    }
}
