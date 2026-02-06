# signalFlow — Todo

## Phase A: Core Audio Engine

- [x] Multi-instance playback — Support multiple playlists in memory
- [x] Active context switching — Playing a track switches the active playlist context
- [x] Transport controls — Play, Stop, Skip Next, Seek
- [x] Crossfading — Configurable fade duration between tracks
- [x] Silence detection — Auto-skip when signal drops below threshold for X seconds

## Phase B: Playlist Management

- [x] Playlist CRUD — Remove, Reorder, Copy, Paste tracks within/between playlists
- [ ] Metadata enhancement — Calculated vs Played duration, filename fallback improvements
- [ ] Auto-Intro system — Check intros folder, play intro before matching artist tracks
- [ ] Auto-Intro dot indicator — Data structure flag for "has_intro" per track

## Phase C: Scheduler

- [ ] Scheduler data model — Scheduled events with time, mode, file path, priority
- [ ] Overlay mode — Play sound on top of current audio
- [ ] Stop mode — Kill current audio, play scheduled item
- [ ] Insert mode — Queue scheduled item as next track
- [ ] Conflict resolution — Define behavior when manual play conflicts with schedule
