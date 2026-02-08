import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { TransportState } from "./types";

function formatTime(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

interface TransportBarProps {
  onTrackChange?: () => void;
}

function TransportBar({ onTrackChange }: TransportBarProps) {
  const [state, setState] = useState<TransportState>({
    is_playing: false,
    is_paused: false,
    elapsed_secs: 0,
    duration_secs: 0,
    track_index: null,
    track_artist: null,
    track_title: null,
  });
  const [isSeeking, setIsSeeking] = useState(false);
  const [seekValue, setSeekValue] = useState(0);
  const pollRef = useRef<number | null>(null);

  const pollStatus = useCallback(async () => {
    try {
      const s = await invoke<TransportState>("transport_status");
      setState(s);
    } catch (e) {
      console.error("transport_status error:", e);
    }
  }, []);

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
      await invoke("transport_play", {});
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

  const handleSeekStart = () => {
    setIsSeeking(true);
    setSeekValue(state.elapsed_secs);
  };

  const handleSeekChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setSeekValue(parseFloat(e.target.value));
  };

  const handleSeekEnd = async () => {
    setIsSeeking(false);
    try {
      await invoke("transport_seek", { positionSecs: seekValue });
      pollStatus();
    } catch (e) {
      console.error("transport_seek error:", e);
    }
  };

  const elapsed = isSeeking ? seekValue : state.elapsed_secs;
  const remaining = Math.max(0, state.duration_secs - elapsed);
  const progress = state.duration_secs > 0 ? (elapsed / state.duration_secs) * 100 : 0;

  return (
    <div className="transport-bar">
      <div className="transport-track-info">
        {state.track_artist && state.track_title ? (
          <span className="transport-track-name">
            {state.track_artist} — {state.track_title}
          </span>
        ) : (
          <span className="transport-track-name transport-no-track">
            No track loaded
          </span>
        )}
      </div>
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
      <div className="transport-seek">
        <span className="transport-time">{formatTime(elapsed)}</span>
        <input
          type="range"
          className="transport-slider"
          min={0}
          max={state.duration_secs || 1}
          step={0.1}
          value={isSeeking ? seekValue : state.elapsed_secs}
          onChange={handleSeekChange}
          onMouseDown={handleSeekStart}
          onMouseUp={handleSeekEnd}
          onTouchStart={handleSeekStart}
          onTouchEnd={handleSeekEnd}
          style={{ "--progress": `${progress}%` } as React.CSSProperties}
        />
        <span className="transport-time">-{formatTime(remaining)}</span>
      </div>
    </div>
  );
}

export default TransportBar;
