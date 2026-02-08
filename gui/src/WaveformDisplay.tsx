import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface WaveformDisplayProps {
  trackPath: string | null;
  elapsed: number;
  duration: number;
  isPlaying: boolean;
  onSeek: (positionSecs: number) => void;
}

function WaveformDisplay({ trackPath, elapsed, duration, isPlaying, onSeek }: WaveformDisplayProps) {
  const [peaks, setPeaks] = useState<number[]>([]);
  const [loading, setLoading] = useState(false);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const lastPathRef = useRef<string | null>(null);

  // Fetch waveform data when track changes
  useEffect(() => {
    if (!trackPath || trackPath === lastPathRef.current) return;
    lastPathRef.current = trackPath;
    setLoading(true);

    invoke<number[]>("get_waveform", { path: trackPath })
      .then((data) => {
        setPeaks(data);
        setLoading(false);
      })
      .catch((e) => {
        console.error("get_waveform error:", e);
        setPeaks([]);
        setLoading(false);
      });
  }, [trackPath]);

  // Reset when no track
  useEffect(() => {
    if (!trackPath) {
      setPeaks([]);
      lastPathRef.current = null;
    }
  }, [trackPath]);

  // Draw waveform
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const container = containerRef.current;
    if (!container) return;

    const rect = container.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const width = rect.width;
    const height = rect.height;

    canvas.width = width * dpr;
    canvas.height = height * dpr;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    ctx.scale(dpr, dpr);

    // Clear
    ctx.clearRect(0, 0, width, height);

    if (peaks.length === 0) return;

    const progress = duration > 0 ? Math.min(1, elapsed / duration) : 0;
    const playheadX = progress * width;
    const barWidth = Math.max(1, width / peaks.length - 0.5);
    const centerY = height / 2;
    const maxBarHeight = height / 2 - 1;

    // Draw bars
    for (let i = 0; i < peaks.length; i++) {
      const x = (i / peaks.length) * width;
      const barH = Math.max(1, peaks[i] * maxBarHeight);
      const isPast = x < playheadX;

      ctx.fillStyle = isPast ? "#e94560" : "#2a2a4a";
      ctx.fillRect(x, centerY - barH, barWidth, barH);
      ctx.fillRect(x, centerY, barWidth, barH);
    }

    // Playhead line
    if (duration > 0 && (isPlaying || elapsed > 0)) {
      ctx.strokeStyle = "#ffffff";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(playheadX, 0);
      ctx.lineTo(playheadX, height);
      ctx.stroke();
    }
  }, [peaks, elapsed, duration, isPlaying]);

  // Handle click-to-seek
  const handleClick = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (duration <= 0) return;
    const rect = containerRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = e.clientX - rect.left;
    const ratio = Math.max(0, Math.min(1, x / rect.width));
    onSeek(ratio * duration);
  }, [duration, onSeek]);

  return (
    <div
      ref={containerRef}
      className="waveform-display"
      onClick={handleClick}
      title={loading ? "Loading waveform..." : undefined}
    >
      <canvas ref={canvasRef} className="waveform-canvas" />
    </div>
  );
}

export default WaveformDisplay;
