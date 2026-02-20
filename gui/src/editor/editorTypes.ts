// ── Waveform & file info ─────────────────────────────────────────────────────

export interface EditorPeakData {
  peaks: number[];
  duration_secs: number;
  sample_rate: number;
  num_peaks: number;
  resolution_ms: number;
}

export interface AudioFileInfo {
  format: string;
  duration_secs: number;
  sample_rate: number;
  channels: number;
  bitrate_kbps: number;
  file_size_bytes: number;
}

export interface EditorPlaybackStatus {
  is_playing: boolean;
  position_secs: number;
}

// ── Edit operations ──────────────────────────────────────────────────────────

export interface CutRegion {
  start_secs: number;
  end_secs: number;
}

export interface SilenceRegion {
  start_secs: number;
  end_secs: number;
}

export interface Marker {
  id: string;
  time_secs: number;
  label: string;
  color: string;
}

export interface EditorOperations {
  trim_start_secs: number;
  trim_end_secs: number;
  volume_db: number;
  speed: number;
  pitch_semitones: number;
  fade_in_secs: number;
  fade_out_secs: number;
  normalize: boolean;
  cuts: CutRegion[];
  total_duration_secs: number;
}

// ── Editor state (managed by editorReducer) ──────────────────────────────────

export interface EditorState {
  path: string;
  ops: EditorOperations;
  selectionStart: number | null;
  selectionEnd: number | null;
  markers: Marker[];
}

export const defaultOps = (duration: number): EditorOperations => ({
  trim_start_secs: 0,
  trim_end_secs: duration,
  volume_db: 0,
  speed: 1.0,
  pitch_semitones: 0,
  fade_in_secs: 0,
  fade_out_secs: 0,
  normalize: false,
  cuts: [],
  total_duration_secs: duration,
});

// ── Undo/redo shell ──────────────────────────────────────────────────────────

export interface UndoRedoState {
  past: EditorState[];
  present: EditorState;
  future: EditorState[];
}

// ── Reducer actions ──────────────────────────────────────────────────────────

export type EditorAction =
  | { type: "SET_TRIM_START"; secs: number }
  | { type: "SET_TRIM_END"; secs: number }
  | { type: "SET_VOLUME"; db: number }
  | { type: "SET_SPEED"; speed: number }
  | { type: "SET_PITCH"; semitones: number }
  | { type: "SET_FADE_IN"; secs: number }
  | { type: "SET_FADE_OUT"; secs: number }
  | { type: "SET_NORMALIZE"; enabled: boolean }
  | { type: "ADD_CUT"; region: CutRegion }
  | { type: "REMOVE_CUT"; index: number }
  | { type: "SET_SELECTION"; start: number | null; end: number | null }
  | { type: "TRIM_TO_SELECTION" }
  | { type: "CUT_SELECTION" }
  | { type: "ADD_MARKER"; marker: Marker }
  | { type: "REMOVE_MARKER"; id: string }
  | { type: "RENAME_MARKER"; id: string; label: string }
  | { type: "UNDO" }
  | { type: "REDO" }
  | { type: "RESET"; path: string; duration: number };

// ── Export ───────────────────────────────────────────────────────────────────

export interface ExportRequest {
  input_path: string;
  output_path: string;
  format: string;
  quality: number;
  operations: EditorOperations;
}

export interface ExportFormat {
  label: string;
  ext: string;
  defaultQuality: number;
}

export const EXPORT_FORMATS: ExportFormat[] = [
  { label: "MP3 (High quality)", ext: "mp3", defaultQuality: 2 },
  { label: "MP3 (Standard)", ext: "mp3", defaultQuality: 4 },
  { label: "WAV (Lossless)", ext: "wav", defaultQuality: 0 },
];
