import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Marker, SilenceRegion, AudioFileInfo } from "./editorTypes";

interface EditorSidebarProps {
  path: string;
  fileInfo: AudioFileInfo | null;
  markers: Marker[];
  silenceRegions: SilenceRegion[];
  duration: number;
  onAddMarker: (marker: Marker) => void;
  onRemoveMarker: (id: string) => void;
  onRenameMarker: (id: string, label: string) => void;
  onSeek: (secs: number) => void;
  onSilenceRegionsLoaded: (regions: SilenceRegion[]) => void;
  onTrimSilenceEdges: () => void;
}

const MARKER_COLORS = [
  "#f0c040", "#4ddd88", "#60aaff", "#e94560", "#c080ff", "#ff8c40",
];

function formatTime(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  const ms = Math.floor((secs % 1) * 100);
  return `${m}:${String(s).padStart(2, "0")}.${String(ms).padStart(2, "0")}`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export default function EditorSidebar({
  path,
  fileInfo,
  markers,
  silenceRegions,
  duration,
  onAddMarker,
  onRemoveMarker,
  onRenameMarker,
  onSeek,
  onSilenceRegionsLoaded,
  onTrimSilenceEdges,
}: EditorSidebarProps) {
  const [editingMarkerId, setEditingMarkerId] = useState<string | null>(null);
  const [editingLabel, setEditingLabel] = useState("");
  const [scanningState, setScanningState] = useState<"idle" | "scanning" | "done">("idle");
  const [colorIndex, setColorIndex] = useState(0);

  const handleAddMarker = () => {
    const id = `marker-${Date.now()}`;
    const color = MARKER_COLORS[colorIndex % MARKER_COLORS.length];
    setColorIndex((c) => c + 1);
    onAddMarker({ id, time_secs: 0, label: `M${markers.length + 1}`, color });
  };

  const handleScanSilence = async () => {
    setScanningState("scanning");
    try {
      const regions = await invoke<SilenceRegion[]>("detect_silence_regions", {
        path,
        thresholdDb: -40.0,
        minDurationSecs: 0.5,
      });
      // Clamp any trailing silence to file duration
      const clamped = regions.map((r) => ({
        ...r,
        end_secs: Math.min(r.end_secs, duration),
      }));
      onSilenceRegionsLoaded(clamped);
      setScanningState("done");
    } catch (e) {
      console.error("Silence scan failed:", e);
      setScanningState("idle");
    }
  };

  return (
    <div className="editor-sidebar">
      {/* File Properties */}
      {fileInfo && (
        <section className="editor-sidebar-section">
          <div className="editor-sidebar-title">File Info</div>
          <div className="editor-sidebar-prop">
            <span>Format</span><span>{fileInfo.format}</span>
          </div>
          <div className="editor-sidebar-prop">
            <span>Sample Rate</span>
            <span>{fileInfo.sample_rate.toLocaleString()} Hz</span>
          </div>
          <div className="editor-sidebar-prop">
            <span>Channels</span><span>{fileInfo.channels}</span>
          </div>
          <div className="editor-sidebar-prop">
            <span>Bitrate</span><span>{fileInfo.bitrate_kbps} kbps</span>
          </div>
          <div className="editor-sidebar-prop">
            <span>Size</span><span>{formatBytes(fileInfo.file_size_bytes)}</span>
          </div>
          <div className="editor-sidebar-prop">
            <span>Duration</span><span>{formatTime(fileInfo.duration_secs)}</span>
          </div>
        </section>
      )}

      {/* Markers */}
      <section className="editor-sidebar-section">
        <div className="editor-sidebar-title">
          Markers
          <button className="editor-sidebar-add-btn" onClick={handleAddMarker} title="Add marker at 0:00">
            +
          </button>
        </div>
        {markers.length === 0 ? (
          <div className="editor-sidebar-empty">
            Double-click on waveform to add markers
          </div>
        ) : (
          <div className="editor-sidebar-list">
            {markers
              .slice()
              .sort((a, b) => a.time_secs - b.time_secs)
              .map((m) => (
                <div
                  key={m.id}
                  className="editor-sidebar-marker-row"
                  style={{ borderLeftColor: m.color }}
                >
                  {editingMarkerId === m.id ? (
                    <input
                      className="editor-sidebar-marker-input"
                      value={editingLabel}
                      autoFocus
                      onChange={(e) => setEditingLabel(e.target.value)}
                      onBlur={() => {
                        if (editingLabel.trim()) {
                          onRenameMarker(m.id, editingLabel.trim());
                        }
                        setEditingMarkerId(null);
                      }}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          if (editingLabel.trim()) onRenameMarker(m.id, editingLabel.trim());
                          setEditingMarkerId(null);
                        }
                        if (e.key === "Escape") setEditingMarkerId(null);
                      }}
                    />
                  ) : (
                    <button
                      className="editor-sidebar-marker-btn"
                      onClick={() => onSeek(m.time_secs)}
                      onDoubleClick={() => {
                        setEditingMarkerId(m.id);
                        setEditingLabel(m.label);
                      }}
                    >
                      <span className="editor-sidebar-marker-time">
                        {formatTime(m.time_secs)}
                      </span>
                      <span className="editor-sidebar-marker-label">{m.label}</span>
                    </button>
                  )}
                  <button
                    className="editor-sidebar-remove-btn"
                    onClick={() => onRemoveMarker(m.id)}
                    title="Remove marker"
                  >
                    ×
                  </button>
                </div>
              ))}
          </div>
        )}
      </section>

      {/* Silence Regions */}
      <section className="editor-sidebar-section">
        <div className="editor-sidebar-title">Silence Regions</div>
        <div className="editor-sidebar-silence-actions">
          <button
            className="editor-tool-btn"
            onClick={handleScanSilence}
            disabled={scanningState === "scanning"}
          >
            {scanningState === "scanning" ? "Scanning…" : "Scan for Silence"}
          </button>
          {silenceRegions.length > 0 && (
            <button
              className="editor-tool-btn"
              onClick={onTrimSilenceEdges}
              title="Trim silence from start and end"
            >
              Trim Edges
            </button>
          )}
        </div>
        {silenceRegions.length === 0 ? (
          <div className="editor-sidebar-empty">
            {scanningState === "done" ? "No silence regions found" : "Not scanned yet"}
          </div>
        ) : (
          <div className="editor-sidebar-list">
            {silenceRegions.map((r, i) => (
              <div
                key={i}
                className="editor-sidebar-silence-row"
                onClick={() => onSeek(r.start_secs)}
              >
                <span>
                  {formatTime(r.start_secs)} – {formatTime(r.end_secs)}
                </span>
                <span className="editor-sidebar-silence-dur">
                  ({(r.end_secs - r.start_secs).toFixed(2)}s)
                </span>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
