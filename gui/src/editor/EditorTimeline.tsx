import { useEffect, useRef } from "react";

interface EditorTimelineProps {
  duration: number;
  zoom: number;
  scrollOffsetSecs: number;
  viewWidthPx: number;
  trimStart: number;
  trimEnd: number;
}

function formatTime(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  const ms = Math.floor((secs % 1) * 10);
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}.${ms}`;
}

const HEIGHT = 24;
const TICK_COLOR_MAJOR = "#a0a8c0";
const TICK_COLOR_MINOR = "#505870";
const TEXT_COLOR = "#c0c8e0";
const TRIM_SHADE = "rgba(233,69,96,0.12)";

export default function EditorTimeline({
  duration,
  zoom,
  scrollOffsetSecs,
  viewWidthPx,
  trimStart,
  trimEnd,
}: EditorTimelineProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || viewWidthPx <= 0 || duration <= 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = viewWidthPx * dpr;
    canvas.height = HEIGHT * dpr;
    canvas.style.width = `${viewWidthPx}px`;
    canvas.style.height = `${HEIGHT}px`;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, viewWidthPx, HEIGHT);

    // Seconds visible on screen
    const secsPerPx = (duration / viewWidthPx) / zoom;
    const visibleSecs = secsPerPx * viewWidthPx;

    // Choose tick interval based on zoom
    const minLabelGapPx = 60;
    const candidates = [
      0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 1800, 3600,
    ];
    let majorInterval = 60;
    for (const c of candidates) {
      if ((c / secsPerPx) >= minLabelGapPx) {
        majorInterval = c;
        break;
      }
    }
    const minorInterval = majorInterval / 5;

    const timeToX = (t: number) => (t - scrollOffsetSecs) / secsPerPx;

    // Shade trim-excluded regions
    ctx.fillStyle = TRIM_SHADE;
    const trimStartX = timeToX(trimStart);
    const trimEndX = timeToX(trimEnd);
    ctx.fillRect(0, 0, Math.max(0, trimStartX), HEIGHT);
    ctx.fillRect(Math.min(viewWidthPx, trimEndX), 0, viewWidthPx, HEIGHT);

    // Minor ticks
    ctx.strokeStyle = TICK_COLOR_MINOR;
    ctx.lineWidth = 1;
    const firstMinor = Math.floor(scrollOffsetSecs / minorInterval) * minorInterval;
    for (
      let t = firstMinor;
      t <= scrollOffsetSecs + visibleSecs;
      t += minorInterval
    ) {
      const x = timeToX(t);
      if (x < 0 || x > viewWidthPx) continue;
      ctx.beginPath();
      ctx.moveTo(Math.round(x) + 0.5, HEIGHT);
      ctx.lineTo(Math.round(x) + 0.5, HEIGHT - 5);
      ctx.stroke();
    }

    // Major ticks + labels
    ctx.strokeStyle = TICK_COLOR_MAJOR;
    ctx.fillStyle = TEXT_COLOR;
    ctx.font = "10px 'Segoe UI', sans-serif";
    ctx.textBaseline = "top";
    const firstMajor = Math.floor(scrollOffsetSecs / majorInterval) * majorInterval;
    for (
      let t = firstMajor;
      t <= scrollOffsetSecs + visibleSecs;
      t += majorInterval
    ) {
      const x = timeToX(t);
      if (x < 0 || x > viewWidthPx) continue;
      ctx.beginPath();
      ctx.moveTo(Math.round(x) + 0.5, HEIGHT);
      ctx.lineTo(Math.round(x) + 0.5, HEIGHT - 10);
      ctx.stroke();
      const label = formatTime(t);
      const tw = ctx.measureText(label).width;
      const lx = Math.min(viewWidthPx - tw - 2, Math.max(2, x - tw / 2));
      ctx.fillText(label, lx, 2);
    }
  }, [duration, zoom, scrollOffsetSecs, viewWidthPx, trimStart, trimEnd]);

  return (
    <canvas
      ref={canvasRef}
      style={{ display: "block", cursor: "default" }}
    />
  );
}
