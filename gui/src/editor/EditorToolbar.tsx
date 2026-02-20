import type { AudioFileInfo } from "./editorTypes";

interface EditorToolbarProps {
  fileInfo: AudioFileInfo | null;
  filename: string;
  canUndo: boolean;
  canRedo: boolean;
  hasSelection: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onTrimToSelection: () => void;
  onCutSelection: () => void;
  onExport: () => void;
  onClose: () => void;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDuration(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}

export default function EditorToolbar({
  fileInfo,
  filename,
  canUndo,
  canRedo,
  hasSelection,
  onUndo,
  onRedo,
  onTrimToSelection,
  onCutSelection,
  onExport,
  onClose,
}: EditorToolbarProps) {
  return (
    <div className="editor-toolbar">
      <div className="editor-toolbar-left">
        <span className="editor-filename" title={filename}>
          {filename.split(/[/\\]/).pop() ?? filename}
        </span>
        {fileInfo && (
          <span className="editor-file-meta">
            {fileInfo.format} · {fileInfo.sample_rate.toLocaleString()} Hz ·{" "}
            {fileInfo.channels}ch · {fileInfo.bitrate_kbps} kbps ·{" "}
            {formatBytes(fileInfo.file_size_bytes)} ·{" "}
            {formatDuration(fileInfo.duration_secs)}
          </span>
        )}
      </div>

      <div className="editor-toolbar-actions">
        <button
          className="editor-tool-btn"
          onClick={onUndo}
          disabled={!canUndo}
          title="Undo (Ctrl+Z)"
        >
          ↩ Undo
        </button>
        <button
          className="editor-tool-btn"
          onClick={onRedo}
          disabled={!canRedo}
          title="Redo (Ctrl+Y)"
        >
          ↪ Redo
        </button>

        <div className="editor-toolbar-divider" />

        <button
          className="editor-tool-btn"
          onClick={onTrimToSelection}
          disabled={!hasSelection}
          title="Trim to selection (T)"
        >
          ✂ Trim
        </button>
        <button
          className="editor-tool-btn"
          onClick={onCutSelection}
          disabled={!hasSelection}
          title="Cut selected region (Delete)"
        >
          ✕ Cut
        </button>

        <div className="editor-toolbar-divider" />

        <button
          className="editor-tool-btn primary"
          onClick={onExport}
          title="Export / Save As"
        >
          ↓ Export…
        </button>
        <button
          className="editor-tool-btn danger"
          onClick={onClose}
          title="Close editor"
        >
          ✕
        </button>
      </div>
    </div>
  );
}
