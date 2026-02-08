import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

interface LevelMeterProps {
  isPlaying: boolean;
}

function LevelMeter({ isPlaying }: LevelMeterProps) {
  const [level, setLevel] = useState(0);
  const pollRef = useRef<number | null>(null);
  const peakRef = useRef(0);
  const peakDecayRef = useRef(0);
  const [peak, setPeak] = useState(0);

  useEffect(() => {
    if (!isPlaying) {
      setLevel(0);
      setPeak(0);
      peakRef.current = 0;
      peakDecayRef.current = 0;
      if (pollRef.current !== null) {
        window.clearInterval(pollRef.current);
        pollRef.current = null;
      }
      return;
    }

    const poll = async () => {
      try {
        const rms = await invoke<number>("get_audio_level");
        // Convert RMS to a 0–100 scale with dB-like response.
        // RMS of ~0.7 is very loud; map to percentage with some headroom.
        const db = rms > 0.0001 ? 20 * Math.log10(rms) : -60;
        // Map -60dB..0dB to 0..100
        const pct = Math.max(0, Math.min(100, ((db + 60) / 60) * 100));
        setLevel(pct);

        // Peak hold with decay
        if (pct > peakRef.current) {
          peakRef.current = pct;
          peakDecayRef.current = 0;
        } else {
          peakDecayRef.current += 1;
          if (peakDecayRef.current > 15) {
            // ~1 second hold, then decay
            peakRef.current = Math.max(0, peakRef.current - 1.5);
          }
        }
        setPeak(peakRef.current);
      } catch {
        // ignore errors
      }
    };

    poll();
    pollRef.current = window.setInterval(poll, 60);
    return () => {
      if (pollRef.current !== null) {
        window.clearInterval(pollRef.current);
        pollRef.current = null;
      }
    };
  }, [isPlaying]);

  return (
    <div className="level-meter" title={`Level: ${level.toFixed(0)}%`}>
      <div className="level-meter-track">
        <div
          className="level-meter-fill"
          style={{ width: `${level}%` }}
        />
        {peak > 0 && (
          <div
            className="level-meter-peak"
            style={{ left: `${Math.min(peak, 100)}%` }}
          />
        )}
      </div>
    </div>
  );
}

export default LevelMeter;
