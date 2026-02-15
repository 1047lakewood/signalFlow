import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { TransportState } from "./types";
import LevelMeter from "./LevelMeter";
import WaveformDisplay from "./WaveformDisplay";

function formatTime(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function formatPlaytime(date: Date): string {
  const parts = new Intl.DateTimeFormat("en-US", {
    weekday: "short",
    hour: "numeric",
    minute: "2-digit",
    second: "2-digit",
    hour12: true,
  }).formatToParts(date);

  const weekday = parts.find((p) => p.type === "weekday")?.value ?? "";
  const hour = parts.find((p) => p.type === "hour")?.value ?? "0";
  const minute = parts.find((p) => p.type === "minute")?.value ?? "00";
  const second = parts.find((p) => p.type === "second")?.value ?? "00";
  const dayPeriod = parts.find((p) => p.type === "dayPeriod")?.value ?? "";

  return `${weekday} ${hour}:${minute}:${second} ${dayPeriod}`.trim();
}

interface TransportBarProps {
  onTrackChange?: () => void;
  selectedTrackIndex?: number | null;
  onPlayingIndexChange?: (index: number | null) => void;
}

function TransportBar({ onTrackChange, selectedTrackIndex, onPlayingIndexChange }: TransportBarProps) {
  const [state, setState] = useState<TransportState>({
    is_playing: false,
    is_paused: false,
    elapsed_secs: 0,
    duration_secs: 0,
    track_index: null,
    track_artist: null,
    track_title: null,
    next_artist: null,
    next_title: null,
    track_path: null,
  });
  const lastReportedIndex = useRef<number | null | undefined>(undefined);

  // Elapsed time interpolation: store base values from last status fetch
  const baseElapsed = useRef(0);
  const baseTimestamp = useRef(0);
  const baseWallClock = useRef(Date.now());
  const [displayElapsed, setDisplayElapsed] = useState(0);

  const fetchStatus = useCallback(async () => {
    try {
      const s = await invoke<TransportState>("transport_status");
      setState(s);
      // Update interpolation base
      baseElapsed.current = s.elapsed_secs;
      baseTimestamp.current = performance.now();
      baseWallClock.current = Date.now() - s.elapsed_secs * 1000;
      // Report playing track index back to parent
      const newIndex = s.is_playing ? (s.track_index ?? null) : null;
      if (newIndex !== lastReportedIndex.current) {
        lastReportedIndex.current = newIndex;
        onPlayingIndexChange?.(newIndex);
      }
    } catch (e) {
      console.error("transport_status error:", e);
    }
  }, [onPlayingIndexChange]);

  // Listen for transport-changed events instead of polling
  useEffect(() => {
    // Fetch initial state on mount
    fetchStatus();

    const unlisten = listen("transport-changed", () => {
      fetchStatus();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [fetchStatus]);

  // Smooth elapsed time interpolation via requestAnimationFrame
  useEffect(() => {
    let rafId: number;

    const tick = () => {
      if (state.is_playing && !state.is_paused) {
        const now = performance.now();
        const delta = (now - baseTimestamp.current) / 1000;
        const interpolated = Math.min(
          baseElapsed.current + delta,
          state.duration_secs
        );
        setDisplayElapsed(interpolated);
      } else {
        setDisplayElapsed(baseElapsed.current);
      }
      rafId = requestAnimationFrame(tick);
    };

    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, [state.is_playing, state.is_paused, state.duration_secs]);

  const handlePlay = async () => {
    try {
      const trackIndex = selectedTrackIndex ?? undefined;
      await invoke("transport_play", { trackIndex });
      onTrackChange?.();
    } catch (e) {
      console.error("transport_play error:", e);
    }
  };

  const handleStop = async () => {
    try {
      await invoke("transport_stop");
      onTrackChange?.();
    } catch (e) {
      console.error("transport_stop error:", e);
    }
  };

  const handlePause = async () => {
    try {
      await invoke("transport_pause");
    } catch (e) {
      console.error("transport_pause error:", e);
    }
  };

  const handleSkip = async () => {
    try {
      await invoke("transport_skip");
      onTrackChange?.();
    } catch (e) {
      console.error("transport_skip error:", e);
    }
  };

  const handleWaveformSeek = useCallback(async (positionSecs: number) => {
    try {
      await invoke("transport_seek", { positionSecs });
    } catch (e) {
      console.error("transport_seek error:", e);
    }
  }, []);

  const elapsed = displayElapsed;
  const remaining = Math.max(0, state.duration_secs - elapsed);
  const hasTrack = state.track_artist || state.track_title;
  const displayPlaytime = formatPlaytime(new Date(baseWallClock.current + elapsed * 1000));

  return (
    <div className="transport-bar">
      {/* Now-playing info panel */}
      <div className="now-playing-panel">
        {hasTrack ? (
          <>
            <span className="now-playing-title">{state.track_title ?? "Unknown"}</span>
            <span className="now-playing-artist">{state.track_artist ?? "Unknown"}</span>
          </>
        ) : (
          <span className="now-playing-empty">No track loaded</span>
        )}
      </div>

      {/* Controls */}
      <div className="transport-controls">
        <button className="transport-btn transport-btn-play" onClick={handlePlay} title="Play">
          {"\u25B6"}
        </button>
        <button
          className="transport-btn"
          onClick={handlePause}
          title="Pause"
          disabled={!state.is_playing || state.is_paused}
        >
          {"\u23F8"}
        </button>
        <button className="transport-btn" onClick={handleStop} title="Stop">
          {"\u23F9"}
        </button>
        <button className="transport-btn" onClick={handleSkip} title="Skip Next">
          {"\u23ED"}
        </button>
      </div>

      {/* Seek / progress with waveform */}
      <div className="transport-seek">
        <span className="transport-time" title="Track playtime">{displayPlaytime}</span>
        <WaveformDisplay
          trackPath={state.track_path}
          elapsed={elapsed}
          duration={state.duration_secs}
          isPlaying={state.is_playing && !state.is_paused}
          onSeek={handleWaveformSeek}
        />
        <span className="transport-time">-{formatTime(remaining)}</span>
      </div>

      {/* Level meter */}
      <LevelMeter isPlaying={state.is_playing && !state.is_paused} />

      {/* Next up */}
      <div className="now-playing-next">
        {state.next_artist || state.next_title ? (
          <>
            <span className="next-label">Next</span>
            <span className="next-track">{state.next_artist ?? "Unknown"} — {state.next_title ?? "Unknown"}</span>
          </>
        ) : (
          <span className="next-label next-empty">—</span>
        )}
      </div>
    </div>
  );
}

export default TransportBar;
