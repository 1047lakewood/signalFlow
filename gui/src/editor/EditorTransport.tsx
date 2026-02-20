interface EditorTransportProps {
  isPlaying: boolean;
  positionSecs: number;
  durationSecs: number;
  loopEnabled: boolean;
  onPlay: () => void;
  onPause: () => void;
  onStop: () => void;
  onToggleLoop: () => void;
}

function formatTime(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  const ms = Math.floor((secs % 1) * 100);
  return `${m}:${String(s).padStart(2, "0")}.${String(ms).padStart(2, "0")}`;
}

export default function EditorTransport({
  isPlaying,
  positionSecs,
  durationSecs,
  loopEnabled,
  onPlay,
  onPause,
  onStop,
  onToggleLoop,
}: EditorTransportProps) {
  return (
    <div className="editor-transport">
      <button
        className="editor-transport-btn"
        onClick={onStop}
        title="Stop (go to start)"
      >
        ■
      </button>
      <button
        className={`editor-transport-btn primary${isPlaying ? " active" : ""}`}
        onClick={isPlaying ? onPause : onPlay}
        title={isPlaying ? "Pause" : "Play (Space)"}
      >
        {isPlaying ? "⏸" : "▶"}
      </button>
      <button
        className={`editor-transport-btn${loopEnabled ? " active" : ""}`}
        onClick={onToggleLoop}
        title="Loop selection"
      >
        ↻
      </button>
      <span className="editor-time-display">
        <span className="editor-time-current">{formatTime(positionSecs)}</span>
        <span className="editor-time-sep"> / </span>
        <span className="editor-time-total">{formatTime(durationSecs)}</span>
      </span>
      <div className="editor-transport-seek">
        <input
          type="range"
          className="editor-seek-slider"
          min={0}
          max={durationSecs}
          step={0.01}
          value={positionSecs}
          style={{ "--progress": `${(positionSecs / Math.max(durationSecs, 0.001)) * 100}%` } as React.CSSProperties}
          readOnly
        />
      </div>
    </div>
  );
}
