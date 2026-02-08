# signalFlow — Scheduler

## Overview

The scheduler system provides time-based event triggers for radio automation. Events fire at configured times and interact with playback via three modes.

## Data Model (DONE)

### ScheduleMode (enum)
- `Overlay` — play sound on top of current audio (jingles, FX)
- `Stop` — kill current audio, play scheduled item (hard news break)
- `Insert` — queue scheduled item as next track in active playlist

### ScheduleEvent (struct)
| Field    | Type          | Description                                    |
|----------|---------------|------------------------------------------------|
| id       | u32           | Unique identifier                              |
| time     | NaiveTime     | Time of day (HH:MM:SS)                         |
| mode     | ScheduleMode  | How to interact with playback                  |
| file     | PathBuf       | Audio file to play                             |
| priority | Priority(u8)  | 1-9, higher wins conflicts (default: 5)        |
| enabled  | bool          | Whether event is active (default: true)        |
| label    | Option<String>| Optional description                           |
| days     | Vec<u8>       | Days of week (0=Mon..6=Sun), empty = daily     |

### Schedule (struct)
- `events: Vec<ScheduleEvent>` — all scheduled events
- `next_id: u32` — auto-incrementing ID counter
- CRUD: `add_event()`, `remove_event()`, `find_event()`, `toggle_event()`
- `events_by_time()` — sorted view

### Priority
- Constants: `LOW(1)`, `NORMAL(5)`, `HIGH(9)`
- Used for conflict resolution (higher priority wins)

## CLI Commands (DONE)

```
schedule add <time> <mode> <file> [-p <priority>] [-l <label>] [-d <days>]
schedule list
schedule remove <id>
schedule toggle <id>
```

### Examples
```
signalflow schedule add 14:00 stop news_open.mp3 -p 9 -l "Afternoon news"
signalflow schedule add 08:00 overlay jingle.mp3 -d 0,1,2,3,4
signalflow schedule list
signalflow schedule remove 3
signalflow schedule toggle 2
```

## Engine Integration (DONE)

- `Engine.schedule: Schedule` — persisted in state JSON
- `#[serde(default)]` for backward compatibility with existing state files
- `status` command shows schedule event count

## Overlay Mode Execution (DONE)

- `Player::play_overlay(path)` — plays a file on a new independent sink, blocks until finished
- CLI: `overlay <file>` — plays a sound on top of current audio (OS-level mixing via WASAPI shared mode)
- Validates file existence before attempting playback
- Works alongside `play` command running in another terminal — true overlay behavior

## Stop Mode Execution (DONE)

- `Player::play_stop_mode(path)` — stops the default sink (kills current audio), plays file on a new sink, blocks until finished
- CLI: `interrupt <file>` — stops current audio and plays the specified file (hard break)
- Validates file existence before attempting playback
- In the current CLI architecture (separate processes), establishes the API that the scheduler monitoring loop will use to truly interrupt in-process playback

## Insert Mode Execution (DONE)

- `Engine::insert_next_track(path)` — creates a Track from the file path and inserts it after `current_index` in the active playlist (position 0 if no current track)
- CLI: `insert <file>` — inserts a file as the next track in the active playlist
- Validates file existence before attempting insertion
- In the current CLI architecture (separate processes), establishes the API that the scheduler monitoring loop will use to queue tracks during live playback

## Not Yet Built

- Real-time schedule monitoring loop
- Conflict resolution logic

## Tests

25 unit tests (19 scheduler + 1 overlay + 1 stop mode + 1 stop mode rejects missing file + 3 insert mode):
- 3 time parsing (HH:MM, HH:MM:SS, invalid)
- 4 mode tests (from_str × 4, display × 3)
- 2 CRUD (add+find, remove)
- 1 remove not found
- 2 toggle
- 1 events_by_time sorting
- 1 unique IDs
- 1 serialization roundtrip
- 2 days display
- 1 priority ordering
- 1 defaults from JSON
- 1 play_stop_mode_rejects_missing_file
- 3 insert mode (insert at beginning, insert after current, no active playlist)
