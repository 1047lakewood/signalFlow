import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { EditorPeakData } from "./editorTypes";

const EDITOR_RESOLUTION_MS = 10; // 10ms per peak ≈ 100 peaks/sec

interface UseEditorWaveformReturn {
  peakData: EditorPeakData | null;
  isLoading: boolean;
  error: string | null;
  /** Return the slice of peaks visible at the current zoom/scroll level. */
  getVisiblePeaks: (
    zoom: number,
    scrollOffsetSecs: number,
    viewWidthPx: number,
  ) => number[];
  /** Seconds per pixel at the given zoom level. */
  secsPerPixel: (zoom: number) => number;
  /** Pixel position for a given time. */
  timeToPixel: (
    timeSecs: number,
    zoom: number,
    scrollOffsetSecs: number,
  ) => number;
  /** Time for a given pixel position. */
  pixelToTime: (
    px: number,
    zoom: number,
    scrollOffsetSecs: number,
  ) => number;
}

export function useEditorWaveform(path: string | null): UseEditorWaveformReturn {
  const [peakData, setPeakData] = useState<EditorPeakData | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!path) {
      setPeakData(null);
      return;
    }
    let cancelled = false;
    setIsLoading(true);
    setError(null);

    invoke<EditorPeakData>("get_editor_waveform", {
      path,
      resolutionMs: EDITOR_RESOLUTION_MS,
    })
      .then((data) => {
        if (!cancelled) {
          setPeakData(data);
          setIsLoading(false);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(String(e));
          setIsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [path]);

  /** Number of seconds represented by one pixel at the given zoom level.
   *  zoom=1 → natural (1 peak = 1px), zoom=2 → 2x wider (fewer peaks visible). */
  const secsPerPixel = useCallback(
    (zoom: number): number => {
      if (!peakData || peakData.duration_secs <= 0) return 0.001;
      // At zoom=1, 1px = resolution_ms worth of audio
      return (peakData.resolution_ms / 1000) / Math.max(zoom, 0.001);
    },
    [peakData],
  );

  const timeToPixel = useCallback(
    (timeSecs: number, zoom: number, scrollOffsetSecs: number): number => {
      const spp = secsPerPixel(zoom);
      return (timeSecs - scrollOffsetSecs) / spp;
    },
    [secsPerPixel],
  );

  const pixelToTime = useCallback(
    (px: number, zoom: number, scrollOffsetSecs: number): number => {
      const spp = secsPerPixel(zoom);
      return scrollOffsetSecs + px * spp;
    },
    [secsPerPixel],
  );

  /** Downsample the peaks array to exactly `viewWidthPx` values for the
   *  current zoom and scroll position (client-side; no Rust round-trip). */
  const getVisiblePeaks = useCallback(
    (
      zoom: number,
      scrollOffsetSecs: number,
      viewWidthPx: number,
    ): number[] => {
      if (!peakData || peakData.peaks.length === 0 || viewWidthPx <= 0) {
        return new Array(Math.max(1, viewWidthPx)).fill(0);
      }

      const spp = secsPerPixel(zoom);
      const totalSecs = peakData.duration_secs;
      const peaksPerSec = peakData.num_peaks / totalSecs;

      const out = new Array(viewWidthPx).fill(0);

      for (let px = 0; px < viewWidthPx; px++) {
        const timeSecs = scrollOffsetSecs + px * spp;
        if (timeSecs < 0 || timeSecs >= totalSecs) continue;

        // Range of source peaks this pixel covers
        const peakStart = (timeSecs * peaksPerSec) | 0;
        const peakEnd = Math.min(
          peakData.num_peaks - 1,
          ((timeSecs + spp) * peaksPerSec) | 0,
        );

        let max = 0;
        for (let i = peakStart; i <= peakEnd; i++) {
          if (peakData.peaks[i] > max) max = peakData.peaks[i];
        }
        out[px] = max;
      }

      return out;
    },
    [peakData, secsPerPixel],
  );

  return {
    peakData,
    isLoading,
    error,
    getVisiblePeaks,
    secsPerPixel,
    timeToPixel,
    pixelToTime,
  };
}
