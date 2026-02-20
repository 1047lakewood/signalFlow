import {
  useCallback,
  useEffect,
  useReducer,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { AudioFileInfo, Marker, SilenceRegion } from "./editorTypes";
import { EXPORT_FORMATS } from "./editorTypes";
import { editorReducer, makeInitialState } from "./editorReducer";
import { useEditorPlayback } from "./useEditorPlayback";
import { useEditorWaveform } from "./useEditorWaveform";
import EditorToolbar from "./EditorToolbar";
import EditorTimeline from "./EditorTimeline";
import EditorWaveform from "./EditorWaveform";
import EditorTransport from "./EditorTransport";
import EditorEffectsPanel from "./EditorEffectsPanel";
import EditorSidebar from "./EditorSidebar";
import "./editorStyles.css";

interface AudioEditorModalProps {
  path: string;
  onClose: () => void;
  /** Called after successful export — provides new file path for "replace in playlist" */
  onExported?: (newPath: string, originalPath: string) => void;
}

export default function AudioEditorModal({
  path,
  onClose,
  onExported,
}: AudioEditorModalProps) {
  const [editorState, dispatch] = useReducer(
    editorReducer,
    makeInitialState(path, 0),
  );
  const { present, past, future } = editorState;

  const [fileInfo, setFileInfo] = useState<AudioFileInfo | null>(null);
  const [silenceRegions, setSilenceRegions] = useState<SilenceRegion[]>([]);
  const [zoom, setZoom] = useState(1);
  const [scrollOffsetSecs, setScrollOffsetSecs] = useState(0);
  const [loopEnabled, setLoopEnabled] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const [exportFormat, setExportFormat] = useState(EXPORT_FORMATS[0]);
  const [showExportDialog, setShowExportDialog] = useState(false);

  const containerRef = useRef<HTMLDivElement>(null);
  const waveformContainerRef = useRef<HTMLDivElement>(null);
  const [viewWidthPx, setViewWidthPx] = useState(800);

  const { peakData, isLoading, error, getVisiblePeaks, pixelToTime, secsPerPixel } =
    useEditorWaveform(path);

  const durationSecs = peakData?.duration_secs ?? fileInfo?.duration_secs ?? 0;

  const { isPlaying, positionSecs, play, pause, stop, seek, togglePlay } =
    useEditorPlayback(path, durationSecs);

  // Initialize editor state when duration is known
  useEffect(() => {
    if (durationSecs > 0 && present.ops.trim_end_secs === 0) {
      dispatch({ type: "RESET", path, duration: durationSecs });
    }
  }, [durationSecs, path, present.ops.trim_end_secs]);

  // Load file info
  useEffect(() => {
    invoke<AudioFileInfo>("get_audio_info", { path }).then(setFileInfo).catch(() => undefined);
  }, [path]);

  // Track container width with ResizeObserver
  useEffect(() => {
    const el = waveformContainerRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width;
      if (w && w > 0) setViewWidthPx(Math.floor(w));
    });
    ro.observe(el);
    setViewWidthPx(Math.floor(el.getBoundingClientRect().width));
    return () => ro.disconnect();
  }, []);

  // Zoom via Ctrl+Wheel
  const handleWheelOnWaveform = useCallback(
    (e: React.WheelEvent) => {
      if (e.ctrlKey || e.metaKey) {
        e.preventDefault();
        const delta = e.deltaY < 0 ? 1.2 : 1 / 1.2;
        setZoom((z) => Math.max(0.2, Math.min(200, z * delta)));
      } else {
        // Horizontal pan
        const spp = secsPerPixel(zoom);
        const delta = e.deltaY * spp * 2;
        setScrollOffsetSecs((s) =>
          Math.max(0, Math.min(durationSecs - viewWidthPx * spp, s + delta)),
        );
      }
    },
    [zoom, secsPerPixel, durationSecs, viewWidthPx],
  );

  // Keyboard shortcuts
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      // Don't fire inside inputs
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

      if (e.key === " ") {
        e.preventDefault();
        togglePlay(positionSecs);
      }
      if (e.key === "Home") {
        e.preventDefault();
        seek(0);
      }
      if (e.key === "End") {
        e.preventDefault();
        seek(durationSecs);
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "z") {
        e.preventDefault();
        dispatch({ type: "UNDO" });
      }
      if ((e.ctrlKey || e.metaKey) && (e.key === "y" || (e.shiftKey && e.key === "z"))) {
        e.preventDefault();
        dispatch({ type: "REDO" });
      }
      if (e.key === "t" || e.key === "T") {
        if (present.selectionStart !== null && present.selectionEnd !== null) {
          dispatch({ type: "TRIM_TO_SELECTION" });
        }
      }
      if (e.key === "Delete" || e.key === "Backspace") {
        if (present.selectionStart !== null && present.selectionEnd !== null) {
          e.preventDefault();
          dispatch({ type: "CUT_SELECTION" });
        }
      }
      if (e.key === "i" || e.key === "I") {
        dispatch({ type: "SET_TRIM_START", secs: positionSecs });
      }
      if (e.key === "o" || e.key === "O") {
        dispatch({ type: "SET_TRIM_END", secs: positionSecs });
      }
      if (e.key === "Escape") {
        onClose();
      }
    };
    const el = containerRef.current;
    if (!el) return;
    el.addEventListener("keydown", handleKey);
    return () => el.removeEventListener("keydown", handleKey);
  }, [togglePlay, positionSecs, seek, durationSecs, present.selectionStart, present.selectionEnd, onClose]);

  // Auto-focus the modal container on mount
  useEffect(() => {
    containerRef.current?.focus();
  }, []);

  // Loop: when playing and position reaches selectionEnd, restart from selectionStart
  useEffect(() => {
    if (
      loopEnabled &&
      isPlaying &&
      present.selectionStart !== null &&
      present.selectionEnd !== null
    ) {
      const end = Math.max(present.selectionStart, present.selectionEnd);
      if (positionSecs >= end) {
        seek(Math.min(present.selectionStart, present.selectionEnd));
        play(Math.min(present.selectionStart, present.selectionEnd));
      }
    }
  }, [positionSecs, loopEnabled, isPlaying, present.selectionStart, present.selectionEnd, seek, play]);

  // Keep playhead visible in scroll
  useEffect(() => {
    const spp = secsPerPixel(zoom);
    const visibleEnd = scrollOffsetSecs + viewWidthPx * spp;
    if (positionSecs < scrollOffsetSecs || positionSecs > visibleEnd) {
      setScrollOffsetSecs(Math.max(0, positionSecs - viewWidthPx * spp * 0.3));
    }
  }, [positionSecs]); // eslint-disable-line

  const visiblePeaks = getVisiblePeaks(zoom, scrollOffsetSecs, viewWidthPx);

  const handleSeek = useCallback(
    (secs: number) => {
      const clamped = Math.max(0, Math.min(secs, durationSecs));
      seek(clamped);
    },
    [seek, durationSecs],
  );

  const handleAddMarker = useCallback((secs: number) => {
    const id = `marker-${Date.now()}`;
    const marker: Marker = {
      id,
      time_secs: secs,
      label: `M`,
      color: "#f0c040",
    };
    dispatch({ type: "ADD_MARKER", marker });
  }, []);

  const handleTrimSilenceEdges = useCallback(() => {
    if (silenceRegions.length === 0) return;
    const first = silenceRegions[0];
    const last = silenceRegions[silenceRegions.length - 1];
    if (first.start_secs < 0.1) {
      dispatch({ type: "SET_TRIM_START", secs: first.end_secs });
    }
    if (last.end_secs >= durationSecs - 0.1) {
      dispatch({ type: "SET_TRIM_END", secs: last.start_secs });
    }
  }, [silenceRegions, durationSecs]);

  const handleExport = async () => {
    setShowExportDialog(false);
    setExporting(true);
    setExportError(null);

    try {
      const ext = exportFormat.ext;
      const suggested = (path.split(/[/\\]/).pop() ?? "output").replace(
        /\.[^.]+$/,
        `_edited.${ext}`,
      );
      const target = await save({
        filters: [{ name: exportFormat.label, extensions: [ext] }],
        defaultPath: suggested,
      });
      if (!target) {
        setExporting(false);
        return;
      }

      const ops = { ...present.ops };
      const outputPath = await invoke<string>("export_edited_audio", {
        request: {
          input_path: path,
          output_path: target,
          format: ext,
          quality: exportFormat.defaultQuality,
          operations: ops,
        },
      });

      onExported?.(outputPath, path);
      setExporting(false);
    } catch (e) {
      setExportError(String(e));
      setExporting(false);
    }
  };

  const hasSelection =
    present.selectionStart !== null && present.selectionEnd !== null;

  return (
    <div
      className="editor-overlay"
      ref={containerRef}
      tabIndex={-1}
    >
      <div className="editor-window">
        {/* Toolbar */}
        <EditorToolbar
          fileInfo={fileInfo}
          filename={path}
          canUndo={past.length > 0}
          canRedo={future.length > 0}
          hasSelection={hasSelection}
          onUndo={() => dispatch({ type: "UNDO" })}
          onRedo={() => dispatch({ type: "REDO" })}
          onTrimToSelection={() => dispatch({ type: "TRIM_TO_SELECTION" })}
          onCutSelection={() => dispatch({ type: "CUT_SELECTION" })}
          onExport={() => setShowExportDialog(true)}
          onClose={onClose}
        />

        {/* Body */}
        <div className="editor-body">
          {/* Left — effects panel */}
          <div className="editor-left-panel">
            <EditorEffectsPanel
              ops={present.ops}
              onChange={(partial) => {
                if ("volume_db" in partial) dispatch({ type: "SET_VOLUME", db: partial.volume_db! });
                if ("speed" in partial) dispatch({ type: "SET_SPEED", speed: partial.speed! });
                if ("pitch_semitones" in partial) dispatch({ type: "SET_PITCH", semitones: partial.pitch_semitones! });
                if ("fade_in_secs" in partial) dispatch({ type: "SET_FADE_IN", secs: partial.fade_in_secs! });
                if ("fade_out_secs" in partial) dispatch({ type: "SET_FADE_OUT", secs: partial.fade_out_secs! });
                if ("normalize" in partial) dispatch({ type: "SET_NORMALIZE", enabled: partial.normalize! });
              }}
            />
          </div>

          {/* Center — waveform + transport */}
          <div className="editor-center">
            {isLoading && (
              <div className="editor-loading">Loading waveform…</div>
            )}
            {error && (
              <div className="editor-error">Error: {error}</div>
            )}

            {/* Waveform + timeline */}
            <div
              className="editor-waveform-container"
              ref={waveformContainerRef}
              onWheel={handleWheelOnWaveform}
            >
              <EditorTimeline
                duration={durationSecs}
                zoom={zoom}
                scrollOffsetSecs={scrollOffsetSecs}
                viewWidthPx={viewWidthPx}
                trimStart={present.ops.trim_start_secs}
                trimEnd={present.ops.trim_end_secs}
              />
              <EditorWaveform
                peaks={visiblePeaks}
                duration={durationSecs}
                positionSecs={positionSecs}
                zoom={zoom}
                scrollOffsetSecs={scrollOffsetSecs}
                trimStart={present.ops.trim_start_secs}
                trimEnd={present.ops.trim_end_secs}
                selectionStart={present.selectionStart}
                selectionEnd={present.selectionEnd}
                cuts={present.ops.cuts}
                markers={present.markers}
                viewWidthPx={viewWidthPx}
                onSeek={handleSeek}
                onSelectionChange={(start, end) =>
                  dispatch({ type: "SET_SELECTION", start, end })
                }
                onAddMarker={handleAddMarker}
                pixelToTime={pixelToTime}
              />

              {/* Zoom controls */}
              <div className="editor-zoom-controls">
                <button
                  className="editor-zoom-btn"
                  onClick={() => setZoom((z) => Math.min(200, z * 1.5))}
                  title="Zoom in (Ctrl+scroll)"
                >
                  +
                </button>
                <span className="editor-zoom-label">{zoom.toFixed(1)}×</span>
                <button
                  className="editor-zoom-btn"
                  onClick={() => setZoom((z) => Math.max(0.2, z / 1.5))}
                  title="Zoom out"
                >
                  −
                </button>
                <button
                  className="editor-zoom-btn"
                  onClick={() => { setZoom(1); setScrollOffsetSecs(0); }}
                  title="Reset zoom"
                >
                  ↺
                </button>
              </div>
            </div>

            {/* Transport */}
            <EditorTransport
              isPlaying={isPlaying}
              positionSecs={positionSecs}
              durationSecs={durationSecs}
              loopEnabled={loopEnabled}
              onPlay={() => play(positionSecs)}
              onPause={pause}
              onStop={stop}
              onToggleLoop={() => setLoopEnabled((l) => !l)}
            />

            {/* Keyboard shortcuts hint */}
            <div className="editor-shortcuts-hint">
              Space: play/pause · I/O: in/out points · T: trim · Del: cut · Ctrl+Z/Y: undo/redo · Esc: close
            </div>
          </div>

          {/* Right — sidebar */}
          <div className="editor-right-panel">
            <EditorSidebar
              path={path}
              fileInfo={fileInfo}
              markers={present.markers}
              silenceRegions={silenceRegions}
              duration={durationSecs}
              onAddMarker={(m) => dispatch({ type: "ADD_MARKER", marker: m })}
              onRemoveMarker={(id) => dispatch({ type: "REMOVE_MARKER", id })}
              onRenameMarker={(id, label) => dispatch({ type: "RENAME_MARKER", id, label })}
              onSeek={handleSeek}
              onSilenceRegionsLoaded={setSilenceRegions}
              onTrimSilenceEdges={handleTrimSilenceEdges}
            />
          </div>
        </div>
      </div>

      {/* Export dialog */}
      {showExportDialog && (
        <div className="editor-dialog-backdrop" onClick={() => setShowExportDialog(false)}>
          <div className="editor-dialog" onClick={(e) => e.stopPropagation()}>
            <div className="editor-dialog-title">Export Audio</div>
            <div className="editor-dialog-body">
              <label className="editor-dialog-label">Format</label>
              <div className="editor-dialog-formats">
                {EXPORT_FORMATS.map((fmt) => (
                  <button
                    key={fmt.label}
                    className={`editor-dialog-fmt-btn${exportFormat.label === fmt.label ? " active" : ""}`}
                    onClick={() => setExportFormat(fmt)}
                  >
                    {fmt.label}
                  </button>
                ))}
              </div>
            </div>
            <div className="editor-dialog-actions">
              <button className="editor-tool-btn primary" onClick={handleExport}>
                Choose File & Export
              </button>
              <button
                className="editor-tool-btn"
                onClick={() => setShowExportDialog(false)}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Exporting overlay */}
      {exporting && (
        <div className="editor-dialog-backdrop">
          <div className="editor-dialog">
            <div className="editor-dialog-title">Exporting…</div>
            <div className="editor-dialog-body">Running ffmpeg, please wait.</div>
          </div>
        </div>
      )}

      {/* Export error */}
      {exportError && (
        <div className="editor-dialog-backdrop" onClick={() => setExportError(null)}>
          <div className="editor-dialog editor-dialog-error" onClick={(e) => e.stopPropagation()}>
            <div className="editor-dialog-title">Export Failed</div>
            <div className="editor-dialog-body">{exportError}</div>
            <div className="editor-dialog-actions">
              <button className="editor-tool-btn" onClick={() => setExportError(null)}>
                OK
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
