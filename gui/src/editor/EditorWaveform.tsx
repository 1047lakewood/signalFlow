import {
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import type { CutRegion, Marker } from "./editorTypes";

interface EditorWaveformProps {
  peaks: number[];
  duration: number;
  positionSecs: number;
  zoom: number;
  scrollOffsetSecs: number;
  trimStart: number;
  trimEnd: number;
  selectionStart: number | null;
  selectionEnd: number | null;
  cuts: CutRegion[];
  markers: Marker[];
  viewWidthPx: number;
  /** Fired when user clicks/drags to a new position */
  onSeek: (secs: number) => void;
  /** Fired when user drag-selects a region */
  onSelectionChange: (start: number | null, end: number | null) => void;
  /** Fired when user places a new marker (double-click) */
  onAddMarker: (secs: number) => void;
  /** px→seconds helper from useEditorWaveform */
  pixelToTime: (px: number, zoom: number, scrollOffsetSecs: number) => number;
}

const HEIGHT = 140;
const WAVE_COLOR = "#3a6bc4";
const WAVE_RMS_COLOR = "#2a4e99";
const PLAYHEAD_COLOR = "#e94560";
const SELECTION_COLOR = "rgba(100,160,255,0.25)";
const SELECTION_BORDER = "rgba(100,160,255,0.8)";
const TRIM_SHADE = "rgba(0,0,0,0.45)";
const CUT_SHADE = "rgba(0,0,0,0.55)";
const TRIM_MARKER_COLOR = "#4ddd88";

export default function EditorWaveform({
  peaks,
  duration,
  positionSecs,
  zoom,
  scrollOffsetSecs,
  trimStart,
  trimEnd,
  selectionStart,
  selectionEnd,
  cuts,
  markers,
  viewWidthPx,
  onSeek,
  onSelectionChange,
  onAddMarker,
  pixelToTime,
}: EditorWaveformProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const dragStartPx = useRef<number | null>(null);
  const [isDragging, setIsDragging] = useState(false);

  const timeToX = useCallback(
    (t: number) => pixelToTime(0, zoom, scrollOffsetSecs) === 0
      ? (t / duration) * viewWidthPx  // fallback
      : ((t - scrollOffsetSecs) / ((duration / viewWidthPx) / zoom)),
    [pixelToTime, zoom, scrollOffsetSecs, duration, viewWidthPx],
  );

  // Draw waveform + overlays
  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || viewWidthPx <= 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = viewWidthPx * dpr;
    canvas.height = HEIGHT * dpr;
    canvas.style.width = `${viewWidthPx}px`;
    canvas.style.height = `${HEIGHT}px`;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);

    const mid = HEIGHT / 2;
    ctx.clearRect(0, 0, viewWidthPx, HEIGHT);

    // Background
    ctx.fillStyle = "#0d1224";
    ctx.fillRect(0, 0, viewWidthPx, HEIGHT);

    // Waveform bars
    for (let px = 0; px < viewWidthPx; px++) {
      const amp = peaks[px] ?? 0;
      const barH = Math.max(1, amp * (HEIGHT / 2 - 2));
      // Top half
      ctx.fillStyle = WAVE_COLOR;
      ctx.fillRect(px, mid - barH, 1, barH);
      // Bottom half (mirror, slightly dimmer)
      ctx.fillStyle = WAVE_RMS_COLOR;
      ctx.fillRect(px, mid, 1, barH);
    }

    // Center line
    ctx.strokeStyle = "rgba(255,255,255,0.08)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, mid);
    ctx.lineTo(viewWidthPx, mid);
    ctx.stroke();

    // Trim shade (before trim_start)
    const tsX = timeToX(trimStart);
    const teX = timeToX(trimEnd);
    if (tsX > 0) {
      ctx.fillStyle = TRIM_SHADE;
      ctx.fillRect(0, 0, Math.min(tsX, viewWidthPx), HEIGHT);
    }
    if (teX < viewWidthPx) {
      ctx.fillStyle = TRIM_SHADE;
      ctx.fillRect(Math.max(0, teX), 0, viewWidthPx - teX, HEIGHT);
    }

    // Trim markers
    ctx.strokeStyle = TRIM_MARKER_COLOR;
    ctx.lineWidth = 2;
    [trimStart, trimEnd].forEach((t) => {
      const x = timeToX(t);
      if (x >= 0 && x <= viewWidthPx) {
        ctx.beginPath();
        ctx.moveTo(Math.round(x) + 0.5, 0);
        ctx.lineTo(Math.round(x) + 0.5, HEIGHT);
        ctx.stroke();
      }
    });

    // Cut regions
    cuts.forEach((cut) => {
      const x1 = Math.max(0, timeToX(cut.start_secs));
      const x2 = Math.min(viewWidthPx, timeToX(cut.end_secs));
      if (x2 > x1) {
        ctx.fillStyle = CUT_SHADE;
        ctx.fillRect(x1, 0, x2 - x1, HEIGHT);
        ctx.strokeStyle = "rgba(233,69,96,0.5)";
        ctx.lineWidth = 1;
        ctx.strokeRect(x1, 0, x2 - x1, HEIGHT);
      }
    });

    // Selection
    if (selectionStart !== null && selectionEnd !== null) {
      const sx = Math.max(0, timeToX(Math.min(selectionStart, selectionEnd)));
      const ex = Math.min(viewWidthPx, timeToX(Math.max(selectionStart, selectionEnd)));
      if (ex > sx) {
        ctx.fillStyle = SELECTION_COLOR;
        ctx.fillRect(sx, 0, ex - sx, HEIGHT);
        ctx.strokeStyle = SELECTION_BORDER;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(Math.round(sx) + 0.5, 0);
        ctx.lineTo(Math.round(sx) + 0.5, HEIGHT);
        ctx.moveTo(Math.round(ex) + 0.5, 0);
        ctx.lineTo(Math.round(ex) + 0.5, HEIGHT);
        ctx.stroke();
      }
    }

    // Markers (flags)
    markers.forEach((marker) => {
      const x = timeToX(marker.time_secs);
      if (x < -10 || x > viewWidthPx + 10) return;
      ctx.strokeStyle = marker.color;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(Math.round(x) + 0.5, 0);
      ctx.lineTo(Math.round(x) + 0.5, HEIGHT);
      ctx.stroke();
      // Flag triangle
      ctx.fillStyle = marker.color;
      ctx.beginPath();
      ctx.moveTo(x + 0.5, 0);
      ctx.lineTo(x + 10, 0);
      ctx.lineTo(x + 0.5, 10);
      ctx.fill();
      // Label
      ctx.fillStyle = "#fff";
      ctx.font = "bold 9px sans-serif";
      ctx.fillText(marker.label, x + 2, 1);
    });

    // Playhead
    const phX = timeToX(positionSecs);
    if (phX >= 0 && phX <= viewWidthPx) {
      ctx.strokeStyle = PLAYHEAD_COLOR;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(Math.round(phX) + 0.5, 0);
      ctx.lineTo(Math.round(phX) + 0.5, HEIGHT);
      ctx.stroke();
      // Playhead handle triangle
      ctx.fillStyle = PLAYHEAD_COLOR;
      ctx.beginPath();
      ctx.moveTo(phX - 6, 0);
      ctx.lineTo(phX + 6, 0);
      ctx.lineTo(phX, 10);
      ctx.fill();
    }
  }, [
    peaks, duration, positionSecs, zoom, scrollOffsetSecs,
    trimStart, trimEnd, selectionStart, selectionEnd,
    cuts, markers, viewWidthPx, timeToX,
  ]);

  // Mouse interaction — click to seek, drag to select
  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (e.button !== 0) return;
      const rect = canvasRef.current!.getBoundingClientRect();
      const px = e.clientX - rect.left;
      const t = pixelToTime(px, zoom, scrollOffsetSecs);
      dragStartPx.current = px;
      setIsDragging(false);
      onSelectionChange(t, null); // clear old selection, set start
    },
    [pixelToTime, zoom, scrollOffsetSecs, onSelectionChange],
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (dragStartPx.current === null || e.buttons === 0) return;
      const rect = canvasRef.current!.getBoundingClientRect();
      const px = e.clientX - rect.left;
      if (Math.abs(px - dragStartPx.current) > 3) {
        setIsDragging(true);
        const t1 = pixelToTime(dragStartPx.current, zoom, scrollOffsetSecs);
        const t2 = pixelToTime(px, zoom, scrollOffsetSecs);
        onSelectionChange(t1, t2);
      }
    },
    [pixelToTime, zoom, scrollOffsetSecs, onSelectionChange],
  );

  const handleMouseUp = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const rect = canvasRef.current!.getBoundingClientRect();
      const px = e.clientX - rect.left;
      if (!isDragging) {
        // Single click = seek
        const t = pixelToTime(px, zoom, scrollOffsetSecs);
        onSelectionChange(null, null);
        onSeek(t);
      }
      dragStartPx.current = null;
      setIsDragging(false);
    },
    [isDragging, pixelToTime, zoom, scrollOffsetSecs, onSeek, onSelectionChange],
  );

  const handleDoubleClick = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const rect = canvasRef.current!.getBoundingClientRect();
      const px = e.clientX - rect.left;
      const t = pixelToTime(px, zoom, scrollOffsetSecs);
      onAddMarker(t);
    },
    [pixelToTime, zoom, scrollOffsetSecs, onAddMarker],
  );

  return (
    <canvas
      ref={canvasRef}
      style={{ display: "block", cursor: isDragging ? "col-resize" : "crosshair" }}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onDoubleClick={handleDoubleClick}
      onMouseLeave={() => {
        if (dragStartPx.current !== null) {
          dragStartPx.current = null;
          setIsDragging(false);
        }
      }}
    />
  );
}
