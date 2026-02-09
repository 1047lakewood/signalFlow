---
name: ad-inserter-spec
description: Complete implementation specification for the ad scheduler, ad inserter service, play logger, report generator, and ad configuration/statistics UI
user_invocable: true
---

# Ad Inserter/Scheduler System - Complete Implementation Specification

Use this spec to implement the ad scheduling, insertion, logging, reporting, and configuration UI within signalFlow. All ad playback is handled internally by the engine — no external tools (RadioBoss, pydub, etc.) are involved.

---

## 1. AdSchedulerHandler - Intelligent Ad Scheduling (DONE)

### 1.1 Constants

```
LOOP_SLEEP                    = 60    # seconds - base check interval (actually dynamic, see 1.5)
TRACK_CHANGE_CHECK_INTERVAL   = 5     # seconds - how often to check for track changes
ERROR_RETRY_DELAY             = 300   # seconds (5 min) - wait after errors
```

### 1.2 Data Model

```rust
AdSchedulerHandler {
    engine: Arc<Mutex<Engine>>       // reference to the core engine
    running: AtomicBool
    last_hour_checked: Mutex<u32>
    last_track_check: Mutex<Instant>
    last_seen_track: Mutex<Option<String>>  // "artist - title"
    waiting_for_track_boundary: AtomicBool
    pending_lecture_check: AtomicBool
    is_hour_start: AtomicBool               // flag for station ID prepend
    lecture_detector: LectureDetector
    ad_service: AdInserterService
}
```

### 1.3 State Machine

Two boolean flags control the state:
- `waiting_for_track_boundary`: True when waiting for a track change to re-evaluate
- `pending_lecture_check`: True when a lecture check is pending after track change

Both are reset to False after an ad is played or when the scheduler decides not to wait.

### 1.4 Main Loop

```
while running:
    1. current_hour = now.hour

    2. HOUR BOUNDARY CHECK: if current_hour != last_hour_checked:
       - Log new hour detection
       - Check if first 5 seconds of hour -> set is_hour_start
       - Call perform_hourly_check() -> perform_lecture_check()
       - Update last_hour_checked
       - Reset is_hour_start after hourly check completes

    3. TRACK CHANGE CHECK: if (time_since_track_check >= 5s)
       AND (waiting_for_track_boundary OR pending_lecture_check):
       - Query engine for current track info
       - Compare against last_seen_track
       - If changed: call perform_lecture_check()
       - Update last_track_check

    4. DYNAMIC SLEEP CALCULATION:
       sleep_time = min(
         seconds_until_next_hour + 2,
         TRACK_CHANGE_CHECK_INTERVAL - time_since_last_track_check,
         60
       )
       sleep_time = max(sleep_time, 1)

    5. sleep(sleep_time)

    On exception: log, sleep ERROR_RETRY_DELAY (300s), continue
```

### 1.5 Track Change Detection

1. Query the engine for the currently playing track (artist, title).
2. Format as `"artist - title"` string.
3. Compare against `last_seen_track`.
4. If changed:
   - Update `last_seen_track`
   - If `waiting_for_track_boundary` or `pending_lecture_check`: call `perform_lecture_check()`

### 1.6 Lecture Check Decision Flow

**Priority rule**: Ads scheduled for this hour MUST play this hour.

```
CHECK 0: Playlist end detection
  - engine.has_next_track()
  - If no next track -> SKIP (don't insert ads), reset flags, return

CHECK 1: Safety margin (< 3 minutes left in hour)
  - minutes_remaining_in_hour() < 3
  - If true -> RUN INSTANT immediately, reset flags, return

CHECK 2: Does current track end this hour?
  - current_track_ends_this_hour()
  - If NO (ends next hour) -> RUN INSTANT immediately, reset flags, return
  - If YES -> continue to check 3

CHECK 3: Is next track a lecture?
  - lecture_detector.is_next_track_lecture()

  IF NEXT IS LECTURE:
    - Check: will_lecture_start_within_hour()
    - If YES -> RUN SCHEDULED insertion (insert as next track, wait for playback)
    - If NO  -> RUN INSTANT insertion
    - Reset flags, return

  IF NEXT IS NOT LECTURE:
    - Calculate: minutes_remaining_after_current_track()
    - If < 3 minutes -> RUN INSTANT (too risky to wait)
    - If >= 3 minutes -> SET waiting_for_track_boundary=true, pending_lecture_check=true
      (wait for next track change and re-evaluate)
    - Return

ON ANY ERROR: Fallback to INSTANT insertion to ensure ad plays this hour.
```

### 1.7 Time Calculation Methods

**minutes_remaining_in_hour()**: seconds until :59:59 / 60.0

**seconds_until_next_hour()**: seconds until next :00:00

**current_track_ends_this_hour()**: track_start + duration <= hour_end

**minutes_remaining_after_current_track()**: (hour_end - track_end) / 60.0

### 1.8 Service Invocation

**Scheduled insertion**: Calls `ad_service.insert_scheduled()` — queues ad roll as next track, waits for engine to confirm playback started.

**Instant insertion**: Calls `ad_service.insert_instant(is_hour_start)` — immediately interrupts current playback with ad roll.

### 1.9 Threading

- Spawns a background thread (or tokio task)
- start(): sets running=true, spawns thread
- stop(): sets running=false, joins thread

---

## 2. AdInserterService - Internal Ad Playback (DONE)

### 2.1 Constants

```
PLAYBACK_CONFIRM_TIMEOUT    = 60    # seconds - max wait for playback confirmation
PLAYBACK_POLL_INTERVAL      = 2     # seconds - how often to check engine state
```

### 2.2 Data Model

```rust
AdInserterService {
    engine: Arc<Mutex<Engine>>
    output_path: PathBuf              // path for concatenated ad roll file
    station_id_enabled: bool
    station_id_file: Option<PathBuf>
    ad_logger: AdPlayLogger
    lecture_detector: LectureDetector
}
```

### 2.3 Main Entry Points

**insert_scheduled()**: Concatenate ads, insert as next track in active playlist, wait for engine to confirm playback.

**insert_instant(is_hour_start: bool)**: Concatenate ads, immediately play via engine (stop mode or overlay), log plays immediately.

Both delegate to `run_insertion(mode, is_hour_start)`.

### 2.4 Insertion Workflow

```
Step 1: SELECT AND VALIDATE ADS
  a. Safety check: engine.has_next_track() — if playlist ended, skip
  b. For each ad in config:
     - Skip if not enabled
     - Skip if not scheduled for current time (is_scheduled check)
     - Skip if MP3 file missing or doesn't exist
     - Add to valid_files list, track ad_names
     - Calculate expected_duration from file metadata (via lofty)
  c. Return (valid_files, ad_names, expected_duration) or None if empty

Step 2: CONCATENATE MP3 FILES (internal)
  a. Create output directory if needed
  b. If is_hour_start AND station_id_enabled AND station_id_file exists:
     - Prepend station ID audio at beginning
  c. Concatenate all MP3 files using rodio/internal audio tools
     - Read each file, write sequentially to output_path
  d. Validate: check output file duration matches expected (within 500ms tolerance)
  e. Return {ok, expected_ms, actual_ms}

Step 3: INSERT INTO ENGINE
  IF INSTANT MODE:
    - engine.stop() current playback
    - engine.play(output_path) — play the concatenated ad roll
    - Log plays immediately for each ad
    - Return true

  IF SCHEDULED MODE:
    - engine.insert_next(output_path) — queue as next track
    - Poll engine state for playback confirmation:
      - Check if currently playing track artist == "adRoll"
      - Poll every PLAYBACK_POLL_INTERVAL (2s)
      - Timeout after PLAYBACK_CONFIRM_TIMEOUT (60s) or end of hour
    - On confirmation: log plays for each ad, return true
    - On timeout: log failure, return false
```

### 2.5 Ad Schedule Matching (is_scheduled)

```
1. If ad.scheduled is false: return true (always play when enabled)
2. Check day: ad.days contains current day name
   - If days is non-empty and current day not in list: return false
3. Check hour: ad.hours contains current hour (0-23)
   - If hours is non-empty and current hour not in list: return false
4. If scheduled but no hours specified: return true (any time)
```

### 2.6 Failure Handling

On any failure (concat, playback):
```
ad_logger.log_failure(ad_names, "prefix:error_message")
```
Prefix codes: "concat:", "playback:", "timeout:"

---

## 3. AdPlayLogger - Play Statistics Storage (DONE)

### 3.1 Storage Format - Plays (ad_plays.json)

Compact JSON:
```json
{"Ad Name":{"MM-DD-YY":[hour_int, hour_int, ...]}, ...}
```

- Date format: MM-DD-YY (2-digit year)
- Hours: list of integers 0-23 (may contain duplicates)
- Saved with compact serialization

### 3.2 Storage Format - Failures (ad_failures.json)

```json
[{"t":"MM-DD-YY HH:MM","ads":["Ad1","Ad2"],"err":"error_description"}]
```

- Maximum 50 entries (oldest discarded)

### 3.3 Key Methods

**log_play(ad_name)**: Appends current hour to the ad's date entry.

**log_failure(ad_names, error)**: Appends failure record, trims to 50 max.

**get_ad_statistics()**: Returns summary with total_ads, enabled_ads, total_plays, per-ad details sorted by play_count descending.

**get_ad_statistics_filtered(start_date, end_date)**: Same but filtered by date range.

**get_play_hours_for_date(ad_name, date_str)**: Returns sorted hours for a specific ad on a specific date.

**get_daily_play_counts(ad_name)**: Returns {date: count} map.

**get_failures()**: Returns full failure list.

**get_daily_confirmed_stats(start_date, end_date)**: Returns {"YYYY-MM-DD": {"Ad Name": count}}.

**get_hourly_confirmed_stats(start_date, end_date)**: Returns {"YYYY-MM-DD_HH": {"Ad Name": count}}.

**reset_all()**: Clears both plays and failures files.

---

## 4. AdReportGenerator - CSV & PDF Reports

### 4.1 Main Entry Point

**generate_report(start_date, end_date, advertiser_name, company_name)**:
1. Get confirmed ad totals for period
2. Filter to ads with plays > 0
3. For each played ad: generate both CSV and PDF
4. Returns (csv_path, pdf_path) tuple

### 4.2 CSV Report Format

```
VERIFIED Advertiser Report

Ad Name: [name]
Report Period: [start] to [end]
Generated: [timestamp]
Total Confirmed Plays: [count]
Hours with Airplay: [count]
Days with Airplay: [count]

HOURLY BREAKDOWN
Date,Hour,Plays
2026-01-11,09:00,1

DAILY SUMMARY
Date,Total Plays
2026-01-11,2

GRAND TOTAL,2
```

### 4.3 PDF Report Structure

- Title with optional company name
- Report info table (ad name, period, generated date)
- Summary box (total plays, hours/days with airplay, average per day)
- Hourly breakdown table with alternating row colors
- Daily summary table
- Footer

### 4.4 Multi-Ad Reports

**generate_multi_ad_report(ad_names, start_date, end_date, output_file, format)**:
- CSV: Matrix format with dates as rows, ad names as columns
- PDF: Same matrix as table with totals row

### 4.5 File Naming

```
REPORT_{ad_name}_{YYYYMMDD_HHMMSS}.csv
REPORT_{ad_name}_{YYYYMMDD_HHMMSS}.pdf
```

---

## 5. Ad Config JSON Schema

### 5.1 Ad Object

```json
{
  "name": "string",
  "enabled": true,
  "mp3_file": "G:\\Ads\\ad_file.mp3",
  "scheduled": false,
  "days": ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
  "hours": [9, 10, 11, 14, 15, 16]
}
```

### 5.2 Ad Inserter Settings

```json
"ad_inserter": {
  "output_mp3": "path/to/adRoll.mp3",
  "station_id_enabled": false,
  "station_id_file": ""
}
```

---

## 6. Ad Config UI (Tauri)

### 6.1 Window Properties

- Modal dialog
- Title: "Ad Inserter"
- Sections for ad CRUD, enable/disable, MP3 file picker, day/hour scheduling

### 6.2 Layout

```
LEFT: Ad list with enable/disable indicators
  - Listbox showing ads with enabled/disabled markers
  - Move up/down buttons
  - Add New / Delete buttons

RIGHT: Ad detail editor
  - Name field
  - Enabled checkbox
  - MP3 File path + Browse button (native file dialog, filtered to mp3/wav/flac/ogg)
  - Scheduled checkbox
  - Day checkboxes (Sun-Sat) with Select All / Clear All
  - Hour checkboxes (0-23 in AM/PM format) with Select All / Clear All

Bottom: Save & Close / Cancel
```

### 6.3 Ad Operations

**Add New**: Creates default ad with name="New Ad", enabled=true, empty mp3, not scheduled.

**Delete**: Confirmation dialog, removes from list.

**Move Up/Down**: Reorder within list.

### 6.4 Schedule Toggle

When "Scheduled" is unchecked: disable day/hour checkboxes.
When checked: enable them.

### 6.5 Save & Close

Persists all ad configs to engine config via IPC commands.

---

## 7. Ad Statistics UI (Tauri)

### 7.1 Window Properties

- Modal dialog
- Title: "Ad Play Statistics"

### 7.2 Layout

```
Controls:
  - Date filter (From/To date pickers + Apply/Clear)
  - Refresh Stats / Reset Counts buttons
  - Export Stats / Generate Report buttons
  - View Failures button

Summary:
  - Total Ads, Enabled Ads, Total Plays, Ads with Plays

Ad Details Table:
  - Columns: Name, Status, Play Count, Last Played, File
  - Sortable column headers

Play Calendar:
  - Ad selector dropdown
  - Month navigation (< Month Year >)
  - Calendar grid (Sun-Sat) with play indicators (dots)
  - Day click shows play times in detail panel
```

### 7.3 Calendar Logic

- Week starts Sunday
- Date format for lookups: MM-DD-YY (matches storage)
- Dot indicators: 1-5 plays = dots, 6+ = dot + count
- Day click shows play times in 12-hour format

### 7.4 Export & Reports

**Export Stats**: Save as JSON.

**Generate Report**: Generates CSV and PDF for each ad with plays in the filtered period.

**View Failures**: Sub-dialog showing recent failures in reverse chronological order.
