---
name: rds-engine-spec
description: Complete implementation specification for the RDS engine, message rotation, lecture detection, and RDS config UI
user_invocable: true
---

# RDS Engine & Config UI - Complete Implementation Specification

Use this spec to implement the RDS message rotation engine and configuration UI within signalFlow. Now-playing information is read directly from the internal engine state — no external XML files or readers are involved.

---

## 1. AutoRDSHandler - RDS Message Rotation Engine (DONE)

### 1.1 Constants

```
SOCKET_TIMEOUT        = 10    # seconds - TCP socket connect/read timeout
COMMAND_DELAY         = 0.2   # seconds - delay after each RDS command send
LOOP_SLEEP            = 1     # seconds - main loop sleep between iterations
ERROR_RETRY_DELAY     = 15    # seconds - wait after a fatal loop error before retry
KEEPALIVE_INTERVAL    = 60    # seconds - resend same message to maintain encoder state
```

### 1.2 Data Model

```rust
AutoRDSHandler {
    engine: Arc<Mutex<Engine>>       // reference to the core engine for now-playing info
    rds_ip: String
    rds_port: u16
    default_message: String
    messages: Vec<RdsMessage>        // loaded from config
    running: AtomicBool
    message_index: AtomicUsize       // rotation index into valid_messages
    last_send_time: Mutex<Instant>
    current_message_duration: AtomicU32  // seconds to display current message
    last_sent_text: Mutex<Option<String>>
    last_send_status: Mutex<Option<String>>  // "success", "timeout", or None
    lecture_detector: LectureDetector
}
```

### 1.3 Configuration

Loaded from engine config:

| Setting | Default |
|---------|---------|
| rds_ip | "127.0.0.1" |
| rds_port | 10001 |
| default_message | "signalFlow Radio Automation" |

### 1.4 Now-Playing Info (from Engine)

Instead of reading an external XML file, the handler queries the engine directly:

```rust
fn get_now_playing(&self) -> NowPlaying {
    let engine = self.engine.lock();
    NowPlaying {
        artist: engine.current_track_artist().unwrap_or_default(),
        title: engine.current_track_title().unwrap_or_default(),
    }
}
```

This eliminates all XML caching, file polling, and anti-caching workarounds.

### 1.5 Message Filtering (should_display_message)

For each message, checks are applied **in order**. If any check fails, return false:

#### Check 1: Enabled
```
if not message.enabled: return false
```

#### Check 2: Lecture Detection (only affects messages containing `{artist}`)
1. Query engine for current track info.
2. Determine if current track is a lecture via `lecture_detector.is_lecture(artist)`.
3. If current track IS a lecture: message allowed (continue checks).
4. If current track is NOT a lecture AND message contains `{artist}`: **SKIP** (return false).
5. If current track is NOT a lecture AND message does NOT contain `{artist}`: allowed.

#### Check 3: Placeholder Availability
```
if "{artist}" in text and artist is empty: return false
if "{title}" in text and title is empty: return false
```

#### Check 4: Schedule (only if message.scheduled.enabled is true)
1. If scheduled.days is non-empty and current day not in list: return false.
2. If scheduled.hours is non-empty and current hour not in list: return false.
3. If all checks pass: return true.

### 1.6 Placeholder Replacement (format_message_text)

```
{artist} -> artist.to_uppercase()   # UPPERCASE
{title}  -> title                   # as-is (case preserved)
```

After replacement, trim whitespace.

### 1.7 RDS Socket Protocol

**Protocol**: TCP socket connection, send command as `"DPSTEXT={text}\r\n"` encoded UTF-8. Read up to 1024 bytes response.

**Sanitization**:
1. Replace `\r` and `\n` with spaces.
2. Trim whitespace.
3. Truncate to **64 characters max**.
4. If result is empty, use `default_message[..64]`.

**Send flow**:
1. Create TCP socket with SOCKET_TIMEOUT.
2. Connect to (rds_ip, rds_port).
3. Send `"DPSTEXT={sanitized_text}\r\n"`.
4. Read response.
5. Success = response is non-empty AND does not start with "Error:".
6. Set `last_send_status` accordingly.
7. Sleep COMMAND_DELAY (0.2s) after send.

### 1.8 Main Loop

```
while running:
    1. Get messages from config
    2. Get now_playing from engine (direct query, no XML)
    3. Filter: valid_messages = messages where should_display_message() is true
    4. Determine display_text and selected_duration:
       - If no valid messages: display_text = default_message, duration = 10
       - Else: select valid_messages[message_index % len]
         - Format text via format_message_text()
         - If formatted text is empty: advance index, skip
         - Else: display_text = formatted, duration = message.duration (default 10)
    5. If display_text is set:
       - time_since_last_send = now - last_send_time
       - rotation_due = time_since_last_send >= current_message_duration
       - keepalive_due = (same text as last) AND (time_since_last_send >= KEEPALIVE_INTERVAL)
       - should_send = rotation_due OR keepalive_due
       - If should_send:
         a. send_message_to_rds(display_text)
         b. Update last_sent_text, last_send_time, current_message_duration
         c. Only advance message_index if rotation_due (not keepalive)
    6. Sleep LOOP_SLEEP (1 second)

    On exception: log, sleep ERROR_RETRY_DELAY (15s), continue
```

### 1.9 Threading

- Spawns a background thread (or tokio task)
- start(): sets running=true, spawns thread
- stop(): sets running=false, joins thread
- Status methods: get_current_display_messages(), get_current_message_status()

---

## 2. LectureDetector - Track Classification (DONE — Phase F)

### 2.1 Classification Rules (applied in order)

```
is_artist_lecture(artist):
    1. if artist is empty: return false
    2. artist_lower = artist.to_lowercase()
    3. if artist_lower in blacklist: return false     # Blacklist overrides everything
    4. if artist_lower in whitelist: return true      # Whitelist forces lecture
    5. if artist_lower.starts_with('r'): return true  # Starts with 'R' = lecture
    6. return false
```

**Priority**: Blacklist > Whitelist > starts-with-'R'

### 2.2 Lists

- **Blacklist**: Set of lowercase artist names that are NEVER lectures, even if they start with 'R'.
- **Whitelist**: Set of lowercase artist names that are ALWAYS lectures.
- Both stored in engine config, case-insensitive.

### 2.3 Data Model

```rust
LectureDetector {
    engine: Arc<Mutex<Engine>>  // for current/next track queries
    blacklist: HashSet<String>  // lowercase
    whitelist: HashSet<String>  // lowercase
}
```

### 2.4 Methods

- `is_current_track_lecture() -> bool`: Queries engine for current track artist, applies rules.
- `is_next_track_lecture() -> bool`: Queries engine for next track artist, applies rules.
- `get_current_track_info() -> (String, String)`: Returns (artist, title) from engine.
- `has_next_track() -> bool`: Queries engine.
- `update_lists()`: Refreshes blacklist/whitelist from config.

---

## 3. Config - RDS Settings JSON Schema (DONE)

### 3.1 Message Object

```json
{
  "text": "string (max 64 chars, may contain {artist} and {title} placeholders)",
  "enabled": true,
  "duration": 10,
  "scheduled": {
    "enabled": false,
    "days": ["Sunday", "Monday"],
    "hours": [9, 10, 14]
  }
}
```

### 3.2 RDS Settings

```json
"rds": {
  "ip": "127.0.0.1",
  "port": 10001,
  "default_message": "signalFlow Radio Automation",
  "messages": [...]
}
```

### 3.3 Shared Lists

```json
"shared": {
  "whitelist": ["artist name 1", "artist name 2"],
  "blacklist": ["artist name 3"]
}
```

---

## 4. RDS Config UI (Tauri)

### 4.1 Window Properties

- Modal dialog
- Title: "Configure RDS Messages"

### 4.2 Layout

```
LEFT: Message list (treeview/table)
  Toolbar: [Add New] [Delete] [Move Up] [Move Down]
  Columns: Message (text), Enabled, Use Sched, Dur (s), Days, Times (Hr)
  Sortable, scrollable

RIGHT: Message detail editor
  - Message content entry (64 char max with counter)
  - Enabled checkbox
  - Duration spinner (1-60 seconds)
  - Use Scheduling checkbox
  - Schedule container (shown when scheduling enabled):
    - Day checkboxes (Sun-Sat) with Select All / Clear All
    - Hour checkboxes (0-23 in AM/PM format) with Select All / Clear All

Bottom: [Save Changes] [Cancel]
```

### 4.3 Hour AM/PM Formatting

```
0  -> "12 AM"
1  -> "1 AM"
...
11 -> "11 AM"
12 -> "12 PM"
13 -> "1 PM"
...
23 -> "11 PM"
```

### 4.4 Message Operations

**Add**: Creates default message with text="New Message", enabled=false, duration=10, scheduling disabled.

**Delete**: Removes from list, selects next available item.

**Move Up/Down**: Reorders in list.

### 4.5 Live Update on Edit

All detail widgets trigger live treeview row updates as values change.

### 4.6 Treeview Display

- Text column: truncated to 30 chars
- Enabled: "Yes" / "No"
- Scheduled: "Yes" / "No"
- Duration: integer
- Days: comma-separated
- Times: comma-separated AM/PM hours

### 4.7 Save/Cancel

- **Save**: Persists messages to engine config via IPC commands.
- **Cancel/Close with unsaved changes**: Confirmation dialog (Save / Discard / Stay).

### 4.8 State Management

- Detail widgets disabled when no selection
- Schedule controls disabled when "Use Scheduling" is unchecked
- Toolbar buttons (Delete, Move) disabled when no selection
