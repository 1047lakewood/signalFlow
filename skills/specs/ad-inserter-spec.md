---
name: ad-inserter-spec
description: Complete implementation specification for the ad scheduler, ad inserter service, play logger, report generator, and ad configuration/statistics UI
user_invocable: true
---

# Ad Inserter/Scheduler System - Complete Implementation Specification

Use this spec to reproduce the ad scheduling, insertion, logging, reporting, and configuration UI in any language. Every constant, data structure, algorithm, and UI widget is documented below.

---

## 1. AdSchedulerHandler - Intelligent Ad Scheduling

### 1.1 Constants

```
LOOP_SLEEP                    = 60    # seconds - base check interval (actually dynamic, see 1.5)
TRACK_CHANGE_CHECK_INTERVAL   = 5     # seconds - how often to check for track changes
ERROR_RETRY_DELAY             = 300   # seconds (5 min) - wait after errors
```

### 1.2 Class Structure

```
AdSchedulerHandler(log_queue, config_manager, station_id):
  - config_manager: ConfigManager instance
  - station_id: string ("station_1047" or "station_887")
  - running: bool = False
  - thread: Thread = None (daemon)
  - last_hour_checked: int = datetime.now().hour
  - last_track_check: float = time.time()
  - last_file_modification: float = 0
  - last_seen_track: string = None  (format: "artist - title")
  - waiting_for_track_boundary: bool = False
  - pending_lecture_check: bool = False
  - _is_hour_start: bool = False  (flag for station ID prepend)
  - lecture_detector: LectureDetector instance
  - ad_service: AdInserterService instance
  - Logger name: "AdScheduler_{station_number}"
```

### 1.3 State Machine

Two boolean flags control the state:
- `waiting_for_track_boundary`: True when waiting for a track change to re-evaluate
- `pending_lecture_check`: True when a lecture check is pending after track change

Both are reset to False after an ad is played or when the scheduler decides not to wait.

### 1.4 Main Loop (run method)

```
while running:
    1. current_hour = datetime.now().hour

    2. HOUR BOUNDARY CHECK: if current_hour != last_hour_checked:
       - Log new hour detection
       - Check if first 5 seconds of hour (current_second <= 5) -> set _is_hour_start
       - Call _perform_hourly_check() -> _perform_lecture_check()
       - Update last_hour_checked
       - Reset _is_hour_start after hourly check completes

    3. TRACK CHANGE CHECK: if (time_since_track_check >= 5s)
       AND (lecture_detector exists AND ad_service exists)
       AND (waiting_for_track_boundary OR pending_lecture_check):
       - Call _check_for_track_change()
       - Update last_track_check

    4. DYNAMIC SLEEP CALCULATION:
       sleep_time = min(
         seconds_until_next_hour + 2,     # +2 buffer to ensure boundary crossed
         TRACK_CHANGE_CHECK_INTERVAL - time_since_last_track_check,
         60                                # safety cap
       )
       sleep_time = max(sleep_time, 1)     # minimum 1 second

    5. time.sleep(sleep_time)

    On exception: log, sleep ERROR_RETRY_DELAY (300s), continue
```

### 1.5 Track Change Detection (_check_for_track_change)

1. Check file modification time via `os.path.getmtime(xml_path)`.
2. If mtime differs from `last_file_modification`, mark as changed, update stored mtime.
3. Force refresh XML via `lecture_detector.force_refresh()`.
4. Get current track info: `"artist - title"` string.
5. If file changed OR track content changed (string comparison):
   - Update `last_seen_track`
   - If `waiting_for_track_boundary` or `pending_lecture_check`: call `_perform_lecture_check()`

### 1.6 Lecture Check Decision Flow (_perform_lecture_check)

**Priority rule**: Ads scheduled for this hour MUST play this hour.

```
CHECK 0: Playlist end detection
  - lecture_detector.has_next_track()
  - If no next track -> SKIP (don't insert ads), reset flags, return

CHECK 1: Safety margin (< 3 minutes left in hour)
  - _minutes_remaining_in_hour() < 3
  - If true -> RUN INSTANT immediately, reset flags, return

CHECK 2: Does current track end this hour?
  - _current_track_ends_this_hour()
  - If NO (ends next hour) -> RUN INSTANT immediately, reset flags, return
  - If YES -> continue to check 3

CHECK 3: Is next track a lecture?
  - lecture_detector.is_next_track_lecture()

  IF NEXT IS LECTURE:
    - Check: _will_lecture_start_within_hour()
    - If YES -> RUN SCHEDULE service (with XML polling confirmation)
    - If NO  -> RUN INSTANT service
    - Reset flags, return

  IF NEXT IS NOT LECTURE:
    - Calculate: _minutes_remaining_after_current_track()
    - If < 3 minutes -> RUN INSTANT (too risky to wait)
    - If >= 3 minutes -> SET waiting_for_track_boundary=True, pending_lecture_check=True
      (wait for next track change and re-evaluate)
    - Return

ON ANY ERROR: Fallback to INSTANT service to ensure ad plays this hour.
```

### 1.7 Time Calculation Methods

**_minutes_remaining_in_hour()**:
```
hour_end = now.replace(minute=59, second=59, microsecond=999999)
return (hour_end - now).total_seconds() / 60.0
```

**_seconds_until_next_hour()**:
```
next_hour = (now + 1 hour).replace(minute=0, second=0, microsecond=0)
return (next_hour - now).total_seconds()
```

**_current_track_ends_this_hour()**:
```
track_end = track_start_time + timedelta(seconds=duration)
hour_end = now.replace(minute=59, second=59, microsecond=999999)
return track_end <= hour_end
```

**_minutes_remaining_after_current_track()**:
```
track_end = track_start_time + timedelta(seconds=duration)
hour_end = now.replace(minute=59, second=59, microsecond=999999)
return (hour_end - track_end).total_seconds() / 60.0
```

**_parse_duration_to_seconds(duration_str)**:
- Supports "MM:SS" and "H:MM:SS" formats
- Returns int seconds or None on error

**_get_current_track_start_time()**:
- Parses STARTED attribute from XML TRACK element
- Format: "YYYY-MM-DD HH:MM:SS" -> `datetime.strptime(s, "%Y-%m-%d %H:%M:%S")`
- Warns if timestamp is stale (> 2 hours old)

### 1.8 Service Invocation

**_run_schedule_service()**: Calls `ad_service.run()` - scheduled mode with XML polling confirmation.

**_run_instant_service()**: Calls `ad_service.run_instant(is_hour_start=self._is_hour_start)` - instant mode, no polling.

### 1.9 Threading

- Thread: `daemon=True`, name="AdSchedulerThread"
- start(): creates thread, sets running=True
- stop(): sets running=False, joins thread with 5s timeout

---

## 2. AdInserterService - Ad Concatenation & Triggering

### 2.1 Constants

```
XML_POLL_INTERVAL           = 2     # seconds between XML checks during polling
XML_POLL_TIMEOUT            = 60    # default max seconds for XML confirmation
CONCAT_DURATION_TOLERANCE_MS = 500  # allowed deviation in ms for duration validation
```

### 2.2 Class Structure

```
AdInserterService(config_manager, station_id):
  - insertion_url: string  (constructed via config_manager.get_ad_inserter_insertion_url())
  - instant_url: string    (constructed via config_manager.get_ad_inserter_instant_url())
  - output_mp3: string     (from config: ad_inserter.output_mp3)
  - station_id_enabled: bool (from config: ad_inserter.station_id_enabled, default False)
  - station_id_file: string  (from config: ad_inserter.station_id_file, default "")
  - xml_path: string       (from config_manager.get_xml_path())
  - _xml_reader: NowPlayingReader (if available)
  - ad_logger: AdPlayLogger (if available)
  - lecture_detector: LectureDetector (for playlist-end detection)
  - Logger name: "AdService_{station_number}"
```

### 2.3 RadioBoss URL Construction

URLs are constructed by ConfigManager:
```
{server}/?pass={password}&action=schedule&type=run&id={event_id}

insertion_url uses: ad_inserter.insertion_event_id (default "INSERT")
instant_url uses:   ad_inserter.instant_event_id (default "PLAY")
server:             radioboss.server (default "http://localhost:9000")
password:           radioboss.password (default "password")
```

### 2.4 Main Entry Points

**run()**: Scheduled mode - concatenate, call insertion URL, poll XML for confirmation.
**run_instant(is_hour_start=False)**: Instant mode - concatenate, call instant URL, log immediately (no polling).

Both delegate to `_run_with_confirmation(url, mode, is_hour_start)`.

### 2.5 Insertion Workflow (_run_with_confirmation)

```
Step 1: SELECT AND VALIDATE ADS (_select_valid_ads)
  a. Safety check: if lecture_detector available, verify has_next_track()
     - If no next track (playlist ended): return None (skip)
  b. For each ad in config_manager.get_station_ads(station_id):
     - Skip if not Enabled
     - Skip if not scheduled for current time (_is_scheduled)
     - Skip if MP3File missing or doesn't exist
     - Add MP3 path to valid_files, name to ad_names
     - Calculate expected_duration_ms via pydub AudioSegment
  c. Return (valid_files, ad_names, expected_duration_ms) or None if empty

Step 2: CONCATENATE MP3 FILES (_concatenate_and_validate)
  a. Create output directory if needed
  b. If is_hour_start AND station_id_enabled AND station_id_file exists:
     - Prepend station ID audio at beginning of concatenation
     - Add its duration to expected_duration_ms
  c. Concatenate all MP3 files using pydub:
     combined = AudioSegment.empty()
     [optional station ID prepend]
     for each file: combined += AudioSegment.from_mp3(file)
  d. Export with artist tag: combined.export(output_mp3, format="mp3", tags={"artist": "adRoll"})
  e. Validate: re-read output file, check duration within CONCAT_DURATION_TOLERANCE_MS (500ms)
  f. Return {ok: bool, expected_ms: float, actual_ms: float, error?: string}

Step 3: CALL INSERTION URL (_call_url_with_result)
  a. urllib.request.urlopen(url, timeout=10)
  b. Return {ok: bool (200-299), status_code: int, error?: string}

Step 4: CONFIRMATION (mode-dependent)
  IF INSTANT MODE:
    - Log plays immediately, skip XML polling
    - Return True

  IF SCHEDULED MODE:
    - Calculate dynamic timeout: seconds until hour end (minimum 60s)
    - Poll XML for ARTIST=="adRoll" confirmation
    - Uses NowPlayingReader.wait_for_artist() if available
    - Poll interval: XML_POLL_INTERVAL (2s)
    - On confirmation: log plays for each ad, return True
    - On timeout: log failure, return False
```

### 2.6 Ad Schedule Matching (_is_scheduled)

```
1. If ad.Scheduled is False: return True (always play when enabled)
2. Check day: ad.Days list contains current day name (strftime("%A"))
   - If Days is non-empty and current day not in list: return False
3. Check hour (two formats):
   a. ad.Hours: list of ints [0, 1, ..., 23]  (preferred)
      - If now.hour in Hours: return True, else return False
   b. ad.Times: list of {"hour": int}  (legacy fallback)
      - Same logic with dict access
4. If scheduled but no hours/times specified: return True (any time)
```

### 2.7 Failure Handling

On any failure (concat, HTTP, XML timeout):
```
ad_logger.log_failure(ad_names, "prefix:error_message[:20]")
```
Prefix codes: "concat:", "http:", "xml:"

---

## 3. AdPlayLogger - Play Statistics Storage

### 3.1 Storage Format - Plays (ad_plays_{station}.json)

Ultra-compact JSON with no separators whitespace:
```json
{"Ad Name":{"MM-DD-YY":[hour_int, hour_int, ...]}, ...}
```

Example:
```json
{"Morning Show Ad":{"01-11-26":[14,16,19],"01-12-26":[9,11]}}
```

- Date format: MM-DD-YY (2-digit year)
- Hours: list of integers 0-23 (may contain duplicates if played multiple times in same hour)
- Saved with `json.dump(data, f, separators=(',', ':'))` for minimal size

### 3.2 Storage Format - Failures (ad_failures_{station}.json)

```json
[{"t":"MM-DD-YY HH:MM","ads":["Ad1","Ad2"],"err":"error_description"}]
```

- Maximum 50 entries (last 50 kept, oldest discarded)
- Timestamp format: "MM-DD-YY HH:MM"

### 3.3 File Paths

```
user_data/ad_plays_{station_number}.json     # e.g., ad_plays_1047.json
user_data/ad_failures_{station_number}.json  # e.g., ad_failures_1047.json
```

### 3.4 Thread Safety

All methods that read/write files use `threading.RLock`:
```python
with self._lock:
    plays = self._load_plays()
    # ... modify ...
    self._save_plays(plays)
```

### 3.5 Key Methods

**log_play(ad_name: str) -> bool**:
```python
date_str = now.strftime("%m-%d-%y")
hour = now.hour
plays[ad_name][date_str].append(hour)
```

**log_failure(ad_names: list, error: str) -> bool**:
```python
failures.append({"t": now.strftime("%m-%d-%y %H:%M"), "ads": ad_names, "err": error})
if len(failures) > MAX_FAILURES (50):
    failures = failures[-MAX_FAILURES:]
```

**get_ad_statistics() -> dict**:
Returns:
```json
{
  "total_ads": int,
  "enabled_ads": int,
  "total_plays": int,
  "ads_with_plays": int,
  "ad_details": [
    {
      "name": str,
      "enabled": bool,
      "play_count": int,
      "last_played": "ISO datetime string or None",
      "mp3_file": str,
      "scheduled": bool
    }
  ]
}
```
- ad_details sorted by play_count descending

**get_ad_statistics_filtered(start_date, end_date)**: Same as above but filters by date range (YYYY-MM-DD format inputs, compared against MM-DD-YY storage).

**get_play_hours_for_date(ad_name, date_str) -> list[int]**: Returns sorted hours for a specific ad on a specific date (MM-DD-YY format).

**get_daily_play_counts(ad_name) -> dict**: Returns `{date_str: play_count}`.

**get_failures() -> list**: Returns full failure list.

**get_daily_confirmed_stats(start_date, end_date) -> dict**: Returns `{"YYYY-MM-DD": {"Ad Name": count}}` for report generator.

**get_hourly_confirmed_stats(start_date, end_date) -> dict**: Returns `{"YYYY-MM-DD_HH": {"Ad Name": count}}` for report generator.

**get_confirmed_ad_totals(start_date, end_date) -> dict**: Returns `{"Ad Name": total_count}`.

**reset_all() -> bool**: Clears both plays and failures files.

### 3.6 Migration from Old Format

On init, checks for legacy `ad_play_events_{station}.json` file:
- Old format: `{"hourly_confirmed": {"YYYY-MM-DD_HH": {"Ad Name": count}}}`
- Migrates to compact format
- Renames old file to `_migrated.json`

### 3.7 Last Played Calculation

Finds the most recent date+hour combination:
```python
for date_str, hours in ad_plays.items():
    dt = datetime.strptime(date_str, "%m-%d-%y")
    max_hour = max(hours)
    if dt > latest_date or (dt == latest_date and max_hour > latest_hour):
        update latest
return latest_date.replace(hour=latest_hour).isoformat()
```

---

## 4. AdReportGenerator - CSV & PDF Reports

### 4.1 Class Structure

```
AdReportGenerator(ad_logger: AdPlayLogger, station_id: str):
  - ad_logger: AdPlayLogger instance
  - station_id: string
  - Logger name: "AdReportGenerator_{station_number}"
  - Uses reportlab (optional dependency) for PDF
```

### 4.2 Main Entry Point

**generate_report(start_date=None, end_date=None, advertiser_name=None, company_name=None)**:
1. Get confirmed ad totals for period
2. Filter to ads with plays > 0
3. For each played ad: generate both CSV and PDF
4. Returns (csv_path, pdf_path) tuple (first files)
5. Falls back to legacy report if no confirmed events

### 4.3 CSV Report Format (Confirmed)

```
VERIFIED Advertiser Report (XML Confirmed)

Ad Name: [name]
Report Period: [start] to [end]
Generated: [timestamp]
Total Confirmed Plays: [count]
Hours with Airplay: [count]
Days with Airplay: [count]

==================================================
HOURLY BREAKDOWN (XML Confirmed)
==================================================
Date,Hour,Plays
2026-01-11,09:00,1
2026-01-11,14:00,1

HOURLY TOTAL,,2

==================================================
DAILY SUMMARY
==================================================
Date,Total Plays
2026-01-11,2

GRAND TOTAL,2

This report contains only XML-confirmed ad plays.
```

### 4.4 PDF Report Structure (Confirmed)

Uses reportlab with letter pagesize:

**Styles**:
- Title: fontSize=22, color=#1a1a1a, centered
- Subtitle: fontSize=12, color=#2e7d32, centered, text="VERIFIED - XML Confirmed Plays Only"
- Heading: fontSize=14, color=#333333

**Sections**:
1. Title (optional company name)
2. Report info table (ad name, period, generated date, optional advertiser)
3. Summary box (green theme #e8f5e9 bg, #a5d6a7 borders):
   - Total Confirmed Plays, Hours with Airplay, Days with Airplay, Average Plays per Day
4. Hourly breakdown table:
   - Header: green #2e7d32 bg, white text
   - Alternating rows: white / #f1f8e9
   - Total row: #c8e6c9 bg
5. Daily summary table:
   - Header: blue #4472C4 bg, white text
   - Alternating rows: white / #f9f9f9
   - Total row: #d9d9d9 bg
6. Footer: "This report contains ONLY XML-confirmed ad plays..."

### 4.5 Multi-Ad Reports

**generate_multi_ad_report(ad_names, start_date, end_date, output_file, format="csv")**:
- CSV: Matrix format with dates as rows, ad names as columns, daily totals
- PDF: Same matrix as table with totals row

### 4.6 File Naming Convention

```
VERIFIED_REPORT_{ad_name}_{YYYYMMDD_HHMMSS}.csv
VERIFIED_REPORT_{ad_name}_{YYYYMMDD_HHMMSS}.pdf
```
Spaces and slashes in ad_name replaced with underscores.

---

## 5. Ad Config JSON Schema

### 5.1 Ad Object

```json
{
  "Name": "string",
  "Enabled": true,
  "MP3File": "G:\\Ads\\ad_file.mp3",
  "Scheduled": false,
  "Days": ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
  "Hours": [9, 10, 11, 14, 15, 16],
  "PlayCount": 0,
  "LastPlayed": null
}
```

- Hours: list of integers 0-23 (preferred new format)
- Legacy Times format: `[{"hour": 9}, {"hour": 10}]` (still supported for reading)
- Days: list of full day names

### 5.2 Station Ad Inserter Settings

```json
"ad_inserter": {
  "insertion_event_id": "INSERT",
  "instant_event_id": "PLAY",
  "output_mp3": "G:\\Ads\\adRoll_1047.mp3",
  "station_id_enabled": false,
  "station_id_file": ""
}
```

---

## 6. AdInserterWindow - Ad Configuration UI

### 6.1 Window Properties

```
- Type: Modal Toplevel (transient + grab_set)
- Title: "Ad Inserter"
- Size: 850x650
- Min size: 750x550
```

### 6.2 Widget Hierarchy

```
AdInserterWindow (Toplevel, modal)
  main_frame (ttk.Frame, padding=10)
    notebook (ttk.Notebook)
      Tab 0: "104.7 FM" (station_1047)
        create_station_tab()
      Tab 1: "88.7 FM" (station_887)
        create_station_tab()
    button_frame:
      [Save & Close] [Cancel]
```

### 6.3 Station Tab Layout

```
LEFT: LabelFrame "Advertisements" (padding=5, fill=Y)
  list_container (Frame):
    Listbox (width=25, height=15, font="Segoe UI" 10, exportselection=False)
      Format: "checkmark ad_name" -> "tick_mark ad_name" or "cross_mark ad_name"
      Display: "{enabled_marker} {ad_name}" where marker = "check" if enabled else "cross"
    Move buttons (Frame, right of listbox):
      [up_arrow] (width=3)
      [down_arrow] (width=3)
  list_buttons (Frame):
    [Add New] [Delete]

RIGHT: LabelFrame "Ad Details" (padding=5, fill=BOTH, expand=True)
  Row 0: Label "Name:" (bold) + Entry (width=50, font="Segoe UI" 10)
    - Binds: FocusOut -> auto_save, KeyRelease -> delayed_auto_save (1s debounce)
  Row 1: Checkbutton "Enabled" (Toggle style)
    - command: auto_save
  Row 2: Label "MP3 File:" (bold) + Entry (width=35) + [Browse] button (width=8)
    - Browse: filedialog.askopenfilename, filetypes=[("MP3 Files","*.mp3")]
    - FocusOut: auto_save
  Row 3: Checkbutton "Scheduled" (Toggle style)
    - command: toggle_schedule + auto_save
  Row 4: Label "Days:" (bold) + 7 day checkboxes (Sun-Sat, abbreviated to 3 chars)
    - Pack side=LEFT, padx=2
    - command: auto_save
  Row 5: Button "Select All Days" / "Clear All Days"
  Row 6: Label "Hours:" (bold) + 24 hour checkboxes
    - Grid: row=h//6, column=h%6, padx=1, pady=1
    - AM/PM format (same as RDS config)
    - width=6
    - command: auto_save
  Row 7: Button "Select All Hours" / "Clear All Hours"
```

### 6.4 Listbox Display Format

```python
enabled_marker = "check_mark" if ad.get('Enabled', False) else "cross_mark"
listbox.insert(END, f"{enabled_marker} {ad_name}")
```

### 6.5 Auto-Save Behavior

- **On checkbox/dropdown change**: Immediate `auto_save_current_ad(station_id)` call.
- **On name entry KeyRelease**: Debounced with `self.after(1000, ...)` - saves after 1 second of no typing. Previous timer cancelled on each keystroke.
- **On FocusOut**: Immediate save.
- `auto_save_current_ad` -> `save_current_ad` which updates the in-memory ad dict from widget values.

### 6.6 Save Current Ad (save_current_ad)

```python
ad['Name'] = name_var.get()
ad['Enabled'] = enabled_var.get()
ad['MP3File'] = mp3_var.get()
ad['Scheduled'] = scheduled_var.get()
ad['Days'] = [day for day, var in day_vars.items() if var.get()]
ad['Hours'] = [h for h, var in hour_vars.items() if var.get()]
```
Then repopulates the listbox to reflect changes.

### 6.7 Ad Operations

**Add New**: Creates default ad dict:
```json
{"Name":"New Ad", "Enabled":true, "MP3File":"", "Scheduled":false,
 "Days":[], "Hours":[], "PlayCount":0, "LastPlayed":null}
```
Selects the new item.

**Delete**: Shows confirmation dialog (`messagebox.askyesno`). Removes from list, clears detail fields.

**Move Up/Down**: Swaps adjacent items in list, repopulates listbox, reselects moved item.

### 6.8 Schedule Toggle

When "Scheduled" checkbox changes:
- If unchecked: disable all day checkboxes, hour checkboxes, and Select All buttons
- If checked: enable all day checkboxes, hour checkboxes, and Select All buttons

### 6.9 Save & Close

1. For each station: if current_index is set, save current ad to memory.
2. For each station: `config_manager.set_station_ads(station_id, ads)`.
3. `config_manager.save_config()`.
4. Close window.

### 6.10 Close Handling

Compares current ads to `initial_ads` (deep copy from init). If different, asks "You have unsaved changes. Close anyway?" (Yes/No).

### 6.11 State Management

- Detail controls disabled when no selection. Enabled on selection.
- Move Up/Down/Delete buttons disabled when no selection.
- Schedule fields (days/hours) respect both selection state AND scheduled checkbox state.
- Uses `copy.deepcopy()` for initial ads snapshot comparison.

---

## 7. AdStatisticsWindow - Play Statistics & Calendar UI

### 7.1 Window Properties

```
- Type: Modal Toplevel (transient + grab_set)
- Title: "Ad Play Statistics"
- Size: 1050x920
- Min size: 950x820
- Centered on screen
```

### 7.2 Dependencies

```python
from tkcalendar import DateEntry  # for date filter inputs
from ad_play_logger import AdPlayLogger
from ad_report_generator import AdReportGenerator
```

### 7.3 Widget Hierarchy

```
AdStatisticsWindow
  window (Toplevel, modal)
    main_frame (ttk.Frame, padding=10)
      notebook (ttk.Notebook)
        Tab 0: "104.7 FM"
          create_station_tab()
        Tab 1: "88.7 FM"
          create_station_tab()
      button_frame:
        [Close]
```

### 7.4 Station Tab Layout

```
controls_frame (Frame):
  LEFT: LabelFrame "Date Filter" (padding=10):
    Label "From:" + DateEntry (width=12, pattern='yyyy-mm-dd')
    Label "To:" + DateEntry (width=12, pattern='yyyy-mm-dd')
    [Apply] (width=8) [Clear] (width=8)
  RIGHT: button_frame:
    Row 1: [Refresh Stats] (w=14) [Reset Counts] (w=14)
    Row 2: [Export Stats] (w=14) [Generate Report] (w=14)
    Row 3: [View Failures] (w=14)

summary_frame: LabelFrame "Summary" (padding=10):
  Grid layout (2 rows x 4 cols):
    "Total Ads:" [value]     "Enabled Ads:" [value]
    "Total Plays:" [value]   "Ads with Plays:" [value]
  Values: bold font "Segoe UI" 10

table_frame: LabelFrame "Ad Details" (padding=10):
  Treeview (columns: Name, Status, Play Count, Last Played, File)
    - height=6
    - Column headings clickable for sorting
    - Scrollbars: vertical + horizontal
    - Column width=120, minwidth=80

calendar_frame: LabelFrame "Play Calendar" (padding=10):
  (see Calendar section below)

status_bar: Label (relief=SUNKEN, anchor=W)
```

### 7.5 Sortable Treeview

Clicking a column header triggers `sort_column(station_id, col)`:
- Toggles sort direction if same column clicked again
- "Play Count" column sorts numerically (`int()`), all others sort alphabetically
- Uses `tree.move()` to rearrange items in place

### 7.6 Calendar View

```
top_row (Frame):
  Label "Ad:" + Combobox (readonly, width=30)
    - Populated with ad names from config
    - On selection: cache play data, refresh grid
  nav_frame (Frame):
    [<] (width=3) + month_label (width=20, bold) + [>] (width=3)

content_frame (Frame):
  LEFT: grid_frame - Calendar Grid
    Row 0: Day headers: Sun Mon Tue Wed Thu Fri Sat
      - Labels: width=7, centered, font="Segoe UI" 11 bold
    Rows 1-6: 7 columns of day cells
      Each cell: Frame (width=60, height=45, propagate=False)
        day_label: text=day_number, font="Segoe UI" 12
          - place(relx=0.5, rely=0.3, anchor=CENTER)
        dot_label: play indicator, color=#28a745 (green), font="Segoe UI" 11 bold
          - place(relx=0.5, rely=0.72, anchor=CENTER)
          - Indicator format:
            1-5 plays: "dots" (bullet characters repeated)
            6+ plays: "dot{count}" (single bullet + number)

  RIGHT: LabelFrame "Play Details" (padding=10, width=200)
    detail_label: wraplength=250, justify=LEFT
    Shows: date, play times in 12-hour format, total count
```

### 7.7 Calendar Logic

**Week start**: Sunday (uses `calendar.Calendar(firstweekday=6)`)

**Date format for lookups**: `MM-DD-YY` (matches AdPlayLogger storage format)
```python
date_str = f"{month:02d}-{day:02d}-{year % 100:02d}"
```

**Grid population**:
1. `cal.monthdayscalendar(year, month)` returns list of weeks (0 = empty)
2. For each day: check if `date_str` exists in `daily_stats_cache`
3. If plays > 0: show dot indicator
4. Bind click handler to cell frame + both labels

**Day click handler**:
- If no ad selected: show "Please select an ad first"
- If no plays: show "No plays for {ad_name}"
- If plays: call `ad_logger.get_play_hours_for_date()`, format as 12-hour times:
  ```
  January 15, 2026

  Played at:
    9:00 AM
    2:00 PM
    4:00 PM

  Total: 3 plays
  ```

### 7.8 Date Filtering

- `apply_date_filter()`: Sets `date_filter_active=True`, calls `refresh_stats()`.
- `clear_date_filter()`: Resets flag, resets date entries to today, calls `refresh_stats()`.
- `refresh_stats()` uses `get_ad_statistics_filtered()` when filter is active, otherwise `get_ad_statistics()`.

### 7.9 Export & Reports

**Export Stats**: `filedialog.asksaveasfilename()` -> JSON dump of `get_ad_statistics()`.

**Generate Report**: Calls `report_generator.generate_report(start_date, end_date)` which generates both CSV and PDF for each ad with plays.

**View Failures**: Opens sub-dialog (Toplevel, 500x400, modal):
- Shows last N failures in reverse chronological order (newest first)
- Format per failure: `"{timestamp}  {ad_names}\n  Error: {error}\n\n"`
- Uses read-only Text widget with Consolas font size 9
- Scrollbar + Close button

### 7.10 Reset Counts

Shows confirmation dialog. Calls `ad_logger.reset_all_play_counts()` then `refresh_stats()`.

### 7.11 Per-Station State

Each station maintains independent:
- ad_logger: AdPlayLogger instance
- report_generator: AdReportGenerator instance
- date_filter_active: bool
- sort_column_name / sort_reverse: for treeview sorting
- calendar_year / calendar_month: current calendar view
- selected_ad: currently selected ad name in calendar
- daily_stats_cache: dict (cached play counts for selected ad)
