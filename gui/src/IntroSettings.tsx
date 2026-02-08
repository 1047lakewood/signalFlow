import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ConfigResponse } from "./types";

interface IntroSettingsProps {
  onClose: () => void;
}

function IntroSettings({ onClose }: IntroSettingsProps) {
  const [folder, setFolder] = useState<string | null>(null);
  const [intervalSecs, setIntervalSecs] = useState<string>("0");
  const [duckVolume, setDuckVolume] = useState<string>("0.3");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const config = await invoke<ConfigResponse>("get_config");
        setFolder(config.intros_folder);
        setIntervalSecs(String(config.recurring_intro_interval_secs));
        setDuckVolume(String(config.recurring_intro_duck_volume));
      } catch (e) {
        console.error("Failed to load config:", e);
      }
    })();
  }, []);

  const isEnabled = folder !== null && folder.length > 0;
  const isRecurringEnabled = parseFloat(intervalSecs) > 0;

  const handleBrowse = async () => {
    try {
      const selected = await open({ directory: true });
      if (selected && typeof selected === "string") {
        setFolder(selected);
      }
    } catch (e) {
      console.error("Failed to open folder dialog:", e);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await invoke("set_intros_folder", { path: folder || null });
      const interval = parseFloat(intervalSecs);
      const duck = parseFloat(duckVolume);
      if (!isNaN(interval) && !isNaN(duck)) {
        await invoke("set_recurring_intro", {
          intervalSecs: interval,
          duckVolume: duck,
        });
      }
      setSaved(true);
      setTimeout(() => setSaved(false), 1500);
    } catch (e) {
      console.error("Failed to save intro settings:", e);
    } finally {
      setSaving(false);
    }
  };

  const handleDisable = async () => {
    setSaving(true);
    try {
      await invoke("set_intros_folder", { path: null });
      await invoke("set_recurring_intro", { intervalSecs: 0, duckVolume: 0.3 });
      setFolder(null);
      setIntervalSecs("0");
      setDuckVolume("0.3");
      setSaved(true);
      setTimeout(() => setSaved(false), 1500);
    } catch (e) {
      console.error("Failed to disable intros:", e);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="settings-panel" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>Auto-Intro</h2>
          <button className="settings-close" onClick={onClose}>{"\u00D7"}</button>
        </div>
        <div className="settings-body">
          <div className="settings-status">
            Status: <span className={isEnabled ? "status-enabled" : "status-disabled"}>
              {isEnabled ? "Enabled" : "Disabled"}
            </span>
          </div>
          <div className="settings-field">
            <label className="settings-label">Intros Folder</label>
            <div className="settings-input-row">
              <input
                type="text"
                className="settings-input settings-input-path"
                value={folder ?? ""}
                readOnly
                placeholder="No folder selected"
              />
              <button className="settings-btn settings-btn-browse" onClick={handleBrowse}>
                Browse
              </button>
            </div>
            <span className="settings-hint">Folder containing Artist.mp3 intro files</span>
          </div>
          <div className="settings-field">
            <label className="settings-label">Recurring Interval (seconds)</label>
            <div className="settings-input-row">
              <input
                type="number"
                className="settings-input"
                min={0}
                max={3600}
                step={1}
                value={intervalSecs}
                onChange={(e) => setIntervalSecs(e.target.value)}
              />
              <span className="settings-hint">
                {isRecurringEnabled
                  ? `Re-play intro every ${intervalSecs}s`
                  : "0 = disabled"}
              </span>
            </div>
          </div>
          <div className="settings-field">
            <label className="settings-label">Duck Volume</label>
            <div className="settings-input-row">
              <input
                type="number"
                className="settings-input"
                min={0}
                max={1}
                step={0.05}
                value={duckVolume}
                onChange={(e) => setDuckVolume(e.target.value)}
              />
              <span className="settings-hint">Main track volume during recurring intro (0–1)</span>
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

export default IntroSettings;
