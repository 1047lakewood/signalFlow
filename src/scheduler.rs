use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// How a scheduled event interacts with current playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleMode {
    /// Play sound on top of current audio (e.g., jingles, sound FX).
    Overlay,
    /// Kill current audio, play scheduled item (e.g., hard news break).
    Stop,
    /// Queue scheduled item as the next track in the active playlist.
    Insert,
}

impl fmt::Display for ScheduleMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScheduleMode::Overlay => write!(f, "overlay"),
            ScheduleMode::Stop => write!(f, "stop"),
            ScheduleMode::Insert => write!(f, "insert"),
        }
    }
}

impl ScheduleMode {
    /// Parse a mode from a string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "overlay" => Ok(ScheduleMode::Overlay),
            "stop" => Ok(ScheduleMode::Stop),
            "insert" => Ok(ScheduleMode::Insert),
            _ => Err(format!(
                "Unknown schedule mode '{}'. Expected: overlay, stop, insert",
                s
            )),
        }
    }
}

/// Priority level for scheduled events (higher = more important).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Priority(pub u8);

impl Priority {
    pub const LOW: Priority = Priority(1);
    pub const NORMAL: Priority = Priority(5);
    pub const HIGH: Priority = Priority(9);
}

impl Default for Priority {
    fn default() -> Self {
        Priority::NORMAL
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A single scheduled event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEvent {
    /// Unique identifier.
    pub id: u32,
    /// Time of day to trigger (HH:MM:SS).
    pub time: NaiveTime,
    /// How to interact with current playback.
    pub mode: ScheduleMode,
    /// Path to the audio file to play.
    pub file: PathBuf,
    /// Priority level (higher wins in conflicts).
    #[serde(default)]
    pub priority: Priority,
    /// Whether this event is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Optional label/description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Days of the week this event recurs (0=Mon..6=Sun). Empty = every day.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<u8>,
}

fn default_true() -> bool {
    true
}

impl ScheduleEvent {
    /// Format the time as HH:MM:SS.
    pub fn time_display(&self) -> String {
        self.time.format("%H:%M:%S").to_string()
    }

    /// Format the days field for display.
    pub fn days_display(&self) -> String {
        if self.days.is_empty() {
            return "daily".to_string();
        }
        let names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        self.days
            .iter()
            .filter_map(|&d| names.get(d as usize))
            .copied()
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// The schedule — a list of timed events managed by the engine.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Schedule {
    pub events: Vec<ScheduleEvent>,
    next_id: u32,
}

impl Schedule {
    pub fn new() -> Self {
        Schedule {
            events: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a new event. Returns the assigned ID.
    pub fn add_event(
        &mut self,
        time: NaiveTime,
        mode: ScheduleMode,
        file: PathBuf,
        priority: Priority,
        label: Option<String>,
        days: Vec<u8>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.events.push(ScheduleEvent {
            id,
            time,
            mode,
            file,
            priority,
            enabled: true,
            label,
            days,
        });
        id
    }

    /// Remove an event by ID. Returns the removed event or an error.
    pub fn remove_event(&mut self, id: u32) -> Result<ScheduleEvent, String> {
        let pos = self
            .events
            .iter()
            .position(|e| e.id == id)
            .ok_or_else(|| format!("Schedule event {} not found", id))?;
        Ok(self.events.remove(pos))
    }

    /// Find an event by ID.
    pub fn find_event(&self, id: u32) -> Option<&ScheduleEvent> {
        self.events.iter().find(|e| e.id == id)
    }

    /// Find an event by ID, mutable.
    pub fn find_event_mut(&mut self, id: u32) -> Option<&mut ScheduleEvent> {
        self.events.iter_mut().find(|e| e.id == id)
    }

    /// Toggle an event's enabled state. Returns the new state.
    pub fn toggle_event(&mut self, id: u32) -> Result<bool, String> {
        let event = self
            .find_event_mut(id)
            .ok_or_else(|| format!("Schedule event {} not found", id))?;
        event.enabled = !event.enabled;
        Ok(event.enabled)
    }

    /// Get all events sorted by time.
    pub fn events_by_time(&self) -> Vec<&ScheduleEvent> {
        let mut sorted: Vec<&ScheduleEvent> = self.events.iter().collect();
        sorted.sort_by_key(|e| e.time);
        sorted
    }

    /// Number of events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Parse a time string in HH:MM or HH:MM:SS format.
pub fn parse_time(s: &str) -> Result<NaiveTime, String> {
    // Try HH:MM:SS first, then HH:MM
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .map_err(|_| format!("Invalid time '{}'. Expected HH:MM or HH:MM:SS", s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_time_hhmm() {
        let t = parse_time("14:00").unwrap();
        assert_eq!(t, NaiveTime::from_hms_opt(14, 0, 0).unwrap());
    }

    #[test]
    fn parse_time_hhmmss() {
        let t = parse_time("14:30:15").unwrap();
        assert_eq!(t, NaiveTime::from_hms_opt(14, 30, 15).unwrap());
    }

    #[test]
    fn parse_time_invalid() {
        assert!(parse_time("25:00").is_err());
        assert!(parse_time("abc").is_err());
        assert!(parse_time("").is_err());
    }

    #[test]
    fn schedule_mode_from_str() {
        assert_eq!(ScheduleMode::from_str_loose("overlay").unwrap(), ScheduleMode::Overlay);
        assert_eq!(ScheduleMode::from_str_loose("STOP").unwrap(), ScheduleMode::Stop);
        assert_eq!(ScheduleMode::from_str_loose("Insert").unwrap(), ScheduleMode::Insert);
        assert!(ScheduleMode::from_str_loose("bogus").is_err());
    }

    #[test]
    fn schedule_mode_display() {
        assert_eq!(format!("{}", ScheduleMode::Overlay), "overlay");
        assert_eq!(format!("{}", ScheduleMode::Stop), "stop");
        assert_eq!(format!("{}", ScheduleMode::Insert), "insert");
    }

    #[test]
    fn schedule_add_and_find() {
        let mut sched = Schedule::new();
        let id = sched.add_event(
            NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
            ScheduleMode::Stop,
            "news.mp3".into(),
            Priority::HIGH,
            Some("News break".to_string()),
            vec![],
        );
        assert_eq!(sched.len(), 1);
        let event = sched.find_event(id).unwrap();
        assert_eq!(event.mode, ScheduleMode::Stop);
        assert_eq!(event.file, PathBuf::from("news.mp3"));
        assert_eq!(event.priority.0, 9);
        assert!(event.enabled);
    }

    #[test]
    fn schedule_remove() {
        let mut sched = Schedule::new();
        let id = sched.add_event(
            NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            ScheduleMode::Overlay,
            "jingle.mp3".into(),
            Priority::NORMAL,
            None,
            vec![],
        );
        assert_eq!(sched.len(), 1);
        let removed = sched.remove_event(id).unwrap();
        assert_eq!(removed.id, id);
        assert_eq!(sched.len(), 0);
    }

    #[test]
    fn schedule_remove_not_found() {
        let mut sched = Schedule::new();
        assert!(sched.remove_event(999).is_err());
    }

    #[test]
    fn schedule_toggle() {
        let mut sched = Schedule::new();
        let id = sched.add_event(
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            ScheduleMode::Insert,
            "promo.mp3".into(),
            Priority::NORMAL,
            None,
            vec![],
        );
        assert!(sched.find_event(id).unwrap().enabled);
        let new_state = sched.toggle_event(id).unwrap();
        assert!(!new_state);
        assert!(!sched.find_event(id).unwrap().enabled);
        let new_state = sched.toggle_event(id).unwrap();
        assert!(new_state);
    }

    #[test]
    fn schedule_events_by_time() {
        let mut sched = Schedule::new();
        sched.add_event(
            NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
            ScheduleMode::Stop,
            "evening.mp3".into(),
            Priority::NORMAL,
            None,
            vec![],
        );
        sched.add_event(
            NaiveTime::from_hms_opt(6, 0, 0).unwrap(),
            ScheduleMode::Overlay,
            "morning.mp3".into(),
            Priority::NORMAL,
            None,
            vec![],
        );
        sched.add_event(
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            ScheduleMode::Insert,
            "noon.mp3".into(),
            Priority::NORMAL,
            None,
            vec![],
        );
        let sorted = sched.events_by_time();
        assert_eq!(sorted[0].time, NaiveTime::from_hms_opt(6, 0, 0).unwrap());
        assert_eq!(sorted[1].time, NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        assert_eq!(sorted[2].time, NaiveTime::from_hms_opt(18, 0, 0).unwrap());
    }

    #[test]
    fn schedule_unique_ids() {
        let mut sched = Schedule::new();
        let id1 = sched.add_event(
            NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            ScheduleMode::Overlay,
            "a.mp3".into(),
            Priority::NORMAL,
            None,
            vec![],
        );
        let id2 = sched.add_event(
            NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            ScheduleMode::Overlay,
            "b.mp3".into(),
            Priority::NORMAL,
            None,
            vec![],
        );
        assert_ne!(id1, id2);
    }

    #[test]
    fn schedule_serialization_roundtrip() {
        let mut sched = Schedule::new();
        sched.add_event(
            NaiveTime::from_hms_opt(14, 30, 0).unwrap(),
            ScheduleMode::Stop,
            "news.mp3".into(),
            Priority::HIGH,
            Some("Afternoon news".to_string()),
            vec![0, 1, 2, 3, 4], // Mon-Fri
        );
        let json = serde_json::to_string(&sched).unwrap();
        let loaded: Schedule = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.len(), 1);
        let event = &loaded.events[0];
        assert_eq!(event.mode, ScheduleMode::Stop);
        assert_eq!(event.priority, Priority::HIGH);
        assert_eq!(event.days, vec![0, 1, 2, 3, 4]);
        assert_eq!(event.label, Some("Afternoon news".to_string()));
    }

    #[test]
    fn days_display_daily() {
        let event = ScheduleEvent {
            id: 1,
            time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            mode: ScheduleMode::Overlay,
            file: "test.mp3".into(),
            priority: Priority::NORMAL,
            enabled: true,
            label: None,
            days: vec![],
        };
        assert_eq!(event.days_display(), "daily");
    }

    #[test]
    fn days_display_weekdays() {
        let event = ScheduleEvent {
            id: 1,
            time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            mode: ScheduleMode::Overlay,
            file: "test.mp3".into(),
            priority: Priority::NORMAL,
            enabled: true,
            label: None,
            days: vec![0, 1, 2, 3, 4],
        };
        assert_eq!(event.days_display(), "Mon,Tue,Wed,Thu,Fri");
    }

    #[test]
    fn priority_ordering() {
        assert!(Priority::HIGH > Priority::NORMAL);
        assert!(Priority::NORMAL > Priority::LOW);
    }

    #[test]
    fn schedule_defaults_when_missing_from_json() {
        let json = r#"{"events":[],"next_id":1}"#;
        let sched: Schedule = serde_json::from_str(json).unwrap();
        assert_eq!(sched.len(), 0);
    }
}
