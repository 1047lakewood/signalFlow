---
name: rds-engine-spec
description: Complete implementation specification for the RDS engine, message rotation, lecture detection, XML reading, and RDS config UI
user_invocable: true
---

# RDS Engine & Config UI - Complete Implementation Specification

Use this spec to reproduce the RDS message rotation engine and configuration UI in any language. Every constant, data structure, algorithm, and UI widget is documented below.

---

## 1. AutoRDSHandler - RDS Message Rotation Engine

### 1.1 Constants

```
SOCKET_TIMEOUT        = 10    # seconds - TCP socket connect/read timeout
COMMAND_DELAY         = 0.2   # seconds - delay after each RDS command send
LOOP_SLEEP            = 1     # seconds - main loop sleep between iterations
ERROR_RETRY_DELAY     = 15    # seconds - wait after a fatal loop error before retry
KEEPALIVE_INTERVAL    = 60    # seconds - resend same message to maintain encoder state
```

### 1.2 Class Structure

```
AutoRDSHandler
  Constructor(log_queue, config_manager, station_id):
    - config_manager: ConfigManager instance
    - station_id: string ("station_1047" or "station_887")
    - running: bool = False
    - thread: Thread = None (daemon thread)
    - message_index: int = 0  (rotation index into valid_messages list)
    - last_send_time: float = 0.0  (monotonic time of last RDS send)
    - current_message_duration: int = 10  (seconds to display current message)
    - last_sent_text: string = None
    - last_send_status: string = None  ('success', 'timeout', or None)
    - XML cache vars: _xml_cache=None, _xml_cache_time=0, _xml_cache_mtime=0
    - Logger name: "AutoRDS_{station_number}" (e.g., "AutoRDS_1047")
    - Calls reload_configuration() and reload_lecture_detector()
```

### 1.3 Configuration Loading (reload_configuration)

Loaded from ConfigManager using `get_station_setting(station_id, key, default)`:

| Setting | Config Path | Default |
|---------|------------|---------|
| rds_ip | settings.rds.ip | "50.208.125.83" |
| rds_port | settings.rds.port | 10001 |
| now_playing_xml | (via get_xml_path) | station-specific |
| default_message | settings.rds.default_message | "732.901.7777 to SUPPORT and hear this program!" |

### 1.4 XML Caching Strategy (_load_now_playing)

1. Check if XML file exists. If not, clear cache, return `{artist: "", title: ""}`.
2. Get file mtime via `os.path.getmtime()`.
3. **Cache hit condition**: cache is not None AND (current_time - cache_time < 2.0 seconds) AND (file mtime == cached mtime). If hit, return cached result.
4. On cache miss: sleep 0.1s (file write completion buffer), then `ET.parse()` the file.
5. Extract from root: `TRACK` element -> `ARTIST` attribute (stripped), `TITLE` child text (stripped).
6. Cache the result with current time and mtime.
7. On any error (ParseError, FileNotFoundError, etc.): clear all cache vars, return empty dict.

**Return format**: `{"artist": string, "title": string}`

### 1.5 Message Filtering (_should_display_message)

For each message, checks are applied **in order**. If any check fails, return False:

#### Check 1: Enabled
```
if not message.get("Enabled", True): return False
```

#### Check 2: Lecture Detection (only affects messages containing `{artist}`)
1. Call `lecture_detector.update_lists()` to refresh whitelist/blacklist.
2. Call `lecture_detector.is_current_track_lecture()`.
3. If artist name exists:
   - If current track IS a lecture: message allowed (continue checks).
   - If current track is NOT a lecture AND message contains `{artist}`: **SKIP this message** (return False).
   - If current track is NOT a lecture AND message does NOT contain `{artist}`: allowed.

#### Check 3: Placeholder Availability
```
if "{artist}" in text and not artist_name: return False
if "{title}" in text and not title: return False
```

#### Check 4: Schedule (only if `message.Scheduled.Enabled` is True)
1. Get current day abbreviation via `strftime("%a")` -> map to full name:
   ```
   {"Sun":"Sunday", "Mon":"Monday", "Tue":"Tuesday", "Wed":"Wednesday",
    "Thu":"Thursday", "Fri":"Friday", "Sat":"Saturday"}
   ```
2. If `Scheduled.Days` is non-empty and current day not in list: return False.
3. If `Scheduled.Times` is non-empty (list of `{"hour": int}`):
   - Check if any hour matches `datetime.now().hour` (0-23).
   - If no match: return False.
4. If all checks pass: return True.

### 1.6 Placeholder Replacement (_format_message_text)

```
{artist} -> artist.upper()   # UPPERCASE
{title}  -> title            # as-is (case preserved)
```

After replacement, strip whitespace from result.

### 1.7 RDS Socket Protocol (_send_command, _send_message_to_rds)

**Protocol**: TCP socket connection, send command as `"DPSTEXT={text}\r\n"` encoded UTF-8. Read up to 1024 bytes response.

**Sanitization** (_send_message_to_rds):
1. Replace `\r` and `\n` with spaces.
2. Strip whitespace.
3. Truncate to **64 characters max**.
4. If result is empty after sanitization, use `default_message[:64]`.

**Send flow**:
1. Create TCP socket with SOCKET_TIMEOUT.
2. Connect to (rds_ip, rds_port).
3. Send `"DPSTEXT={sanitized_text}\r\n"`.
4. Read response, decode UTF-8 (ignore errors), strip.
5. Success = response is non-empty AND does not start with "Error:".
6. Set `last_send_status` to 'success' or 'timeout'.
7. Sleep COMMAND_DELAY (0.2s) after send.

**Error responses**: "Error: Timeout", "Error: Connection Refused", "Error: Socket Error ({strerror})", "Error: {ExceptionType}".

### 1.8 Main Loop (run method)

```
while running:
    1. Get messages from config_manager.get_station_messages(station_id)
    2. Get now_playing from _load_now_playing()
    3. Filter: valid_messages = [m for m in messages if _should_display_message(m, now_playing)]
    4. Determine display_text and selected_duration:
       - If no valid messages: display_text = default_message, duration = 10
       - Else: select valid_messages[message_index % len(valid_messages)]
         - Format text via _format_message_text()
         - If formatted text is empty: advance index, don't set display_text
         - Else: display_text = formatted, duration = message["Message Time"] (default 10)
    5. If display_text is set:
       - time_since_last_send = monotonic() - last_send_time
       - rotation_due = (time_since_last_send >= current_message_duration)
       - keepalive_due = (same text as last) AND (time_since_last_send >= KEEPALIVE_INTERVAL)
       - should_send = rotation_due OR keepalive_due
       - If should_send:
         a. Call _send_message_to_rds(display_text)
         b. Update last_sent_text, last_send_time, current_message_duration
         c. Only advance message_index if rotation_due (not keepalive)
    6. Sleep LOOP_SLEEP (1 second)

    On exception: log, sleep ERROR_RETRY_DELAY (15s), continue
```

### 1.9 Observer Pattern Integration

AutoRDSHandler exposes `reload_configuration()` and `reload_lecture_detector()`. The main app registers these as config observers via `config_manager.register_observer(handler.reload_configuration)`. When config is saved, all observers are notified automatically.

### 1.10 Threading

- Thread: `daemon=True`, name="AutoRDSThread"
- start(): creates thread targeting run(), sets running=True
- stop(): sets running=False, thread=None (daemon auto-exits)
- Status methods: `get_current_display_messages()`, `get_current_message_status()`

---

## 2. NowPlayingReader - Robust XML Reading

### 2.1 XML Format

```xml
<NOWPLAYING>
  <TRACK ARTIST="Artist Name" STARTED="2025-09-29 11:05:15" DURATION="3:45">
    <TITLE>Track Title</TITLE>
  </TRACK>
  <NEXTTRACK>
    <TRACK ARTIST="Next Artist" STARTED="" DURATION="5:30">
      <TITLE>Next Track Title</TITLE>
    </TRACK>
  </NEXTTRACK>
</NOWPLAYING>
```

- `ARTIST`: attribute on TRACK element
- `STARTED`: attribute, format "YYYY-MM-DD HH:MM:SS"
- `DURATION`: attribute, format "MM:SS" or "H:MM:SS"
- `TITLE`: child text element of TRACK

### 2.2 Anti-Caching Strategy

**Critical**: Uses `open() -> read() -> ET.fromstring()` instead of `ET.parse()` to avoid OS file handle caching on Windows.

```python
with open(xml_path, 'r', encoding='utf-8') as f:
    content = f.read()
root = ET.fromstring(content)
```

### 2.3 Class Structure

```
NowPlayingReader(xml_path, logger=None):
  - xml_path: string
  - _last_modified: float = 0
  - _last_content_hash: string = ""
```

### 2.4 Methods

**get_current_track(retries=2, retry_delay=0.5)**:
- Reads TRACK element via `_read_track_element("TRACK")`
- Returns dict: `{artist, title, started_at, duration, modified_at}` or None
- Retry loop: up to `retries` additional attempts with `retry_delay` between

**get_next_track(retries=2, retry_delay=0.5)**:
- Reads via `_read_track_element("NEXTTRACK/TRACK")`
- Same return format and retry logic

**has_next_track()**:
- Reads XML root, checks if `root.find("NEXTTRACK")` is not None
- Returns bool

**has_file_changed()**:
- Compares current mtime to `_last_modified`
- Updates `_last_modified` on each call
- Returns bool

**wait_for_artist(target_artist, timeout=60, poll_interval=2, same_hour_required=True, attempt_hour=None)**:
- Polls XML every `poll_interval` seconds until `timeout`
- Case-insensitive artist match (`artist.lower() == target_lower`)
- Checks same-hour requirement (compares current hour to attempt_hour)
- Returns: `{ok: bool, artist: str, started_at: str, same_hour: bool}` on success
- Returns: `{ok: False, reason: "timeout", last_artist: str}` on timeout

**force_refresh()**:
- Touches file access time via `os.utime()` to help invalidate Windows file cache

### 2.5 Convenience Functions (module-level)

```
read_nowplaying_artist(xml_path) -> Optional[str]
read_nowplaying_track(xml_path) -> Optional[Dict]
is_adroll_playing(xml_path) -> bool  # case-insensitive check for "adroll"
```

---

## 3. LectureDetector - Track Classification

### 3.1 Classification Rules (applied in order)

```
_is_artist_lecture(artist):
    1. if not artist: return False
    2. artist_lower = artist.lower()
    3. if artist_lower in blacklist: return False     # Blacklist overrides everything
    4. if artist_lower in whitelist: return True      # Whitelist forces lecture
    5. if artist_lower.startswith('r'): return True   # Starts with 'R' = lecture
    6. return False
```

**Priority**: Blacklist > Whitelist > starts-with-'R'

### 3.2 Lists

- **Blacklist** (shared across stations): Set of lowercase artist names that are NEVER lectures, even if they start with 'R'. Retrieved via `config_manager.get_shared_blacklist()`.
- **Whitelist** (shared across stations): Set of lowercase artist names that are ALWAYS lectures. Retrieved via `config_manager.get_shared_whitelist()`.
- Both stored as case-insensitive sets. `update_lists()` refreshes from config.

### 3.3 Class Structure

```
LectureDetector(xml_path, config_manager=None, station_id=None, blacklist=None, whitelist=None):
  - Uses NowPlayingReader internally if available
  - Falls back to direct XML parsing via _read_xml_fresh()

Methods:
  - is_current_track_lecture() -> bool  (checks TRACK)
  - is_next_track_lecture() -> bool     (checks NEXTTRACK/TRACK)
  - get_current_track_info() -> {artist: str, title: str}
  - get_current_track_duration() -> str (e.g., "3:45")
  - get_next_track_duration() -> str
  - has_next_track() -> bool
  - is_adroll_playing() -> bool
  - update_lists()      # refresh blacklist/whitelist from config
  - force_refresh()     # invalidate XML cache
  - get_current_artist() -> str
```

---

## 4. ConfigManager - Configuration System

### 4.1 Config File Structure (config.json)

```json
{
  "stations": {
    "station_1047": {
      "name": "104.7 FM",
      "Messages": [
        {
          "Text": "Call 732.901.7777 to hear {artist}!",
          "Enabled": true,
          "Message Time": 15,
          "Scheduled": {
            "Enabled": false,
            "Days": ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
            "Times": [{"hour": 9}, {"hour": 10}, {"hour": 14}]
          }
        }
      ],
      "Ads": [...],
      "settings": {
        "now_playing_xml": "G:\\To_RDS\\nowplaying.xml",
        "radioboss": {
          "server": "http://192.168.3.12:9000",
          "password": "bmas220"
        },
        "rds": {
          "ip": "50.208.125.83",
          "port": 10001,
          "default_message": "732.901.7777 to SUPPORT and hear this program!"
        },
        "intro_loader": {...},
        "ad_inserter": {...}
      }
    },
    "station_887": {
      "name": "88.7 FM",
      "Messages": [...],
      "Ads": [...],
      "settings": {...}
    }
  },
  "shared": {
    "Whitelist": ["artist name 1", "artist name 2"],
    "Blacklist": ["artist name 3"],
    "playlist_presets": {},
    "debug": {"enable_debug_logs": false}
  }
}
```

### 4.2 Message JSON Schema

```json
{
  "Text": "string (max 64 chars, may contain {artist} and {title} placeholders)",
  "Enabled": "boolean",
  "Message Time": "integer (1-60 seconds, default 10)",
  "Scheduled": {
    "Enabled": "boolean",
    "Days": ["Sunday", "Monday", ...],
    "Times": [{"hour": 0}, {"hour": 1}, ...]
  }
}
```

### 4.3 Key Methods

```
- get_station_messages(station_id) -> list
- set_station_messages(station_id, messages)
- get_station_setting(station_id, dotted_key, default) # e.g., "rds.ip"
- get_xml_path(station_id) -> string
- get_shared_whitelist() / get_shared_blacklist() -> list
- save_config(make_backup=False, notify_observers=True)
- register_observer(callback) / unregister_observer(callback)
```

### 4.4 Observer Pattern

- `_observers`: list of callables
- `save_config()` calls `_notify_observers()` which calls each observer (outside lock)
- Thread safety: RLock for all config access
- Backups: timestamped `config_backup_YYYYMMDD_HHMMSS.json`

---

## 5. ConfigWindow - RDS Message Configuration UI

### 5.1 Window Properties

```
- Type: Modal Toplevel (transient + grab_set)
- Title: "Configure RDS Messages"
- Size: 1200x800
- Theme: Inherits from parent (ttkthemes)
```

### 5.2 Widget Hierarchy

```
ConfigWindow (Toplevel, modal)
  main_frame (ttk.Frame, padding=10)
    notebook (ttk.Notebook)
      Tab 0: "Station 104.7 FM" (station_1047)
        create_station_tab() -> PanedWindow
      Tab 1: "Station 88.7 FM" (station_887)
        create_station_tab() -> PanedWindow
```

### 5.3 Station Tab Layout (create_station_tab)

```
PanedWindow (horizontal)
  LEFT: LabelFrame "Scheduled Messages"
    Toolbar (Frame):
      [Add New] [Delete] [Move Up] [Move Down]
    Treeview (selectmode="browse", columns below):
      Column: "Text"      (heading "Message",   width=200, stretch=True,  anchor=W)
      Column: "Enabled"   (heading "Enabled",   width=60,  stretch=False, anchor=CENTER)
      Column: "Scheduled" (heading "Use Sched", width=70,  stretch=False, anchor=CENTER)
      Column: "Duration"  (heading "Dur (s)",   width=60,  stretch=False, anchor=CENTER)
      Column: "Days"      (heading "Days",      width=150, stretch=True,  anchor=W)
      Column: "Times"     (heading "Times (Hr)",width=100, stretch=True,  anchor=W)
    Vertical + Horizontal scrollbars

  RIGHT: LabelFrame "Message Details" (weight=2 in paned)
    create_station_details_widgets()
```

### 5.4 Detail Panel Widgets

```
LabelFrame "Message Content (64 char max)":
  Entry (textvariable, font="Segoe UI" 10, width=64)
  Label: "{count}/64" char counter (updates on trace_add "write")

LabelFrame "Message Settings":
  Grid layout:
    Row 0, Col 0: Checkbutton "Message Enabled" (BooleanVar)
    Row 0, Col 1: Label "Duration (seconds):"
    Row 0, Col 2: Spinbox (from=1, to=60, width=5, validate='key')

LabelFrame "Message Schedule":
  Checkbutton "Use Scheduling" (BooleanVar)
  Label: help text (font="Segoe UI" 9, gray #666666)

  Schedule container (shown/hidden based on Use Scheduling):
    Frame "Schedule Days:":
      7 day checkboxes: Sunday-Saturday
      Layout: divmod(i, 4) -> row, col; padx=5, pady=2
      Button: "Select All Days" / "Clear All Days" (toggles)

    Frame "Schedule Hours:":
      24 hour checkboxes (0-23) in AM/PM format
      Layout: divmod(h, 6) -> row, col; padx=2, pady=1
      AM/PM format: 0->"12 AM", 1->"1 AM"...11->"11 AM", 12->"12 PM"...23->"11 PM"
      Button: "Select All Hours" / "Clear All Hours" (toggles)

Frame (bottom):
  [Save Changes] [Cancel]
```

### 5.5 Hour AM/PM Formatting

```python
def format_hour_ampm(hour):  # hour: 0-23
    if hour == 0:   return "12 AM"
    elif hour == 12: return "12 PM"
    elif hour < 12:  return f"{hour} AM"
    else:            return f"{hour - 12} PM"
```

### 5.6 Message Operations

**Add**: Creates `{"Text": "New Message", "Enabled": False, "Message Time": 10, "Scheduled": {"Enabled": False, "Days": [], "Times": []}}`. Appends to list, reloads tree, selects new item.

**Delete**: No confirmation dialog. Removes from list, reloads tree, selects next available item (clamped to list bounds).

**Move Up/Down**: Uses `list.insert(idx-1, list.pop(idx))` pattern. Reloads tree, reselects moved item.

**No duplicate detection. No drag-and-drop.**

### 5.7 Live Update on Edit

- All detail widgets trigger `mark_station_changes(station_id)` on change.
- `mark_station_changes` calls `update_station_current_message_data()` which:
  - Reads all widget values into the message dict in memory
  - Calls `update_treeview_item()` to refresh the tree row
- Uses `is_loading_selection` flag to prevent feedback loops during selection loading.
- Duration validation: `1 <= int(value) <= 60`, empty string allowed.

### 5.8 Treeview Display Formatting

- Text column: truncated to 30 chars (`text[:27] + "..."` if > 30)
- Enabled: "Yes" / "No"
- Scheduled: "Yes" / "No"
- Duration: integer
- Days: comma-separated full names
- Times: comma-separated AM/PM formatted hours

### 5.9 Save/Cancel/Close

- **Save Changes**: For each station with pending changes, calls `config_manager.set_station_messages()` then `config_manager.save_config()`.
- **Cancel / Close (X)**: If any station has unsaved changes, shows Yes/No/Cancel dialog:
  - Yes: save and close
  - No: discard and close
  - Cancel: do nothing (stay open)

### 5.10 State Management

- Detail widgets disabled when no selection. Enabled on selection.
- Schedule controls (days/hours) disabled when "Use Scheduling" is unchecked, even if a message is selected.
- Toolbar buttons (Delete, Move Up, Move Down) disabled when no selection. Add New always enabled.
- Toggle buttons text updates: "Select All Days" <-> "Clear All Days" (same for hours).

### 5.11 Per-Station Storage

Each station maintains independent:
- `station_messages[station_id]`: list of message dicts (deep copy from config)
- `station_selected_indices[station_id]`: int or None
- `station_changes_pending[station_id]`: bool
- All widget variable references keyed by station_id

Tab switching triggers `on_tab_changed` which updates `self.current_station` and reloads the tree.
