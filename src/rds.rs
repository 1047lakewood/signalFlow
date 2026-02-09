use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Timeout for TCP socket connect and read (seconds).
const SOCKET_TIMEOUT_SECS: u64 = 10;
/// Delay after each RDS command send (seconds).
const COMMAND_DELAY_MS: u64 = 200;
/// Main loop sleep between iterations (seconds).
const LOOP_SLEEP_MS: u64 = 1000;
/// Wait after a fatal loop error before retry (seconds).
const ERROR_RETRY_DELAY_SECS: u64 = 15;
/// Resend same message to maintain encoder state (seconds).
const KEEPALIVE_INTERVAL_SECS: u64 = 60;
/// Maximum RDS text length.
const MAX_RDS_TEXT_LEN: usize = 64;

/// A single RDS message configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdsMessage {
    /// Message text (max 64 chars, may contain {artist} and {title} placeholders).
    pub text: String,
    /// Whether this message is active.
    pub enabled: bool,
    /// Display duration in seconds (1–60).
    #[serde(default = "default_duration")]
    pub duration: u32,
    /// Schedule settings (optional day/hour restrictions).
    #[serde(default)]
    pub scheduled: RdsSchedule,
}

fn default_duration() -> u32 {
    10
}

/// Schedule settings for an RDS message.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RdsSchedule {
    /// Whether scheduling is active for this message.
    pub enabled: bool,
    /// Day names (e.g., "Sunday", "Monday"). Empty = all days.
    #[serde(default)]
    pub days: Vec<String>,
    /// Hours (0–23). Empty = all hours.
    #[serde(default)]
    pub hours: Vec<u8>,
}

fn default_rds_port() -> u16 {
    10001
}

fn default_rds_ip() -> String {
    "127.0.0.1".to_string()
}

fn default_rds_message() -> String {
    "signalFlow Radio Automation".to_string()
}

/// RDS configuration stored in Engine state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdsConfig {
    /// RDS encoder IP address.
    #[serde(default = "default_rds_ip")]
    pub ip: String,
    /// RDS encoder TCP port.
    #[serde(default = "default_rds_port")]
    pub port: u16,
    /// Default message when no valid messages are available.
    #[serde(default = "default_rds_message")]
    pub default_message: String,
    /// Configured RDS messages.
    #[serde(default)]
    pub messages: Vec<RdsMessage>,
}

impl Default for RdsConfig {
    fn default() -> Self {
        RdsConfig {
            ip: default_rds_ip(),
            port: default_rds_port(),
            default_message: default_rds_message(),
            messages: Vec::new(),
        }
    }
}

impl RdsConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RdsMessage {
    /// Create a new default message.
    pub fn new(text: &str) -> Self {
        RdsMessage {
            text: text.to_string(),
            enabled: false,
            duration: 10,
            scheduled: RdsSchedule::default(),
        }
    }

    /// Display-friendly schedule summary.
    pub fn days_display(&self) -> String {
        if !self.scheduled.enabled || self.scheduled.days.is_empty() {
            "All".to_string()
        } else {
            self.scheduled.days.join(",")
        }
    }

    /// Display-friendly hours summary.
    pub fn hours_display(&self) -> String {
        if !self.scheduled.enabled || self.scheduled.hours.is_empty() {
            "All".to_string()
        } else {
            self.scheduled
                .hours
                .iter()
                .map(|h| format_hour_ampm(*h))
                .collect::<Vec<_>>()
                .join(",")
        }
    }
}

/// Format an hour (0–23) as AM/PM string.
pub fn format_hour_ampm(hour: u8) -> String {
    match hour {
        0 => "12AM".to_string(),
        1..=11 => format!("{}AM", hour),
        12 => "12PM".to_string(),
        13..=23 => format!("{}PM", hour - 12),
        _ => format!("{}?", hour),
    }
}

/// Sanitize text for RDS protocol: remove newlines, trim, truncate to 64 chars.
pub fn sanitize_rds_text(text: &str, default_message: &str) -> String {
    let cleaned = text.replace('\r', " ").replace('\n', " ");
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        let fallback = &default_message[..default_message.len().min(MAX_RDS_TEXT_LEN)];
        return fallback.to_string();
    }
    if trimmed.len() > MAX_RDS_TEXT_LEN {
        trimmed[..MAX_RDS_TEXT_LEN].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Replace placeholders in message text.
/// {artist} -> UPPERCASE, {title} -> as-is.
pub fn format_message_text(text: &str, artist: &str, title: &str) -> String {
    let result = text
        .replace("{artist}", &artist.to_uppercase())
        .replace("{title}", title);
    result.trim().to_string()
}

/// Check whether a message should be displayed based on filtering rules.
///
/// Checks in order: enabled, lecture detection, placeholder availability, schedule.
pub fn should_display_message(
    message: &RdsMessage,
    artist: &str,
    title: &str,
    is_lecture: bool,
    current_day: &str,
    current_hour: u8,
) -> bool {
    // Check 1: Enabled
    if !message.enabled {
        return false;
    }

    // Check 2: Lecture detection (only affects messages containing {artist})
    // If current track is NOT a lecture AND message contains {artist}: SKIP
    if !is_lecture && message.text.contains("{artist}") {
        return false;
    }

    // Check 3: Placeholder availability
    if message.text.contains("{artist}") && artist.is_empty() {
        return false;
    }
    if message.text.contains("{title}") && title.is_empty() {
        return false;
    }

    // Check 4: Schedule
    if message.scheduled.enabled {
        if !message.scheduled.days.is_empty() {
            let day_lower = current_day.to_lowercase();
            let matches = message
                .scheduled
                .days
                .iter()
                .any(|d| d.to_lowercase() == day_lower);
            if !matches {
                return false;
            }
        }
        if !message.scheduled.hours.is_empty() && !message.scheduled.hours.contains(&current_hour) {
            return false;
        }
    }

    true
}

/// Send a DPSTEXT command to the RDS encoder via TCP.
/// Returns Ok(response) on success, Err(message) on failure.
pub fn send_message_to_rds(ip: &str, port: u16, text: &str) -> Result<String, String> {
    let addr = format!("{}:{}", ip, port);
    let timeout = Duration::from_secs(SOCKET_TIMEOUT_SECS);

    let mut stream = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|e| format!("Invalid address {}: {}", addr, e))?,
        timeout,
    )
    .map_err(|e| format!("TCP connect to {} failed: {}", addr, e))?;

    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("Set read timeout: {}", e))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| format!("Set write timeout: {}", e))?;

    let command = format!("DPSTEXT={}\r\n", text);
    stream
        .write_all(command.as_bytes())
        .map_err(|e| format!("TCP write failed: {}", e))?;

    let mut buf = [0u8; 1024];
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("TCP read failed: {}", e))?;
    let response = String::from_utf8_lossy(&buf[..n]).to_string();

    if response.is_empty() || response.starts_with("Error:") {
        Err(format!("RDS error response: {}", response))
    } else {
        Ok(response)
    }
}

/// Current day name (e.g., "Sunday", "Monday").
pub fn current_day_name() -> String {
    chrono::Local::now().format("%A").to_string()
}

/// Current hour (0–23).
pub fn current_hour() -> u8 {
    chrono::Local::now().format("%H").to_string().parse().unwrap_or(0)
}

/// Status of the RDS handler.
#[derive(Debug, Clone)]
pub struct RdsStatus {
    pub running: bool,
    pub last_sent_text: Option<String>,
    pub last_send_status: Option<String>,
    pub message_index: usize,
}

/// The RDS message rotation handler.
///
/// Manages a background thread that rotates through configured RDS messages,
/// sending them to an RDS encoder via TCP socket using DPSTEXT commands.
#[allow(dead_code)]
pub struct RdsHandler {
    running: Arc<AtomicBool>,
    message_index: Arc<AtomicUsize>,
    current_message_duration: Arc<AtomicU32>,
    last_sent_text: Arc<Mutex<Option<String>>>,
    last_send_time: Arc<Mutex<Instant>>,
    last_send_status: Arc<Mutex<Option<String>>>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl RdsHandler {
    /// Create and start the RDS handler background thread.
    ///
    /// The handler queries `get_config` for RDS settings and `get_now_playing`
    /// for current track info on each loop iteration.
    pub fn start<F, G>(get_config: F, get_now_playing: G) -> Self
    where
        F: Fn() -> (RdsConfig, bool) + Send + 'static, // returns (config, is_lecture)
        G: Fn() -> (String, String) + Send + 'static,  // returns (artist, title)
    {
        let running = Arc::new(AtomicBool::new(true));
        let message_index = Arc::new(AtomicUsize::new(0));
        let current_message_duration = Arc::new(AtomicU32::new(10));
        let last_sent_text: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let last_send_time = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(KEEPALIVE_INTERVAL_SECS + 1)));
        let last_send_status: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let r = running.clone();
        let mi = message_index.clone();
        let cmd = current_message_duration.clone();
        let lst = last_sent_text.clone();
        let ltime = last_send_time.clone();
        let lstat = last_send_status.clone();

        let handle = std::thread::spawn(move || {
            Self::run_loop(r, mi, cmd, lst, ltime, lstat, get_config, get_now_playing);
        });

        RdsHandler {
            running,
            message_index,
            current_message_duration,
            last_sent_text,
            last_send_time,
            last_send_status,
            thread_handle: Some(handle),
        }
    }

    fn run_loop<F, G>(
        running: Arc<AtomicBool>,
        message_index: Arc<AtomicUsize>,
        current_message_duration: Arc<AtomicU32>,
        last_sent_text: Arc<Mutex<Option<String>>>,
        last_send_time: Arc<Mutex<Instant>>,
        last_send_status: Arc<Mutex<Option<String>>>,
        get_config: F,
        get_now_playing: G,
    ) where
        F: Fn() -> (RdsConfig, bool),
        G: Fn() -> (String, String),
    {
        while running.load(Ordering::Relaxed) {
            match Self::loop_iteration(
                &message_index,
                &current_message_duration,
                &last_sent_text,
                &last_send_time,
                &last_send_status,
                &get_config,
                &get_now_playing,
            ) {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("[RDS] Error: {}", e);
                    *last_send_status.lock().unwrap() = Some(format!("error: {}", e));
                    std::thread::sleep(Duration::from_secs(ERROR_RETRY_DELAY_SECS));
                    continue;
                }
            }
            std::thread::sleep(Duration::from_millis(LOOP_SLEEP_MS));
        }
    }

    fn loop_iteration<F, G>(
        message_index: &Arc<AtomicUsize>,
        current_message_duration: &Arc<AtomicU32>,
        last_sent_text: &Arc<Mutex<Option<String>>>,
        last_send_time: &Arc<Mutex<Instant>>,
        last_send_status: &Arc<Mutex<Option<String>>>,
        get_config: &F,
        get_now_playing: &G,
    ) -> Result<(), String>
    where
        F: Fn() -> (RdsConfig, bool),
        G: Fn() -> (String, String),
    {
        let (config, is_lecture) = get_config();
        let (artist, title) = get_now_playing();
        let day = current_day_name();
        let hour = current_hour();

        // Filter valid messages
        let valid_messages: Vec<&RdsMessage> = config
            .messages
            .iter()
            .filter(|m| should_display_message(m, &artist, &title, is_lecture, &day, hour))
            .collect();

        // Determine display text and duration
        let (display_text, duration) = if valid_messages.is_empty() {
            (config.default_message.clone(), 10u32)
        } else {
            let idx = message_index.load(Ordering::Relaxed) % valid_messages.len();
            let msg = valid_messages[idx];
            let formatted = format_message_text(&msg.text, &artist, &title);
            if formatted.is_empty() {
                // Advance index, skip this iteration
                message_index.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
            (formatted, msg.duration)
        };

        let sanitized = sanitize_rds_text(&display_text, &config.default_message);

        // Check timing
        let now = Instant::now();
        let elapsed = {
            let lt = last_send_time.lock().unwrap();
            now.duration_since(*lt)
        };
        let msg_dur = Duration::from_secs(current_message_duration.load(Ordering::Relaxed) as u64);
        let rotation_due = elapsed >= msg_dur;
        let same_text = {
            let lst = last_sent_text.lock().unwrap();
            lst.as_deref() == Some(&sanitized)
        };
        let keepalive_due = same_text && elapsed >= Duration::from_secs(KEEPALIVE_INTERVAL_SECS);
        let should_send = rotation_due || keepalive_due;

        if should_send {
            let result = send_message_to_rds(&config.ip, config.port, &sanitized);
            match &result {
                Ok(_) => {
                    *last_send_status.lock().unwrap() = Some("success".to_string());
                }
                Err(e) => {
                    *last_send_status.lock().unwrap() = Some(format!("error: {}", e));
                }
            }
            *last_sent_text.lock().unwrap() = Some(sanitized);
            *last_send_time.lock().unwrap() = now;
            current_message_duration.store(duration, Ordering::Relaxed);

            // Only advance index on rotation, not keepalive
            if rotation_due && !valid_messages.is_empty() {
                message_index.fetch_add(1, Ordering::Relaxed);
            }

            std::thread::sleep(Duration::from_millis(COMMAND_DELAY_MS));
        }

        Ok(())
    }

    /// Stop the handler and join the background thread.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if the handler is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get current handler status.
    pub fn status(&self) -> RdsStatus {
        RdsStatus {
            running: self.running.load(Ordering::Relaxed),
            last_sent_text: self.last_sent_text.lock().unwrap().clone(),
            last_send_status: self.last_send_status.lock().unwrap().clone(),
            message_index: self.message_index.load(Ordering::Relaxed),
        }
    }
}

impl Drop for RdsHandler {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- RdsMessage tests ---

    #[test]
    fn rds_message_new_defaults() {
        let msg = RdsMessage::new("Hello World");
        assert_eq!(msg.text, "Hello World");
        assert!(!msg.enabled);
        assert_eq!(msg.duration, 10);
        assert!(!msg.scheduled.enabled);
    }

    #[test]
    fn rds_message_serialization_roundtrip() {
        let msg = RdsMessage {
            text: "Test {artist}".to_string(),
            enabled: true,
            duration: 15,
            scheduled: RdsSchedule {
                enabled: true,
                days: vec!["Monday".to_string(), "Friday".to_string()],
                hours: vec![9, 10, 14],
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let loaded: RdsMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.text, "Test {artist}");
        assert!(loaded.enabled);
        assert_eq!(loaded.duration, 15);
        assert!(loaded.scheduled.enabled);
        assert_eq!(loaded.scheduled.days.len(), 2);
        assert_eq!(loaded.scheduled.hours.len(), 3);
    }

    #[test]
    fn rds_message_defaults_when_missing_fields() {
        let json = r#"{"text":"Hi","enabled":true}"#;
        let msg: RdsMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.duration, 10);
        assert!(!msg.scheduled.enabled);
        assert!(msg.scheduled.days.is_empty());
    }

    // --- RdsConfig tests ---

    #[test]
    fn rds_config_defaults() {
        let config = RdsConfig::default();
        assert_eq!(config.ip, "127.0.0.1");
        assert_eq!(config.port, 10001);
        assert_eq!(config.default_message, "signalFlow Radio Automation");
        assert!(config.messages.is_empty());
    }

    #[test]
    fn rds_config_serialization_roundtrip() {
        let mut config = RdsConfig::default();
        config.ip = "192.168.1.100".to_string();
        config.port = 5000;
        config.messages.push(RdsMessage::new("Test"));
        let json = serde_json::to_string(&config).unwrap();
        let loaded: RdsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.ip, "192.168.1.100");
        assert_eq!(loaded.port, 5000);
        assert_eq!(loaded.messages.len(), 1);
    }

    #[test]
    fn rds_config_defaults_when_missing_from_json() {
        let json = "{}";
        let config: RdsConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.ip, "127.0.0.1");
        assert_eq!(config.port, 10001);
    }

    // --- sanitize_rds_text tests ---

    #[test]
    fn sanitize_removes_newlines() {
        let result = sanitize_rds_text("Hello\r\nWorld\n!", "default");
        assert_eq!(result, "Hello  World !");
    }

    #[test]
    fn sanitize_truncates_to_64() {
        let long = "A".repeat(100);
        let result = sanitize_rds_text(&long, "default");
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn sanitize_empty_uses_default() {
        let result = sanitize_rds_text("", "My Default");
        assert_eq!(result, "My Default");
    }

    #[test]
    fn sanitize_whitespace_only_uses_default() {
        let result = sanitize_rds_text("   ", "Fallback");
        assert_eq!(result, "Fallback");
    }

    #[test]
    fn sanitize_trims_whitespace() {
        let result = sanitize_rds_text("  Hello  ", "default");
        assert_eq!(result, "Hello");
    }

    // --- format_message_text tests ---

    #[test]
    fn format_replaces_artist_uppercase() {
        let result = format_message_text("Now: {artist}", "The Beatles", "Hey Jude");
        assert_eq!(result, "Now: THE BEATLES");
    }

    #[test]
    fn format_replaces_title_as_is() {
        let result = format_message_text("Playing: {title}", "Adele", "Hello");
        assert_eq!(result, "Playing: Hello");
    }

    #[test]
    fn format_replaces_both_placeholders() {
        let result = format_message_text("{artist} - {title}", "Queen", "Bohemian Rhapsody");
        assert_eq!(result, "QUEEN - Bohemian Rhapsody");
    }

    #[test]
    fn format_no_placeholders_returns_text() {
        let result = format_message_text("Station WXYZ", "", "");
        assert_eq!(result, "Station WXYZ");
    }

    #[test]
    fn format_trims_result() {
        let result = format_message_text("  {artist}  ", "test", "");
        assert_eq!(result, "TEST");
    }

    // --- should_display_message tests ---

    fn make_msg(text: &str, enabled: bool) -> RdsMessage {
        RdsMessage {
            text: text.to_string(),
            enabled,
            duration: 10,
            scheduled: RdsSchedule::default(),
        }
    }

    fn make_scheduled_msg(text: &str, days: Vec<&str>, hours: Vec<u8>) -> RdsMessage {
        RdsMessage {
            text: text.to_string(),
            enabled: true,
            duration: 10,
            scheduled: RdsSchedule {
                enabled: true,
                days: days.into_iter().map(String::from).collect(),
                hours,
            },
        }
    }

    #[test]
    fn disabled_message_not_displayed() {
        let msg = make_msg("Hello", false);
        assert!(!should_display_message(&msg, "Artist", "Title", false, "Monday", 10));
    }

    #[test]
    fn enabled_message_without_placeholders_displayed() {
        let msg = make_msg("Station WXYZ", true);
        assert!(should_display_message(&msg, "Artist", "Title", false, "Monday", 10));
    }

    #[test]
    fn artist_placeholder_blocked_when_not_lecture() {
        let msg = make_msg("Now: {artist}", true);
        // Not a lecture -> skip messages with {artist}
        assert!(!should_display_message(&msg, "The Beatles", "Hey Jude", false, "Monday", 10));
    }

    #[test]
    fn artist_placeholder_allowed_when_lecture() {
        let msg = make_msg("Now: {artist}", true);
        // Is a lecture -> allow messages with {artist}
        assert!(should_display_message(&msg, "Rabbi Shalom", "Torah", true, "Monday", 10));
    }

    #[test]
    fn empty_artist_blocks_artist_placeholder() {
        let msg = make_msg("{artist} playing", true);
        assert!(!should_display_message(&msg, "", "Title", true, "Monday", 10));
    }

    #[test]
    fn empty_title_blocks_title_placeholder() {
        let msg = make_msg("Now: {title}", true);
        assert!(!should_display_message(&msg, "Artist", "", false, "Monday", 10));
    }

    #[test]
    fn scheduled_day_filter_matches() {
        let msg = make_scheduled_msg("Hello", vec!["Monday", "Friday"], vec![]);
        assert!(should_display_message(&msg, "A", "T", false, "Monday", 10));
    }

    #[test]
    fn scheduled_day_filter_blocks() {
        let msg = make_scheduled_msg("Hello", vec!["Monday", "Friday"], vec![]);
        assert!(!should_display_message(&msg, "A", "T", false, "Wednesday", 10));
    }

    #[test]
    fn scheduled_hour_filter_matches() {
        let msg = make_scheduled_msg("Hello", vec![], vec![9, 10, 14]);
        assert!(should_display_message(&msg, "A", "T", false, "Monday", 10));
    }

    #[test]
    fn scheduled_hour_filter_blocks() {
        let msg = make_scheduled_msg("Hello", vec![], vec![9, 10, 14]);
        assert!(!should_display_message(&msg, "A", "T", false, "Monday", 12));
    }

    #[test]
    fn scheduled_day_case_insensitive() {
        let msg = make_scheduled_msg("Hello", vec!["monday"], vec![]);
        assert!(should_display_message(&msg, "A", "T", false, "Monday", 10));
    }

    #[test]
    fn unscheduled_message_ignores_day_hour() {
        let msg = make_msg("Hello", true);
        // No scheduling enabled -> always passes schedule check
        assert!(should_display_message(&msg, "A", "T", false, "Sunday", 3));
    }

    // --- format_hour_ampm tests ---

    #[test]
    fn hour_ampm_formatting() {
        assert_eq!(format_hour_ampm(0), "12AM");
        assert_eq!(format_hour_ampm(1), "1AM");
        assert_eq!(format_hour_ampm(11), "11AM");
        assert_eq!(format_hour_ampm(12), "12PM");
        assert_eq!(format_hour_ampm(13), "1PM");
        assert_eq!(format_hour_ampm(23), "11PM");
    }

    // --- RdsMessage display helpers ---

    #[test]
    fn days_display_all_when_unscheduled() {
        let msg = make_msg("Hello", true);
        assert_eq!(msg.days_display(), "All");
    }

    #[test]
    fn days_display_shows_scheduled_days() {
        let msg = make_scheduled_msg("Hello", vec!["Mon", "Fri"], vec![]);
        assert_eq!(msg.days_display(), "Mon,Fri");
    }

    #[test]
    fn hours_display_all_when_unscheduled() {
        let msg = make_msg("Hello", true);
        assert_eq!(msg.hours_display(), "All");
    }

    #[test]
    fn hours_display_shows_scheduled_hours() {
        let msg = make_scheduled_msg("Hello", vec![], vec![9, 14]);
        assert_eq!(msg.hours_display(), "9AM,2PM");
    }
}
