import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ConfigResponse } from "./types";

interface CrossfadeSettingsProps {
  onClose: () => void;
}

function CrossfadeSettings({ onClose }: CrossfadeSettingsProps) {
  const [fadeSecs, setFadeSecs] = useState<string>("0");
  const [curveType] = useState("linear");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const config = await invoke<ConfigResponse>("get_config");
        setFadeSecs(String(config.crossfade_secs));
      } catch (e) {
        console.error("Failed to load config:", e);
      }
    })();
  }, []);

  const handleSave = async () => {
    const secs = parseFloat(fadeSecs);
    if (isNaN(secs) || secs < 0) return;
    setSaving(true);
    try {
      await invoke("set_crossfade", { secs });
      setSaved(true);
      setTimeout(() => setSaved(false), 1500);
    } catch (e) {
      console.error("Failed to set crossfade:", e);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="settings-panel" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>Crossfade Settings</h2>
          <button className="settings-close" onClick={onClose}>{"\u00D7"}</button>
        </div>
        <div className="settings-body">
          <div className="settings-field">
            <label className="settings-label">Fade Duration (seconds)</label>
            <div className="settings-input-row">
              <input
                type="number"
                className="settings-input"
                min={0}
                max={30}
                step={0.5}
                value={fadeSecs}
                onChange={(e) => setFadeSecs(e.target.value)}
              />
              <span className="settings-hint">0 = disabled</span>
            </div>
          </div>
          <div className="settings-field">
            <label className="settings-label">Curve Type</label>
            <select className="settings-select" value={curveType} disabled>
              <option value="linear">Linear</option>
            </select>
            <span className="settings-hint">More curve types coming soon</span>
          </div>
        </div>
        <div className="settings-footer">
          <button className="settings-btn settings-btn-save" onClick={handleSave} disabled={saving}>
            {saved ? "Saved!" : saving ? "Saving..." : "Save"}
          </button>
          <button className="settings-btn" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}

export default CrossfadeSettings;
