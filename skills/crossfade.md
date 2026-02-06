# Crossfade — Design Doc (DONE)

## Goal
Configurable fade duration between tracks. Fade-out of ending track overlaps with fade-in of next track. Linear curve initially.

## Architecture: Independent Sink Approach

Player creates independent sinks via `create_sink()` from the shared `OutputStreamHandle`. No persistent sink array — each track gets its own sink:

1. Track N plays on sink A (created via `play_file_new_sink()`)
2. When `elapsed >= track_duration - crossfade_duration`:
   - Start track N+1 on a new sink B with `source.fade_in(crossfade_duration)`
   - Ramp sink A volume from 1.0 → 0.0 over crossfade_duration (linear steps every 50ms)
3. When crossfade completes: stop sink A (dropped), sink B becomes current
4. Repeat

### Edge Cases
- Track shorter than 2x crossfade_duration → skip crossfade, play normally
- Last track in playlist → just fade out (no next track to fade in)
- Track duration unknown (0) → skip crossfade for that transition
- Crossfade duration 0.0 → original sequential behavior (no overlap)

## Data Model Changes

### Engine
- `crossfade_secs: f32` — persisted config, default 0.0

### Player
- `create_sink()` — creates independent sinks on `stream_handle`; no persistent `sinks` array or `active_sink` index
- Existing methods (stop, pause, resume, etc.) operate on the default sink

## CLI Changes
- `config crossfade <seconds>` — set default crossfade duration
- `play --crossfade <seconds>` — override for this session
