use crate::lecture_detector::LectureDetector;
use chrono::{Local, Timelike};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// --- Constants ---

/// Base check interval in seconds (dynamic sleep overrides this).
pub const LOOP_SLEEP: u64 = 60;

/// How often to check for track changes (seconds).
pub const TRACK_CHANGE_CHECK_INTERVAL: u64 = 5;

/// Wait after errors before retrying (seconds).
pub const ERROR_RETRY_DELAY: u64 = 300;

/// Safety margin: if fewer than this many minutes remain in the hour,
/// force an instant ad insertion.
pub const SAFETY_MARGIN_MINUTES: f64 = 3.0;

// --- Ad Configuration ---

/// A single ad definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdConfig {
    /// Display name of the ad.
    pub name: String,
    /// Whether this ad is active.
    pub enabled: bool,
    /// Path to the MP3 file.
    pub mp3_file: PathBuf,
    /// Whether this ad uses day/hour scheduling.
    /// If false, the ad plays whenever it's enabled (any hour).
    pub scheduled: bool,
    /// Days of the week this ad plays.
    /// Uses day names: "Sunday", "Monday", etc. Empty = all days.
    #[serde(default)]
    pub days: Vec<String>,
    /// Hours of the day this ad plays (0-23). Empty = all hours.
    #[serde(default)]
    pub hours: Vec<u8>,
}

impl AdConfig {
    /// Create a new ad with default settings.
    pub fn new(name: String, mp3_file: PathBuf) -> Self {
        AdConfig {
            name,
            enabled: true,
            mp3_file,
            scheduled: false,
            days: Vec::new(),
            hours: Vec::new(),
        }
    }

    /// Check if this ad is scheduled to play at the given day and hour.
    ///
    /// Rules:
    /// 1. If not scheduled -> true (always plays when enabled)
    /// 2. Check day: if days is non-empty and current day not in list -> false
    /// 3. Check hour: if hours is non-empty and current hour not in list -> false
    /// 4. Otherwise -> true
    pub fn is_scheduled_for(&self, day_name: &str, hour: u8) -> bool {
        if !self.scheduled {
            return true;
        }
        if !self.days.is_empty() {
            let day_lower = day_name.to_lowercase();
            if !self.days.iter().any(|d| d.to_lowercase() == day_lower) {
                return false;
            }
        }
        if !self.hours.is_empty() && !self.hours.contains(&hour) {
            return false;
        }
        true
    }

    /// Check if this ad is valid for playback right now.
    /// Must be enabled, file must exist, and schedule must match.
    pub fn is_valid_now(&self, day_name: &str, hour: u8) -> bool {
        self.enabled && self.mp3_file.exists() && self.is_scheduled_for(day_name, hour)
    }

    /// Format days for display.
    pub fn days_display(&self) -> String {
        if self.days.is_empty() {
            "all".to_string()
        } else {
            self.days.join(",")
        }
    }

    /// Format hours for display.
    pub fn hours_display(&self) -> String {
        if self.hours.is_empty() {
            "all".to_string()
        } else {
            self.hours
                .iter()
                .map(|h| format!("{}", h))
                .collect::<Vec<_>>()
                .join(",")
        }
    }
}

/// Settings for the ad inserter service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdInserterSettings {
    /// Path for the concatenated ad roll output file.
    #[serde(default = "default_output_mp3")]
    pub output_mp3: PathBuf,
    /// Whether to prepend station ID at the top of the hour.
    #[serde(default)]
    pub station_id_enabled: bool,
    /// Path to the station ID audio file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station_id_file: Option<PathBuf>,
}

fn default_output_mp3() -> PathBuf {
    PathBuf::from("adRoll.mp3")
}

impl Default for AdInserterSettings {
    fn default() -> Self {
        AdInserterSettings {
            output_mp3: default_output_mp3(),
            station_id_enabled: false,
            station_id_file: None,
        }
    }
}

// --- Ad Insertion Mode ---

/// How the ad should be inserted into playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdInsertionMode {
    /// Queue ad roll as next track, wait for engine to start playing it.
    Scheduled,
    /// Immediately stop current audio and play ad roll.
    Instant,
}

// --- Scheduler Decision Result ---

/// Result of the lecture check decision flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerDecision {
    /// Insert ads immediately (instant mode).
    InsertInstant,
    /// Insert ads as next track (scheduled mode, wait for track boundary).
    InsertScheduled,
    /// Wait for next track change and re-evaluate.
    WaitForTrackBoundary,
    /// Skip — don't insert ads this check (e.g., playlist ended).
    Skip,
}

// --- Track Info (passed to decision logic) ---

/// Snapshot of track info needed for scheduling decisions.
#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub artist: String,
    pub title: String,
    /// When the track started playing.
    pub start_time: Instant,
    /// Total duration of the track.
    pub duration: Duration,
}

impl TrackInfo {
    /// Estimated end time of the current track.
    pub fn estimated_end(&self) -> Instant {
        self.start_time + self.duration
    }

    /// Format as "artist - title" for comparison.
    pub fn identity(&self) -> String {
        format!("{} - {}", self.artist, self.title)
    }
}

// --- Time Calculation Helpers ---

/// Minutes remaining in the current hour.
pub fn minutes_remaining_in_hour() -> f64 {
    let now = Local::now();
    let seconds_left = 3600.0 - (now.minute() * 60 + now.second()) as f64;
    seconds_left / 60.0
}

/// Seconds until the next hour boundary (:00:00).
pub fn seconds_until_next_hour() -> u64 {
    let now = Local::now();
    3600 - (now.minute() * 60 + now.second()) as u64
}

/// Check if a track that started at `start_time` with the given `duration`
/// will end before the current hour ends.
pub fn track_ends_this_hour(start_time: Instant, duration: Duration) -> bool {
    let now = Instant::now();
    let remaining_track_secs = if start_time + duration > now {
        (start_time + duration - now).as_secs_f64()
    } else {
        0.0
    };
    remaining_track_secs <= seconds_until_next_hour() as f64
}

/// Minutes remaining after the current track ends (within this hour).
pub fn minutes_remaining_after_track(start_time: Instant, duration: Duration) -> f64 {
    let now = Instant::now();
    let track_remaining_secs = if start_time + duration > now {
        (start_time + duration - now).as_secs_f64()
    } else {
        0.0
    };
    let hour_remaining_secs = seconds_until_next_hour() as f64;
    (hour_remaining_secs - track_remaining_secs) / 60.0
}

/// Check if we're in the first 5 seconds of the hour.
pub fn is_hour_start() -> bool {
    let now = Local::now();
    now.minute() == 0 && now.second() < 5
}

/// Get the current day name (e.g., "Sunday", "Monday", etc.).
pub fn current_day_name() -> String {
    Local::now().format("%A").to_string()
}

/// Get the current hour (0-23).
pub fn current_hour() -> u8 {
    Local::now().hour() as u8
}

// --- Decision Logic (pure functions) ---

/// Core decision flow for ad scheduling.
///
/// This implements the lecture check decision flow from the spec (CHECK 0-3).
/// All inputs are passed as parameters so this function is pure and testable.
pub fn decide_ad_insertion(
    has_next_track: bool,
    current_track: Option<&TrackInfo>,
    next_track_artist: Option<&str>,
    lecture_detector: &LectureDetector,
    minutes_remaining: f64,
    track_ends_in_hour: bool,
    minutes_after_track: f64,
) -> SchedulerDecision {
    // CHECK 0: Playlist end detection
    if !has_next_track {
        return SchedulerDecision::Skip;
    }

    // CHECK 1: Safety margin (< 3 minutes left in hour)
    if minutes_remaining < SAFETY_MARGIN_MINUTES {
        return SchedulerDecision::InsertInstant;
    }

    // Need current track info for remaining checks
    let _current = match current_track {
        Some(t) => t,
        None => return SchedulerDecision::Skip,
    };

    // CHECK 2: Does current track end this hour?
    if !track_ends_in_hour {
        // Track extends into next hour -> instant insertion
        return SchedulerDecision::InsertInstant;
    }

    // CHECK 3: Is next track a lecture?
    match next_track_artist {
        Some(next_artist) => {
            if lecture_detector.is_lecture(next_artist) {
                // Next is lecture: check if it will start within this hour
                if minutes_after_track > 0.0 {
                    // Lecture starts this hour -> scheduled insertion (wait for boundary)
                    SchedulerDecision::InsertScheduled
                } else {
                    // Lecture would start next hour -> instant
                    SchedulerDecision::InsertInstant
                }
            } else {
                // Next is NOT lecture
                if minutes_after_track < SAFETY_MARGIN_MINUTES {
                    // Too risky to wait -> instant
                    SchedulerDecision::InsertInstant
                } else {
                    // Safe to wait for track boundary
                    SchedulerDecision::WaitForTrackBoundary
                }
            }
        }
        None => {
            // No next track info available -> fallback to instant
            SchedulerDecision::InsertInstant
        }
    }
}

// --- AdSchedulerHandler ---

/// The ad scheduler handler runs as a background thread, checking hourly
/// boundaries and track changes to determine when to insert ads.
pub struct AdSchedulerHandler {
    running: Arc<AtomicBool>,
    last_hour_checked: Arc<Mutex<u32>>,
    last_track_check: Arc<Mutex<Instant>>,
    last_seen_track: Arc<Mutex<Option<String>>>,
    waiting_for_track_boundary: Arc<AtomicBool>,
    pending_lecture_check: Arc<AtomicBool>,
    is_hour_start_flag: Arc<AtomicBool>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl AdSchedulerHandler {
    /// Create a new handler (not yet started).
    pub fn new() -> Self {
        AdSchedulerHandler {
            running: Arc::new(AtomicBool::new(false)),
            last_hour_checked: Arc::new(Mutex::new(u32::MAX)),
            last_track_check: Arc::new(Mutex::new(Instant::now())),
            last_seen_track: Arc::new(Mutex::new(None)),
            waiting_for_track_boundary: Arc::new(AtomicBool::new(false)),
            pending_lecture_check: Arc::new(AtomicBool::new(false)),
            is_hour_start_flag: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
        }
    }

    /// Whether the scheduler is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Start the scheduler background thread.
    ///
    /// The `get_state` callback is called each iteration to get current state.
    /// The `on_insert` callback is called when ads should be inserted.
    pub fn start<F, G>(&mut self, get_state: F, on_insert: G)
    where
        F: Fn() -> Option<SchedulerState> + Send + 'static,
        G: Fn(AdInsertionMode, bool) + Send + 'static,
    {
        if self.is_running() {
            return;
        }
        self.running.store(true, Ordering::Relaxed);

        let running = self.running.clone();
        let last_hour_checked = self.last_hour_checked.clone();
        let last_track_check = self.last_track_check.clone();
        let last_seen_track = self.last_seen_track.clone();
        let waiting_for_boundary = self.waiting_for_track_boundary.clone();
        let pending_check = self.pending_lecture_check.clone();
        let hour_start_flag = self.is_hour_start_flag.clone();

        let handle = thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    scheduler_tick(
                        &get_state,
                        &on_insert,
                        &last_hour_checked,
                        &last_track_check,
                        &last_seen_track,
                        &waiting_for_boundary,
                        &pending_check,
                        &hour_start_flag,
                    );
                }));

                if result.is_err() {
                    eprintln!("[AdScheduler] Error in tick, retrying in {}s", ERROR_RETRY_DELAY);
                    thread::sleep(Duration::from_secs(ERROR_RETRY_DELAY));
                    continue;
                }

                // Dynamic sleep calculation
                let secs_to_hour = seconds_until_next_hour() + 2;
                let last_check = last_track_check.lock().unwrap();
                let since_check = last_check.elapsed().as_secs();
                drop(last_check);

                let track_check_remaining = if since_check < TRACK_CHANGE_CHECK_INTERVAL {
                    TRACK_CHANGE_CHECK_INTERVAL - since_check
                } else {
                    0
                };

                let needs_track_check = waiting_for_boundary.load(Ordering::Relaxed)
                    || pending_check.load(Ordering::Relaxed);

                let sleep_time = if needs_track_check {
                    track_check_remaining.min(LOOP_SLEEP).min(secs_to_hour).max(1)
                } else {
                    LOOP_SLEEP.min(secs_to_hour).max(1)
                };

                thread::sleep(Duration::from_secs(sleep_time));
            }
        });

        self.thread_handle = Some(handle);
    }

    /// Stop the scheduler and wait for the thread to finish.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AdSchedulerHandler {
    fn drop(&mut self) {
        self.stop();
    }
}

/// State snapshot provided to the scheduler each tick.
pub struct SchedulerState {
    pub current_track: Option<TrackInfo>,
    pub next_track_artist: Option<String>,
    pub has_next_track: bool,
    pub lecture_detector: LectureDetector,
    pub ads: Vec<AdConfig>,
}

/// One iteration of the scheduler main loop.
fn scheduler_tick<F, G>(
    get_state: &F,
    on_insert: &G,
    last_hour_checked: &Mutex<u32>,
    last_track_check: &Mutex<Instant>,
    last_seen_track: &Mutex<Option<String>>,
    waiting_for_boundary: &AtomicBool,
    pending_check: &AtomicBool,
    hour_start_flag: &AtomicBool,
) where
    F: Fn() -> Option<SchedulerState>,
    G: Fn(AdInsertionMode, bool),
{
    let now_hour = current_hour() as u32;
    let day = current_day_name();

    // HOUR BOUNDARY CHECK
    {
        let mut last = last_hour_checked.lock().unwrap();
        if now_hour != *last {
            *last = now_hour;
            hour_start_flag.store(is_hour_start(), Ordering::Relaxed);

            // Get state and run decision
            if let Some(state) = get_state() {
                // Check if any ads are scheduled for this hour
                let has_valid_ads = state.ads.iter().any(|a| a.is_valid_now(&day, now_hour as u8));
                if has_valid_ads {
                    run_lecture_check(
                        &state,
                        on_insert,
                        waiting_for_boundary,
                        pending_check,
                        hour_start_flag.load(Ordering::Relaxed),
                    );
                }
            }

            hour_start_flag.store(false, Ordering::Relaxed);
        }
    }

    // TRACK CHANGE CHECK
    let needs_check = waiting_for_boundary.load(Ordering::Relaxed)
        || pending_check.load(Ordering::Relaxed);
    if needs_check {
        let mut check_time = last_track_check.lock().unwrap();
        if check_time.elapsed() >= Duration::from_secs(TRACK_CHANGE_CHECK_INTERVAL) {
            *check_time = Instant::now();
            drop(check_time);

            if let Some(state) = get_state() {
                let current_identity = state.current_track.as_ref().map(|t| t.identity());
                let mut last_track = last_seen_track.lock().unwrap();

                if current_identity != *last_track {
                    *last_track = current_identity;
                    drop(last_track);

                    // Track changed — run lecture check
                    run_lecture_check(
                        &state,
                        on_insert,
                        waiting_for_boundary,
                        pending_check,
                        false,
                    );
                }
            }
        }
    }
}

/// Execute the lecture check decision flow and act on the result.
fn run_lecture_check<G>(
    state: &SchedulerState,
    on_insert: &G,
    waiting_for_boundary: &AtomicBool,
    pending_check: &AtomicBool,
    is_hour_start: bool,
) where
    G: Fn(AdInsertionMode, bool),
{
    let mins_remaining = minutes_remaining_in_hour();
    let track_ends = state.current_track.as_ref().map_or(true, |t| {
        track_ends_this_hour(t.start_time, t.duration)
    });
    let mins_after = state.current_track.as_ref().map_or(0.0, |t| {
        minutes_remaining_after_track(t.start_time, t.duration)
    });

    let decision = decide_ad_insertion(
        state.has_next_track,
        state.current_track.as_ref(),
        state.next_track_artist.as_deref(),
        &state.lecture_detector,
        mins_remaining,
        track_ends,
        mins_after,
    );

    match decision {
        SchedulerDecision::InsertInstant => {
            waiting_for_boundary.store(false, Ordering::Relaxed);
            pending_check.store(false, Ordering::Relaxed);
            on_insert(AdInsertionMode::Instant, is_hour_start);
        }
        SchedulerDecision::InsertScheduled => {
            waiting_for_boundary.store(false, Ordering::Relaxed);
            pending_check.store(false, Ordering::Relaxed);
            on_insert(AdInsertionMode::Scheduled, is_hour_start);
        }
        SchedulerDecision::WaitForTrackBoundary => {
            waiting_for_boundary.store(true, Ordering::Relaxed);
            pending_check.store(true, Ordering::Relaxed);
        }
        SchedulerDecision::Skip => {
            waiting_for_boundary.store(false, Ordering::Relaxed);
            pending_check.store(false, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- AdConfig tests ---

    #[test]
    fn ad_config_new_defaults() {
        let ad = AdConfig::new("Test Ad".into(), "ad.mp3".into());
        assert!(ad.enabled);
        assert!(!ad.scheduled);
        assert!(ad.days.is_empty());
        assert!(ad.hours.is_empty());
    }

    #[test]
    fn ad_config_unscheduled_always_matches() {
        let ad = AdConfig::new("Test".into(), "ad.mp3".into());
        assert!(ad.is_scheduled_for("Monday", 14));
        assert!(ad.is_scheduled_for("Sunday", 0));
    }

    #[test]
    fn ad_config_scheduled_day_filter() {
        let ad = AdConfig {
            name: "Test".into(),
            enabled: true,
            mp3_file: "ad.mp3".into(),
            scheduled: true,
            days: vec!["Monday".into(), "Wednesday".into()],
            hours: vec![],
        };
        assert!(ad.is_scheduled_for("Monday", 10));
        assert!(ad.is_scheduled_for("wednesday", 10));
        assert!(!ad.is_scheduled_for("Tuesday", 10));
    }

    #[test]
    fn ad_config_scheduled_hour_filter() {
        let ad = AdConfig {
            name: "Test".into(),
            enabled: true,
            mp3_file: "ad.mp3".into(),
            scheduled: true,
            days: vec![],
            hours: vec![9, 10, 14, 15],
        };
        assert!(ad.is_scheduled_for("Monday", 9));
        assert!(ad.is_scheduled_for("Monday", 14));
        assert!(!ad.is_scheduled_for("Monday", 12));
    }

    #[test]
    fn ad_config_scheduled_day_and_hour() {
        let ad = AdConfig {
            name: "Test".into(),
            enabled: true,
            mp3_file: "ad.mp3".into(),
            scheduled: true,
            days: vec!["Monday".into()],
            hours: vec![9],
        };
        assert!(ad.is_scheduled_for("Monday", 9));
        assert!(!ad.is_scheduled_for("Monday", 10));
        assert!(!ad.is_scheduled_for("Tuesday", 9));
    }

    #[test]
    fn ad_config_serialization_roundtrip() {
        let ad = AdConfig {
            name: "Test Ad".into(),
            enabled: true,
            mp3_file: "G:\\Ads\\test.mp3".into(),
            scheduled: true,
            days: vec!["Monday".into(), "Friday".into()],
            hours: vec![9, 10, 14],
        };
        let json = serde_json::to_string(&ad).unwrap();
        let loaded: AdConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.name, "Test Ad");
        assert!(loaded.scheduled);
        assert_eq!(loaded.days.len(), 2);
        assert_eq!(loaded.hours, vec![9, 10, 14]);
    }

    #[test]
    fn ad_config_days_display() {
        let ad = AdConfig::new("Test".into(), "ad.mp3".into());
        assert_eq!(ad.days_display(), "all");

        let ad2 = AdConfig {
            days: vec!["Mon".into(), "Fri".into()],
            ..ad
        };
        assert_eq!(ad2.days_display(), "Mon,Fri");
    }

    #[test]
    fn ad_config_hours_display() {
        let ad = AdConfig::new("Test".into(), "ad.mp3".into());
        assert_eq!(ad.hours_display(), "all");

        let ad2 = AdConfig {
            hours: vec![9, 14],
            ..ad
        };
        assert_eq!(ad2.hours_display(), "9,14");
    }

    // --- AdInserterSettings tests ---

    #[test]
    fn ad_inserter_settings_defaults() {
        let settings = AdInserterSettings::default();
        assert_eq!(settings.output_mp3, PathBuf::from("adRoll.mp3"));
        assert!(!settings.station_id_enabled);
        assert!(settings.station_id_file.is_none());
    }

    #[test]
    fn ad_inserter_settings_serialization() {
        let settings = AdInserterSettings {
            output_mp3: "out.mp3".into(),
            station_id_enabled: true,
            station_id_file: Some("station.mp3".into()),
        };
        let json = serde_json::to_string(&settings).unwrap();
        let loaded: AdInserterSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.output_mp3, PathBuf::from("out.mp3"));
        assert!(loaded.station_id_enabled);
        assert_eq!(loaded.station_id_file, Some(PathBuf::from("station.mp3")));
    }

    // --- Time calculation tests ---

    #[test]
    fn seconds_until_next_hour_is_positive() {
        let secs = seconds_until_next_hour();
        assert!(secs > 0);
        assert!(secs <= 3600);
    }

    #[test]
    fn minutes_remaining_is_positive() {
        let mins = minutes_remaining_in_hour();
        assert!(mins > 0.0);
        assert!(mins <= 60.0);
    }

    #[test]
    fn track_ends_this_hour_short_track() {
        // A 30-second track that just started should end this hour
        // (unless we're in the last 30 seconds of the hour)
        let start = Instant::now();
        let dur = Duration::from_secs(30);
        // This will almost always be true
        if minutes_remaining_in_hour() > 1.0 {
            assert!(track_ends_this_hour(start, dur));
        }
    }

    #[test]
    fn minutes_after_track_positive_for_short_track() {
        let start = Instant::now();
        let dur = Duration::from_secs(30);
        if minutes_remaining_in_hour() > 1.0 {
            assert!(minutes_remaining_after_track(start, dur) > 0.0);
        }
    }

    // --- Decision Logic tests ---

    fn make_track_info(artist: &str, title: &str, duration_secs: u64) -> TrackInfo {
        TrackInfo {
            artist: artist.into(),
            title: title.into(),
            start_time: Instant::now(),
            duration: Duration::from_secs(duration_secs),
        }
    }

    #[test]
    fn decision_skip_when_no_next_track() {
        let ld = LectureDetector::new();
        let track = make_track_info("Artist", "Song", 180);
        let decision = decide_ad_insertion(
            false, // no next track
            Some(&track),
            None,
            &ld,
            30.0, // plenty of time
            true,
            25.0,
        );
        assert_eq!(decision, SchedulerDecision::Skip);
    }

    #[test]
    fn decision_instant_when_safety_margin() {
        let ld = LectureDetector::new();
        let track = make_track_info("Artist", "Song", 180);
        let decision = decide_ad_insertion(
            true,
            Some(&track),
            Some("Next Artist"),
            &ld,
            2.0, // < 3 min safety margin
            true,
            1.0,
        );
        assert_eq!(decision, SchedulerDecision::InsertInstant);
    }

    #[test]
    fn decision_instant_when_track_extends_past_hour() {
        let ld = LectureDetector::new();
        let track = make_track_info("Artist", "Song", 180);
        let decision = decide_ad_insertion(
            true,
            Some(&track),
            Some("Next"),
            &ld,
            30.0,
            false, // track does NOT end this hour
            0.0,
        );
        assert_eq!(decision, SchedulerDecision::InsertInstant);
    }

    #[test]
    fn decision_scheduled_when_next_is_lecture_starts_this_hour() {
        let ld = LectureDetector::new();
        let track = make_track_info("Artist", "Song", 60);
        let decision = decide_ad_insertion(
            true,
            Some(&track),
            Some("Rabbi Shalom"), // starts with R = lecture
            &ld,
            30.0,
            true,
            25.0, // plenty of time after track
        );
        assert_eq!(decision, SchedulerDecision::InsertScheduled);
    }

    #[test]
    fn decision_instant_when_next_is_lecture_but_no_time() {
        let ld = LectureDetector::new();
        let track = make_track_info("Artist", "Song", 60);
        let decision = decide_ad_insertion(
            true,
            Some(&track),
            Some("Rabbi Shalom"),
            &ld,
            30.0,
            true,
            0.0, // no time remaining after track
        );
        assert_eq!(decision, SchedulerDecision::InsertInstant);
    }

    #[test]
    fn decision_wait_when_next_not_lecture_and_time_available() {
        let ld = LectureDetector::new();
        let track = make_track_info("Artist", "Song", 60);
        let decision = decide_ad_insertion(
            true,
            Some(&track),
            Some("Beatles"), // not a lecture
            &ld,
            30.0,
            true,
            10.0, // plenty of time
        );
        assert_eq!(decision, SchedulerDecision::WaitForTrackBoundary);
    }

    #[test]
    fn decision_instant_when_next_not_lecture_but_low_time() {
        let ld = LectureDetector::new();
        let track = make_track_info("Artist", "Song", 60);
        let decision = decide_ad_insertion(
            true,
            Some(&track),
            Some("Beatles"),
            &ld,
            30.0,
            true,
            2.0, // < 3 min after track
        );
        assert_eq!(decision, SchedulerDecision::InsertInstant);
    }

    #[test]
    fn decision_skip_when_no_current_track() {
        let ld = LectureDetector::new();
        let decision = decide_ad_insertion(
            true,
            None, // no current track
            Some("Next"),
            &ld,
            30.0,
            true,
            25.0,
        );
        assert_eq!(decision, SchedulerDecision::Skip);
    }

    #[test]
    fn decision_respects_blacklist() {
        let mut ld = LectureDetector::new();
        ld.add_blacklist("Rihanna");
        let track = make_track_info("Artist", "Song", 60);
        let decision = decide_ad_insertion(
            true,
            Some(&track),
            Some("Rihanna"), // blacklisted, NOT a lecture
            &ld,
            30.0,
            true,
            10.0,
        );
        // Rihanna is not a lecture, so with enough time we wait
        assert_eq!(decision, SchedulerDecision::WaitForTrackBoundary);
    }

    // --- Handler tests ---

    #[test]
    fn handler_starts_and_stops() {
        let mut handler = AdSchedulerHandler::new();
        assert!(!handler.is_running());

        handler.start(|| None, |_, _| {});
        assert!(handler.is_running());

        handler.stop();
        assert!(!handler.is_running());
    }
}
