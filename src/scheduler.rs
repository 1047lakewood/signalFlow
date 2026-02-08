use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Policy for resolving conflicts between manual playback and scheduled events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictPolicy {
    /// Scheduled events always fire, even if the operator is manually playing something.
    /// HIGH priority events interrupt; NORMAL/LOW events queue or overlay as configured.
    ScheduleWins,
    /// Manual playback takes precedence. Only HIGH priority (7+) scheduled events
    /// will still fire during manual activity. NORMAL and LOW events are skipped.
    ManualWins,
}

impl Default for ConflictPolicy {
    fn default() -> Self {
        ConflictPolicy::ScheduleWins
    }
}

impl fmt::Display for ConflictPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConflictPolicy::ScheduleWins => write!(f, "schedule-wins"),
            ConflictPolicy::ManualWins => write!(f, "manual-wins"),
        }
    }
}

impl ConflictPolicy {
    /// Parse a policy from a string (case-insensitive, accepts hyphens or underscores).
    pub fn from_str_loose(s: &str) -> Result<Self, String> {
        match s.to_lowercase().replace('_', "-").as_str() {
            "schedule-wins" | "schedule" => Ok(ConflictPolicy::ScheduleWins),
            "manual-wins" | "manual" => Ok(ConflictPolicy::ManualWins),
            _ => Err(format!(
                "Unknown conflict policy '{}'. Expected: schedule-wins, manual-wins",
                s
            )),
        }
    }

    /// The minimum priority level required for a scheduled event to fire
    /// when manual playback is active under this policy.
    pub fn manual_override_threshold(&self) -> Priority {
        match self {
            ConflictPolicy::ScheduleWins => Priority::LOW, // all events fire
            ConflictPolicy::ManualWins => Priority(7),      // only high-priority events fire
        }
    }
}

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

    /// Given events that fire at the same time, resolve conflicts.
    /// For each mode, only the highest-priority event wins.
    /// Returns the winning events (one per mode, at most).
    pub fn resolve_time_conflicts<'a>(events: &[&'a ScheduleEvent]) -> Vec<&'a ScheduleEvent> {
        let mut best_overlay: Option<&ScheduleEvent> = None;
        let mut best_stop: Option<&ScheduleEvent> = None;
        let mut best_insert: Option<&ScheduleEvent> = None;

        for &event in events {
            if !event.enabled {
                continue;
            }
            let slot = match event.mode {
                ScheduleMode::Overlay => &mut best_overlay,
                ScheduleMode::Stop => &mut best_stop,
                ScheduleMode::Insert => &mut best_insert,
            };
            match slot {
                Some(current) if event.priority > current.priority => *slot = Some(event),
                None => *slot = Some(event),
                _ => {}
            }
        }

        let mut winners = Vec::new();
        // Stop fires first (most disruptive), then insert, then overlay
        if let Some(e) = best_stop {
            winners.push(e);
        }
        if let Some(e) = best_insert {
            winners.push(e);
        }
        if let Some(e) = best_overlay {
            winners.push(e);
        }
        winners
    }

    /// Filter events for when manual playback is active.
    /// Under the given policy, only events meeting the priority threshold are returned.
    pub fn filter_for_manual_playback<'a>(
        events: &[&'a ScheduleEvent],
        policy: ConflictPolicy,
    ) -> Vec<&'a ScheduleEvent> {
        let threshold = policy.manual_override_threshold();
        events
            .iter()
            .filter(|e| e.enabled && e.priority >= threshold)
            .copied()
            .collect()
    }

    /// Get events that should fire at a given time, considering a tolerance window (in seconds).
    /// Returns enabled events whose time falls within [time - tolerance, time + tolerance].
    pub fn events_at_time(&self, time: NaiveTime, tolerance_secs: i64) -> Vec<&ScheduleEvent> {
        self.events
            .iter()
            .filter(|e| {
                if !e.enabled {
                    return false;
                }
                let diff = (e.time - time).num_seconds().abs();
                diff <= tolerance_secs
            })
            .collect()
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

    // --- Conflict Policy tests ---

    #[test]
    fn conflict_policy_default_is_schedule_wins() {
        assert_eq!(ConflictPolicy::default(), ConflictPolicy::ScheduleWins);
    }

    #[test]
    fn conflict_policy_from_str() {
        assert_eq!(
            ConflictPolicy::from_str_loose("schedule-wins").unwrap(),
            ConflictPolicy::ScheduleWins
        );
        assert_eq!(
            ConflictPolicy::from_str_loose("manual-wins").unwrap(),
            ConflictPolicy::ManualWins
        );
        assert_eq!(
            ConflictPolicy::from_str_loose("schedule").unwrap(),
            ConflictPolicy::ScheduleWins
        );
        assert_eq!(
            ConflictPolicy::from_str_loose("MANUAL").unwrap(),
            ConflictPolicy::ManualWins
        );
        assert!(ConflictPolicy::from_str_loose("bogus").is_err());
    }

    #[test]
    fn conflict_policy_display() {
        assert_eq!(format!("{}", ConflictPolicy::ScheduleWins), "schedule-wins");
        assert_eq!(format!("{}", ConflictPolicy::ManualWins), "manual-wins");
    }

    #[test]
    fn conflict_policy_serialization_roundtrip() {
        let policy = ConflictPolicy::ManualWins;
        let json = serde_json::to_string(&policy).unwrap();
        let loaded: ConflictPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, ConflictPolicy::ManualWins);
    }

    #[test]
    fn manual_override_threshold_schedule_wins() {
        let policy = ConflictPolicy::ScheduleWins;
        // All events should fire — threshold is LOW (1)
        assert_eq!(policy.manual_override_threshold(), Priority::LOW);
    }

    #[test]
    fn manual_override_threshold_manual_wins() {
        let policy = ConflictPolicy::ManualWins;
        // Only priority 7+ events fire during manual playback
        assert_eq!(policy.manual_override_threshold(), Priority(7));
    }

    // --- Time conflict resolution tests ---

    fn make_event(id: u32, mode: ScheduleMode, priority: u8) -> ScheduleEvent {
        ScheduleEvent {
            id,
            time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            mode,
            file: format!("event_{}.mp3", id).into(),
            priority: Priority(priority),
            enabled: true,
            label: None,
            days: vec![],
        }
    }

    #[test]
    fn resolve_time_conflicts_single_event() {
        let e1 = make_event(1, ScheduleMode::Stop, 5);
        let events: Vec<&ScheduleEvent> = vec![&e1];
        let winners = Schedule::resolve_time_conflicts(&events);
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].id, 1);
    }

    #[test]
    fn resolve_time_conflicts_same_mode_higher_priority_wins() {
        let e1 = make_event(1, ScheduleMode::Stop, 3);
        let e2 = make_event(2, ScheduleMode::Stop, 9);
        let events: Vec<&ScheduleEvent> = vec![&e1, &e2];
        let winners = Schedule::resolve_time_conflicts(&events);
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].id, 2);
    }

    #[test]
    fn resolve_time_conflicts_different_modes_coexist() {
        let e1 = make_event(1, ScheduleMode::Stop, 9);
        let e2 = make_event(2, ScheduleMode::Insert, 5);
        let e3 = make_event(3, ScheduleMode::Overlay, 5);
        let events: Vec<&ScheduleEvent> = vec![&e1, &e2, &e3];
        let winners = Schedule::resolve_time_conflicts(&events);
        assert_eq!(winners.len(), 3);
        // Order: stop, insert, overlay
        assert_eq!(winners[0].mode, ScheduleMode::Stop);
        assert_eq!(winners[1].mode, ScheduleMode::Insert);
        assert_eq!(winners[2].mode, ScheduleMode::Overlay);
    }

    #[test]
    fn resolve_time_conflicts_disabled_events_excluded() {
        let e1 = make_event(1, ScheduleMode::Stop, 9);
        let mut e2 = make_event(2, ScheduleMode::Stop, 5);
        e2.enabled = false;
        let events: Vec<&ScheduleEvent> = vec![&e1, &e2];
        let winners = Schedule::resolve_time_conflicts(&events);
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].id, 1);
    }

    #[test]
    fn resolve_time_conflicts_empty_input() {
        let events: Vec<&ScheduleEvent> = vec![];
        let winners = Schedule::resolve_time_conflicts(&events);
        assert!(winners.is_empty());
    }

    // --- Manual playback filter tests ---

    #[test]
    fn filter_for_manual_schedule_wins_passes_all() {
        let e1 = make_event(1, ScheduleMode::Stop, 1);
        let e2 = make_event(2, ScheduleMode::Overlay, 5);
        let events: Vec<&ScheduleEvent> = vec![&e1, &e2];
        let filtered = Schedule::filter_for_manual_playback(&events, ConflictPolicy::ScheduleWins);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_for_manual_manual_wins_filters_low_priority() {
        let e1 = make_event(1, ScheduleMode::Stop, 5); // below threshold (7)
        let e2 = make_event(2, ScheduleMode::Stop, 9); // above threshold
        let e3 = make_event(3, ScheduleMode::Overlay, 7); // at threshold
        let events: Vec<&ScheduleEvent> = vec![&e1, &e2, &e3];
        let filtered = Schedule::filter_for_manual_playback(&events, ConflictPolicy::ManualWins);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|e| e.priority >= Priority(7)));
    }

    // --- events_at_time tests ---

    #[test]
    fn events_at_time_exact_match() {
        let mut sched = Schedule::new();
        sched.add_event(
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            ScheduleMode::Stop,
            "noon.mp3".into(),
            Priority::NORMAL,
            None,
            vec![],
        );
        sched.add_event(
            NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
            ScheduleMode::Overlay,
            "evening.mp3".into(),
            Priority::NORMAL,
            None,
            vec![],
        );
        let at_noon = sched.events_at_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap(), 0);
        assert_eq!(at_noon.len(), 1);
        assert_eq!(at_noon[0].file, PathBuf::from("noon.mp3"));
    }

    #[test]
    fn events_at_time_with_tolerance() {
        let mut sched = Schedule::new();
        sched.add_event(
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            ScheduleMode::Stop,
            "noon.mp3".into(),
            Priority::NORMAL,
            None,
            vec![],
        );
        // Query at 12:00:02 with 5-second tolerance should find it
        let found = sched.events_at_time(NaiveTime::from_hms_opt(12, 0, 2).unwrap(), 5);
        assert_eq!(found.len(), 1);
        // Query at 12:00:10 with 5-second tolerance should miss it
        let missed = sched.events_at_time(NaiveTime::from_hms_opt(12, 0, 10).unwrap(), 5);
        assert!(missed.is_empty());
    }
}
