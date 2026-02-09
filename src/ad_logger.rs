use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Failure record for an ad insertion attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdFailure {
    /// Timestamp in "MM-DD-YY HH:MM" format.
    pub t: String,
    /// Ad names involved.
    pub ads: Vec<String>,
    /// Error description (e.g. "concat:error").
    pub err: String,
}

/// Play data: ad_name -> date_str -> vec of hour integers.
pub type AdPlayData = HashMap<String, HashMap<String, Vec<u8>>>;

/// JSON-based ad play statistics logger.
///
/// Records every ad play (per-ad, per-date, per-hour) and tracks failures.
/// Loads from disk on each operation and saves after mutations.
pub struct AdPlayLogger {
    plays_path: PathBuf,
    failures_path: PathBuf,
}

const MAX_FAILURES: usize = 50;

impl AdPlayLogger {
    /// Create a new logger storing files in the given directory.
    pub fn new(directory: &Path) -> Self {
        Self {
            plays_path: directory.join("ad_plays.json"),
            failures_path: directory.join("ad_failures.json"),
        }
    }

    /// Record a play for the given ad at the current date and hour.
    pub fn log_play(&self, ad_name: &str) {
        let now = Local::now();
        let date_key = now.format("%m-%d-%y").to_string();
        let hour = now.format("%H").to_string().parse::<u8>().unwrap_or(0);

        let mut data = self.load_plays();
        data.entry(ad_name.to_string())
            .or_default()
            .entry(date_key)
            .or_default()
            .push(hour);
        self.save_plays(&data);
    }

    /// Record a play for the given ad at a specific date and hour (for testing).
    pub fn log_play_at(&self, ad_name: &str, date_key: &str, hour: u8) {
        let mut data = self.load_plays();
        data.entry(ad_name.to_string())
            .or_default()
            .entry(date_key.to_string())
            .or_default()
            .push(hour);
        self.save_plays(&data);
    }

    /// Record a failure. Trims to MAX_FAILURES (oldest discarded).
    pub fn log_failure(&self, ad_names: &[String], error: &str) {
        let now = Local::now();
        let timestamp = now.format("%m-%d-%y %H:%M").to_string();

        let mut failures = self.load_failures();
        failures.push(AdFailure {
            t: timestamp,
            ads: ad_names.to_vec(),
            err: error.to_string(),
        });
        // Keep only the most recent MAX_FAILURES
        if failures.len() > MAX_FAILURES {
            let excess = failures.len() - MAX_FAILURES;
            failures.drain(..excess);
        }
        self.save_failures(&failures);
    }

    /// Get summary statistics: total plays and per-ad counts sorted descending.
    pub fn get_ad_statistics(&self) -> AdStatistics {
        let data = self.load_plays();
        Self::compute_statistics(&data)
    }

    /// Get statistics filtered to a date range (inclusive, MM-DD-YY format).
    pub fn get_ad_statistics_filtered(&self, start: &str, end: &str) -> AdStatistics {
        let data = self.load_plays();
        let filtered = Self::filter_by_date_range(&data, start, end);
        Self::compute_statistics(&filtered)
    }

    /// Get sorted hours for a specific ad on a specific date.
    pub fn get_play_hours_for_date(&self, ad_name: &str, date_str: &str) -> Vec<u8> {
        let data = self.load_plays();
        let mut hours = data
            .get(ad_name)
            .and_then(|dates| dates.get(date_str))
            .cloned()
            .unwrap_or_default();
        hours.sort();
        hours
    }

    /// Get daily play counts for a specific ad: {date: count}.
    pub fn get_daily_play_counts(&self, ad_name: &str) -> HashMap<String, usize> {
        let data = self.load_plays();
        data.get(ad_name)
            .map(|dates| {
                dates
                    .iter()
                    .map(|(date, hours)| (date.clone(), hours.len()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all failure records.
    pub fn get_failures(&self) -> Vec<AdFailure> {
        self.load_failures()
    }

    /// Get daily confirmed stats: {"YYYY-MM-DD": {"Ad Name": count}}.
    pub fn get_daily_confirmed_stats(
        &self,
        start: &str,
        end: &str,
    ) -> HashMap<String, HashMap<String, usize>> {
        let data = self.load_plays();
        let filtered = Self::filter_by_date_range(&data, start, end);
        let mut result: HashMap<String, HashMap<String, usize>> = HashMap::new();

        for (ad_name, dates) in &filtered {
            for (date_mm, hours) in dates {
                let date_iso = mm_dd_yy_to_iso(date_mm);
                result
                    .entry(date_iso)
                    .or_default()
                    .insert(ad_name.clone(), hours.len());
            }
        }
        result
    }

    /// Get hourly confirmed stats: {"YYYY-MM-DD_HH": {"Ad Name": count}}.
    pub fn get_hourly_confirmed_stats(
        &self,
        start: &str,
        end: &str,
    ) -> HashMap<String, HashMap<String, usize>> {
        let data = self.load_plays();
        let filtered = Self::filter_by_date_range(&data, start, end);
        let mut result: HashMap<String, HashMap<String, usize>> = HashMap::new();

        for (ad_name, dates) in &filtered {
            for (date_mm, hours) in dates {
                let date_iso = mm_dd_yy_to_iso(date_mm);
                // Count plays per hour
                let mut hour_counts: HashMap<u8, usize> = HashMap::new();
                for &h in hours {
                    *hour_counts.entry(h).or_default() += 1;
                }
                for (hour, count) in hour_counts {
                    let key = format!("{}_{:02}", date_iso, hour);
                    result
                        .entry(key)
                        .or_default()
                        .insert(ad_name.clone(), count);
                }
            }
        }
        result
    }

    /// Clear all play data and failures.
    pub fn reset_all(&self) {
        self.save_plays(&HashMap::new());
        self.save_failures(&Vec::new());
    }

    // --- Private helpers ---

    fn load_plays(&self) -> AdPlayData {
        load_json_or_default(&self.plays_path)
    }

    fn save_plays(&self, data: &AdPlayData) {
        save_json(&self.plays_path, data);
    }

    fn load_failures(&self) -> Vec<AdFailure> {
        load_json_or_default(&self.failures_path)
    }

    fn save_failures(&self, data: &Vec<AdFailure>) {
        save_json(&self.failures_path, data);
    }

    fn compute_statistics(data: &AdPlayData) -> AdStatistics {
        let mut total_plays: usize = 0;
        let mut per_ad: Vec<AdStatEntry> = Vec::new();

        for (ad_name, dates) in data {
            let count: usize = dates.values().map(|h| h.len()).sum();
            total_plays += count;
            per_ad.push(AdStatEntry {
                name: ad_name.clone(),
                play_count: count,
            });
        }

        // Sort by play_count descending, then name ascending
        per_ad.sort_by(|a, b| b.play_count.cmp(&a.play_count).then(a.name.cmp(&b.name)));

        AdStatistics {
            total_plays,
            per_ad,
        }
    }

    fn filter_by_date_range(data: &AdPlayData, start: &str, end: &str) -> AdPlayData {
        let mut filtered = AdPlayData::new();
        for (ad_name, dates) in data {
            for (date_key, hours) in dates {
                if date_key.as_str() >= start && date_key.as_str() <= end {
                    filtered
                        .entry(ad_name.clone())
                        .or_default()
                        .insert(date_key.clone(), hours.clone());
                }
            }
        }
        filtered
    }
}

/// Summary statistics for ad plays.
#[derive(Debug, Serialize)]
pub struct AdStatistics {
    pub total_plays: usize,
    pub per_ad: Vec<AdStatEntry>,
}

/// Per-ad statistics entry.
#[derive(Debug, Serialize)]
pub struct AdStatEntry {
    pub name: String,
    pub play_count: usize,
}

/// Convert MM-DD-YY to YYYY-MM-DD.
fn mm_dd_yy_to_iso(date: &str) -> String {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() == 3 {
        format!("20{}-{}-{}", parts[2], parts[0], parts[1])
    } else {
        date.to_string()
    }
}

/// Load JSON from a file, returning a default value on missing/corrupt files.
fn load_json_or_default<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => T::default(),
    }
}

/// Save a value as JSON to a file.
fn save_json<T: Serialize>(path: &Path, data: &T) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(data) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_logger() -> (AdPlayLogger, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let logger = AdPlayLogger::new(dir.path());
        (logger, dir)
    }

    #[test]
    fn log_play_creates_file_and_records() {
        let (logger, _dir) = temp_logger();
        logger.log_play_at("TestAd", "01-15-26", 9);

        let data = logger.load_plays();
        assert_eq!(data.len(), 1);
        assert!(data.contains_key("TestAd"));
        let hours = &data["TestAd"]["01-15-26"];
        assert_eq!(hours, &vec![9u8]);
    }

    #[test]
    fn log_play_appends_to_existing_date() {
        let (logger, _dir) = temp_logger();
        logger.log_play_at("TestAd", "01-15-26", 9);
        logger.log_play_at("TestAd", "01-15-26", 14);

        let data = logger.load_plays();
        let hours = &data["TestAd"]["01-15-26"];
        assert_eq!(hours, &vec![9u8, 14]);
    }

    #[test]
    fn log_play_new_date_creates_entry() {
        let (logger, _dir) = temp_logger();
        logger.log_play_at("TestAd", "01-15-26", 9);
        logger.log_play_at("TestAd", "01-16-26", 10);

        let data = logger.load_plays();
        assert_eq!(data["TestAd"].len(), 2);
        assert!(data["TestAd"].contains_key("01-15-26"));
        assert!(data["TestAd"].contains_key("01-16-26"));
    }

    #[test]
    fn log_failure_records_and_trims() {
        let (logger, _dir) = temp_logger();

        // Add 55 failures — should trim to 50
        for i in 0..55 {
            logger.log_failure(
                &[format!("Ad{}", i)],
                &format!("error:{}", i),
            );
        }

        let failures = logger.get_failures();
        assert_eq!(failures.len(), MAX_FAILURES);
        // Oldest (0-4) should have been trimmed, first remaining should be #5
        assert_eq!(failures[0].ads[0], "Ad5");
        assert_eq!(failures[49].ads[0], "Ad54");
    }

    #[test]
    fn get_ad_statistics_returns_sorted() {
        let (logger, _dir) = temp_logger();
        // Ad A: 3 plays, Ad B: 5 plays, Ad C: 1 play
        for _ in 0..3 {
            logger.log_play_at("Ad A", "01-15-26", 9);
        }
        for _ in 0..5 {
            logger.log_play_at("Ad B", "01-15-26", 10);
        }
        logger.log_play_at("Ad C", "01-15-26", 11);

        let stats = logger.get_ad_statistics();
        assert_eq!(stats.total_plays, 9);
        assert_eq!(stats.per_ad.len(), 3);
        assert_eq!(stats.per_ad[0].name, "Ad B");
        assert_eq!(stats.per_ad[0].play_count, 5);
        assert_eq!(stats.per_ad[1].name, "Ad A");
        assert_eq!(stats.per_ad[1].play_count, 3);
        assert_eq!(stats.per_ad[2].name, "Ad C");
        assert_eq!(stats.per_ad[2].play_count, 1);
    }

    #[test]
    fn get_ad_statistics_filtered_by_date() {
        let (logger, _dir) = temp_logger();
        logger.log_play_at("Ad A", "01-10-26", 9);
        logger.log_play_at("Ad A", "01-15-26", 10);
        logger.log_play_at("Ad A", "01-20-26", 11);

        let stats = logger.get_ad_statistics_filtered("01-12-26", "01-18-26");
        assert_eq!(stats.total_plays, 1);
        assert_eq!(stats.per_ad[0].play_count, 1);
    }

    #[test]
    fn get_daily_play_counts() {
        let (logger, _dir) = temp_logger();
        logger.log_play_at("Ad A", "01-15-26", 9);
        logger.log_play_at("Ad A", "01-15-26", 10);
        logger.log_play_at("Ad A", "01-16-26", 14);

        let counts = logger.get_daily_play_counts("Ad A");
        assert_eq!(counts["01-15-26"], 2);
        assert_eq!(counts["01-16-26"], 1);
    }

    #[test]
    fn get_play_hours_for_date() {
        let (logger, _dir) = temp_logger();
        logger.log_play_at("Ad A", "01-15-26", 14);
        logger.log_play_at("Ad A", "01-15-26", 9);
        logger.log_play_at("Ad A", "01-15-26", 14);

        let hours = logger.get_play_hours_for_date("Ad A", "01-15-26");
        assert_eq!(hours, vec![9, 14, 14]); // sorted
    }

    #[test]
    fn get_daily_confirmed_stats() {
        let (logger, _dir) = temp_logger();
        logger.log_play_at("Ad A", "01-15-26", 9);
        logger.log_play_at("Ad A", "01-15-26", 10);
        logger.log_play_at("Ad B", "01-15-26", 9);

        let stats = logger.get_daily_confirmed_stats("01-10-26", "01-20-26");
        assert_eq!(stats["2026-01-15"]["Ad A"], 2);
        assert_eq!(stats["2026-01-15"]["Ad B"], 1);
    }

    #[test]
    fn get_hourly_confirmed_stats() {
        let (logger, _dir) = temp_logger();
        logger.log_play_at("Ad A", "01-15-26", 9);
        logger.log_play_at("Ad A", "01-15-26", 9);
        logger.log_play_at("Ad A", "01-15-26", 14);

        let stats = logger.get_hourly_confirmed_stats("01-10-26", "01-20-26");
        assert_eq!(stats["2026-01-15_09"]["Ad A"], 2);
        assert_eq!(stats["2026-01-15_14"]["Ad A"], 1);
    }

    #[test]
    fn reset_all_clears_both_files() {
        let (logger, _dir) = temp_logger();
        logger.log_play_at("Ad A", "01-15-26", 9);
        logger.log_failure(&["Ad A".into()], "test error");

        logger.reset_all();

        let stats = logger.get_ad_statistics();
        assert_eq!(stats.total_plays, 0);
        assert!(stats.per_ad.is_empty());
        assert!(logger.get_failures().is_empty());
    }

    #[test]
    fn handles_missing_files_gracefully() {
        let dir = tempfile::tempdir().unwrap();
        let logger = AdPlayLogger::new(dir.path());
        // No files exist yet — should return empty data
        let stats = logger.get_ad_statistics();
        assert_eq!(stats.total_plays, 0);
        assert!(logger.get_failures().is_empty());
    }

    #[test]
    fn handles_corrupt_json_gracefully() {
        let dir = tempfile::tempdir().unwrap();
        let logger = AdPlayLogger::new(dir.path());
        // Write garbage to both files
        fs::write(dir.path().join("ad_plays.json"), "not valid json{{{").unwrap();
        fs::write(dir.path().join("ad_failures.json"), "also garbage").unwrap();

        // Should fall back to empty
        let stats = logger.get_ad_statistics();
        assert_eq!(stats.total_plays, 0);
        assert!(logger.get_failures().is_empty());
    }

    #[test]
    fn mm_dd_yy_to_iso_converts_correctly() {
        assert_eq!(mm_dd_yy_to_iso("01-15-26"), "2026-01-15");
        assert_eq!(mm_dd_yy_to_iso("12-31-25"), "2025-12-31");
    }
}
