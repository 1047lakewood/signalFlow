# Crossfade — Design Doc

## Goal
Configurable fade duration between tracks. Fade-out of ending track overlaps with fade-in of next track. Linear curve initially.

## Architecture: Dual-Sink Approach

Player holds two rodio Sinks that alternate roles (active / standby):

1. Track N plays on sink A
2. When `elapsed >= track_duration - crossfade_duration`:
   - Start track N+1 on sink B with `source.fade_in(crossfade_duration)`
   - Ramp sink A volume from 1.0 → 0.0 over crossfade_duration (linear steps every 50ms)
3. When crossfade completes: stop sink A, swap roles (B becomes active)
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
- `sinks: [Sink; 2]` — dual sinks
- `active_sink: usize` — index of currently playing sink (0 or 1)
- Existing methods (stop, pause, resume, etc.) operate on the active sink

## CLI Changes
- `config crossfade <seconds>` — set default crossfade duration
- `play --crossfade <seconds>` — override for this session
