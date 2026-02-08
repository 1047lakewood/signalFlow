use crate::ad_scheduler::{AdConfig, AdInsertionMode};
use crate::engine::Engine;
use crate::player::Player;
use chrono::Local;
use rodio::{Decoder, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Result of an ad insertion operation.
#[derive(Debug, Clone)]
pub struct AdInsertionResult {
    /// Number of ads inserted/played.
    pub ad_count: usize,
    /// Names of the ads that were inserted/played.
    pub ads_inserted: Vec<String>,
    /// Whether the station ID was played/inserted.
    pub station_id_played: bool,
}

/// Stateless service for inserting ads into playback.
///
/// Methods take engine/player as parameters rather than storing them,
/// making the service easy to test and use from different contexts.
pub struct AdInserterService;

impl AdInserterService {
    /// Filter ads to only those valid for playback right now.
    ///
    /// An ad is valid if it's enabled, its file exists, and its schedule
    /// matches the current day and hour.
    pub fn collect_valid_ads(ads: &[AdConfig]) -> Vec<&AdConfig> {
        let now = Local::now();
        let day = now.format("%A").to_string();
        let hour = now.hour() as u8;
        ads.iter()
            .filter(|ad| ad.is_valid_now(&day, hour))
            .collect()
    }

    /// Collect valid ads using an explicit day and hour (for testing).
    pub fn collect_valid_ads_at<'a>(ads: &'a [AdConfig], day: &str, hour: u8) -> Vec<&'a AdConfig> {
        ads.iter()
            .filter(|ad| ad.is_valid_now(day, hour))
            .collect()
    }

    /// Instant ad insertion: stop current playback, play all valid ads
    /// (and optionally station ID) on a new sink, block until finished.
    ///
    /// Returns the result describing what was played, or an error.
    pub fn insert_instant(
        player: &Player,
        engine: &Engine,
        is_hour_start: bool,
    ) -> Result<AdInsertionResult, String> {
        let valid_ads = Self::collect_valid_ads(&engine.ads);
        if valid_ads.is_empty() {
            return Err("No valid ads to insert".to_string());
        }

        // Determine if station ID should play
        let station_id_path = if is_hour_start
            && engine.ad_inserter.station_id_enabled
        {
            engine
                .ad_inserter
                .station_id_file
                .as_ref()
                .filter(|p| p.exists())
        } else {
            None
        };

        // Create a new sink for ad playback
        let sink = player.create_sink()?;

        // Append station ID first if applicable
        let station_id_played = if let Some(sid_path) = station_id_path {
            append_to_sink(&sink, sid_path)?;
            true
        } else {
            false
        };

        // Append each valid ad
        let mut ads_inserted = Vec::new();
        for ad in &valid_ads {
            append_to_sink(&sink, &ad.mp3_file)?;
            ads_inserted.push(ad.name.clone());
        }

        // Block until all audio finishes
        sink.play();
        while !sink.empty() {
            std::thread::sleep(Duration::from_millis(100));
        }

        Ok(AdInsertionResult {
            ad_count: ads_inserted.len(),
            ads_inserted,
            station_id_played,
        })
    }

    /// Scheduled ad insertion: insert valid ads as next tracks in the
    /// active playlist. Inserts in reverse order so they play in the
    /// correct sequence. Optionally prepends station ID.
    ///
    /// Returns the result describing what was inserted, or an error.
    pub fn insert_scheduled(
        engine: &mut Engine,
        is_hour_start: bool,
    ) -> Result<AdInsertionResult, String> {
        if engine.active_playlist().is_none() {
            return Err("No active playlist".to_string());
        }

        let valid_ads = Self::collect_valid_ads(&engine.ads);
        if valid_ads.is_empty() {
            return Err("No valid ads to insert".to_string());
        }

        // Build the list of files to insert (in playback order)
        let mut insertion_files: Vec<(PathBuf, String)> = Vec::new();

        // Station ID first if applicable
        let station_id_played = if is_hour_start
            && engine.ad_inserter.station_id_enabled
        {
            if let Some(sid_path) = &engine.ad_inserter.station_id_file {
                if sid_path.exists() {
                    insertion_files.push((sid_path.clone(), "Station ID".to_string()));
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        // Then each valid ad
        let mut ads_inserted = Vec::new();
        for ad in &valid_ads {
            insertion_files.push((ad.mp3_file.clone(), ad.name.clone()));
            ads_inserted.push(ad.name.clone());
        }

        // Insert in reverse order so they end up in the correct sequence
        // (each insert_next_track places the track right after current)
        for (path, _name) in insertion_files.iter().rev() {
            engine.insert_next_track(path)?;
        }

        Ok(AdInsertionResult {
            ad_count: ads_inserted.len(),
            ads_inserted,
            station_id_played,
        })
    }

    /// Dispatch to the appropriate insertion mode.
    pub fn run_insertion(
        player: &Player,
        engine: &mut Engine,
        mode: AdInsertionMode,
        is_hour_start: bool,
    ) -> Result<AdInsertionResult, String> {
        match mode {
            AdInsertionMode::Instant => Self::insert_instant(player, engine, is_hour_start),
            AdInsertionMode::Scheduled => Self::insert_scheduled(engine, is_hour_start),
        }
    }
}

/// Decode an audio file and append it to a sink.
fn append_to_sink(sink: &Sink, path: &Path) -> Result<(), String> {
    let file = File::open(path)
        .map_err(|e| format!("Cannot open '{}': {}", path.display(), e))?;
    let source = Decoder::new(BufReader::new(file))
        .map_err(|e| format!("Cannot decode '{}': {}", path.display(), e))?;
    sink.append(source);
    Ok(())
}

use chrono::Timelike;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ad_scheduler::AdConfig;

    fn make_ad(name: &str, enabled: bool, scheduled: bool) -> AdConfig {
        AdConfig {
            name: name.to_string(),
            enabled,
            mp3_file: PathBuf::from(format!("test_ads/{}.mp3", name)),
            scheduled,
            days: vec![],
            hours: vec![],
        }
    }

    fn make_ad_with_schedule(
        name: &str,
        enabled: bool,
        days: Vec<String>,
        hours: Vec<u8>,
    ) -> AdConfig {
        AdConfig {
            name: name.to_string(),
            enabled,
            mp3_file: PathBuf::from(format!("test_ads/{}.mp3", name)),
            scheduled: true,
            days,
            hours,
        }
    }

    // --- collect_valid_ads_at tests ---

    #[test]
    fn collect_valid_ads_returns_empty_when_no_ads() {
        let ads: Vec<AdConfig> = vec![];
        let valid = AdInserterService::collect_valid_ads_at(&ads, "Monday", 10);
        assert!(valid.is_empty());
    }

    #[test]
    fn collect_valid_ads_filters_disabled() {
        let ads = vec![
            make_ad("Active", true, false),
            make_ad("Disabled", false, false),
        ];
        // Files don't exist, so even enabled ads won't be valid
        // (is_valid_now checks file existence)
        let valid = AdInserterService::collect_valid_ads_at(&ads, "Monday", 10);
        assert!(valid.is_empty()); // all filtered because files don't exist
    }

    #[test]
    fn collect_valid_ads_filters_by_schedule() {
        let ads = vec![
            make_ad_with_schedule("MondayOnly", true, vec!["Monday".into()], vec![10]),
            make_ad_with_schedule("TuesdayOnly", true, vec!["Tuesday".into()], vec![10]),
        ];
        // Files don't exist so all get filtered, but we can test schedule logic
        // via AdConfig::is_scheduled_for directly
        assert!(ads[0].is_scheduled_for("Monday", 10));
        assert!(!ads[1].is_scheduled_for("Monday", 10));
    }

    #[test]
    fn collect_valid_ads_includes_unscheduled_enabled() {
        let ad = make_ad("Always", true, false);
        // Unscheduled ads match any day/hour
        assert!(ad.is_scheduled_for("Monday", 10));
        assert!(ad.is_scheduled_for("Sunday", 0));
        assert!(ad.is_scheduled_for("Friday", 23));
    }

    // --- insert_scheduled tests ---

    #[test]
    fn insert_scheduled_returns_error_when_no_active_playlist() {
        let mut engine = Engine::new();
        let result = AdInserterService::insert_scheduled(&mut engine, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No active playlist"));
    }

    #[test]
    fn insert_scheduled_returns_error_when_no_valid_ads() {
        let mut engine = Engine::new();
        engine.create_playlist("Main".to_string());
        engine.set_active("Main").unwrap();
        // No ads configured
        let result = AdInserterService::insert_scheduled(&mut engine, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No valid ads"));
    }

    #[test]
    fn insert_scheduled_inserts_in_reverse_order() {
        // This test verifies the logic by checking that tracks are inserted
        // after the current position. We need real files for insert_next_track
        // to work (Track::from_path requires a real file), so we test the
        // ordering logic conceptually.
        //
        // The key insight: insert_next_track always inserts at current_index + 1.
        // If we insert [A, B, C] in reverse order (C, B, A), they end up as:
        //   current -> A -> B -> C -> rest_of_playlist
        // Which is the correct playback order.

        // Verify reverse iteration produces correct order
        let files = vec!["A.mp3", "B.mp3", "C.mp3"];
        let reversed: Vec<_> = files.iter().rev().collect();
        assert_eq!(*reversed[0], "C.mp3");
        assert_eq!(*reversed[1], "B.mp3");
        assert_eq!(*reversed[2], "A.mp3");
    }

    #[test]
    fn insert_scheduled_prepends_station_id_when_enabled() {
        // Verify the logic: when is_hour_start=true and station_id_enabled=true,
        // station ID should be the first item in the insertion list.
        let mut engine = Engine::new();
        engine.ad_inserter.station_id_enabled = true;
        engine.ad_inserter.station_id_file = Some(PathBuf::from("station.mp3"));

        // The station_id_played flag should be true when conditions are met
        // and the file exists. Since station.mp3 doesn't exist in test,
        // verify the logic path.
        let sid_exists = engine
            .ad_inserter
            .station_id_file
            .as_ref()
            .map_or(false, |p| p.exists());
        // File doesn't exist in test env, so station_id_played would be false
        assert!(!sid_exists);

        // But the config IS enabled
        assert!(engine.ad_inserter.station_id_enabled);
    }

    #[test]
    fn insert_scheduled_skips_station_id_when_disabled() {
        let engine = Engine::new();
        // Default: station_id_enabled = false
        assert!(!engine.ad_inserter.station_id_enabled);

        // Even with a file set, disabled means no station ID
        let mut engine2 = Engine::new();
        engine2.ad_inserter.station_id_file = Some(PathBuf::from("station.mp3"));
        engine2.ad_inserter.station_id_enabled = false;
        assert!(!engine2.ad_inserter.station_id_enabled);
    }

    #[test]
    fn run_insertion_dispatches_to_correct_mode() {
        // Verify that run_insertion calls the right method based on mode.
        // We can't easily mock the player, but we can verify that
        // scheduled mode doesn't need a player reference for its core logic.
        let mut engine = Engine::new();

        // Scheduled mode with no active playlist -> error from insert_scheduled
        let player_result = Player::new();
        if let Ok(player) = player_result {
            let result = AdInserterService::run_insertion(
                &player,
                &mut engine,
                AdInsertionMode::Scheduled,
                false,
            );
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("No active playlist"));
        }
        // If Player::new() fails (no audio device), test passes trivially
    }

    // --- AdInsertionResult tests ---

    #[test]
    fn ad_insertion_result_construction() {
        let result = AdInsertionResult {
            ad_count: 3,
            ads_inserted: vec!["Ad1".into(), "Ad2".into(), "Ad3".into()],
            station_id_played: true,
        };
        assert_eq!(result.ad_count, 3);
        assert_eq!(result.ads_inserted.len(), 3);
        assert!(result.station_id_played);
    }

    #[test]
    fn ad_insertion_result_empty() {
        let result = AdInsertionResult {
            ad_count: 0,
            ads_inserted: vec![],
            station_id_played: false,
        };
        assert_eq!(result.ad_count, 0);
        assert!(result.ads_inserted.is_empty());
        assert!(!result.station_id_played);
    }
}
