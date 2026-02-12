import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { TransportState } from "./types";
import LevelMeter from "./LevelMeter";
import WaveformDisplay from "./WaveformDisplay";

function formatTime(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
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
  const pollRef = useRef<number | null>(null);
  const lastReportedIndex = useRef<number | null | undefined>(undefined);
  const pollInFlight = useRef(false);

  const pollStatus = useCallback(async () => {
    if (pollInFlight.current) return; // skip if previous poll hasn't returned
    pollInFlight.current = true;
    try {
      const s = await invoke<TransportState>("transport_status");
      setState(s);
      // Report playing track index back to parent
      const newIndex = s.is_playing ? (s.track_index ?? null) : null;
      if (newIndex !== lastReportedIndex.current) {
        lastReportedIndex.current = newIndex;
        onPlayingIndexChange?.(newIndex);
      }
    } catch (e) {
      console.error("transport_status error:", e);
    } finally {
      pollInFlight.current = false;
    }
  }, [onPlayingIndexChange]);

  useEffect(() => {
    // Poll every 500ms
    pollStatus();
    pollRef.current = window.setInterval(pollStatus, 500);
    return () => {
      if (pollRef.current !== null) {
        window.clearInterval(pollRef.current);
      }
    };
  }, [pollStatus]);

  const handlePlay = async () => {
    try {
      const trackIndex = selectedTrackIndex ?? undefined;
      await invoke("transport_play", { trackIndex });
      onTrackChange?.();
      pollStatus();
    } catch (e) {
      console.error("transport_play error:", e);
    }
  };

  const handleStop = async () => {
    try {
      await invoke("transport_stop");
      onTrackChange?.();
      pollStatus();
    } catch (e) {
      console.error("transport_stop error:", e);
    }
  };

  const handlePause = async () => {
    try {
      await invoke("transport_pause");
      pollStatus();
    } catch (e) {
      console.error("transport_pause error:", e);
    }
  };

  const handleSkip = async () => {
    try {
      await invoke("transport_skip");
      onTrackChange?.();
      pollStatus();
    } catch (e) {
      console.error("transport_skip error:", e);
    }
  };

  const handleWaveformSeek = useCallback(async (positionSecs: number) => {
    try {
      await invoke("transport_seek", { positionSecs });
      pollStatus();
    } catch (e) {
      console.error("transport_seek error:", e);
    }
  }, [pollStatus]);

  const elapsed = state.elapsed_secs;
  const remaining = Math.max(0, state.duration_secs - elapsed);
  const hasTrack = state.track_artist || state.track_title;

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
        {state.is_playing && !state.is_paused ? (
          <button className="transport-btn" onClick={handlePause} title="Pause">
            {"\u23F8"}
          </button>
        ) : (
          <button className="transport-btn transport-btn-play" onClick={state.is_paused ? handlePause : handlePlay} title={state.is_paused ? "Resume" : "Play"}>
            {"\u25B6"}
          </button>
        )}
        <button className="transport-btn" onClick={handleStop} title="Stop">
          {"\u23F9"}
        </button>
        <button className="transport-btn" onClick={handleSkip} title="Skip Next">
          {"\u23ED"}
        </button>
      </div>

      {/* Seek / progress with waveform */}
      <div className="transport-seek">
        <span className="transport-time">{formatTime(elapsed)}</span>
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
