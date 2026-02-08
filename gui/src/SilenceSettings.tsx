import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ConfigResponse } from "./types";

interface SilenceSettingsProps {
  onClose: () => void;
}

function SilenceSettings({ onClose }: SilenceSettingsProps) {
  const [threshold, setThreshold] = useState<string>("0.01");
  const [durationSecs, setDurationSecs] = useState<string>("0");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const config = await invoke<ConfigResponse>("get_config");
        setThreshold(String(config.silence_threshold));
        setDurationSecs(String(config.silence_duration_secs));
      } catch (e) {
        console.error("Failed to load config:", e);
      }
    })();
  }, []);

  const isEnabled = parseFloat(durationSecs) > 0 && parseFloat(threshold) > 0;

  const handleSave = async () => {
    const t = parseFloat(threshold);
    const d = parseFloat(durationSecs);
    if (isNaN(t) || isNaN(d) || t < 0 || d < 0) return;
    setSaving(true);
    try {
      await invoke("set_silence_detection", { threshold: t, durationSecs: d });
      setSaved(true);
      setTimeout(() => setSaved(false), 1500);
    } catch (e) {
      console.error("Failed to set silence detection:", e);
    } finally {
      setSaving(false);
    }
  };

  const handleDisable = async () => {
    setSaving(true);
    try {
      await invoke("set_silence_detection", { threshold: 0, durationSecs: 0 });
      setThreshold("0");
      setDurationSecs("0");
      setSaved(true);
      setTimeout(() => setSaved(false), 1500);
    } catch (e) {
      console.error("Failed to disable silence detection:", e);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="settings-panel" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>Silence Detection</h2>
          <button className="settings-close" onClick={onClose}>{"\u00D7"}</button>
        </div>
        <div className="settings-body">
          <div className="settings-status">
            Status: <span className={isEnabled ? "status-enabled" : "status-disabled"}>
              {isEnabled ? "Enabled" : "Disabled"}
            </span>
          </div>
          <div className="settings-field">
            <label className="settings-label">Silence Threshold</label>
            <div className="settings-input-row">
              <input
                type="number"
                className="settings-input"
                min={0}
                max={1}
                step={0.005}
                value={threshold}
                onChange={(e) => setThreshold(e.target.value)}
              />
              <span className="settings-hint">RMS amplitude (0–1), e.g. 0.01</span>
            </div>
          </div>
          <div className="settings-field">
            <label className="settings-label">Skip After (seconds)</label>
            <div className="settings-input-row">
              <input
                type="number"
                className="settings-input"
                min={0}
                max={300}
                step={1}
                value={durationSecs}
                onChange={(e) => setDurationSecs(e.target.value)}
              />
              <span className="settings-hint">0 = disabled</span>
            </div>
          </div>
        </div>
        <div className="settings-footer">
          {isEnabled && (
            <button className="settings-btn settings-btn-danger" onClick={handleDisable} disabled={saving}>
              Disable
            </button>
          )}
          <button className="settings-btn settings-btn-save" onClick={handleSave} disabled={saving}>
            {saved ? "Saved!" : saving ? "Saving..." : "Save"}
          </button>
          <button className="settings-btn" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}

export default SilenceSettings;
