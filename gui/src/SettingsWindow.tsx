import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ConfigResponse } from "./types";

interface SettingsWindowProps {
  onClose: () => void;
  initialTab?: string;
}

type TabId = "crossfade" | "silence" | "intro" | "nowplaying" | "conflict";

const TABS: { id: TabId; label: string }[] = [
  { id: "crossfade", label: "Crossfade" },
  { id: "silence", label: "Silence Detection" },
  { id: "intro", label: "Auto-Intro" },
  { id: "nowplaying", label: "Now-Playing XML" },
  { id: "conflict", label: "Conflict Policy" },
];

function SettingsWindow({ onClose, initialTab }: SettingsWindowProps) {
  const [activeTab, setActiveTab] = useState<TabId>(
    (initialTab as TabId) || "crossfade"
  );
  const [config, setConfig] = useState<ConfigResponse | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  // Crossfade
  const [fadeSecs, setFadeSecs] = useState("0");
  const [curveType] = useState("linear");

  // Silence
  const [silenceThreshold, setSilenceThreshold] = useState("0.01");
  const [silenceDuration, setSilenceDuration] = useState("0");

  // Intro
  const [introsFolder, setIntrosFolder] = useState<string | null>(null);
  const [introInterval, setIntroInterval] = useState("0");
  const [introDuck, setIntroDuck] = useState("0.3");

  // Now-Playing
  const [nowPlayingPath, setNowPlayingPath] = useState<string | null>(null);

  // Conflict
  const [conflictPolicy, setConflictPolicy] = useState("schedule-wins");

  useEffect(() => {
    (async () => {
      try {
        const c = await invoke<ConfigResponse>("get_config");
        setConfig(c);
        setFadeSecs(String(c.crossfade_secs));
        setSilenceThreshold(String(c.silence_threshold));
        setSilenceDuration(String(c.silence_duration_secs));
        setIntrosFolder(c.intros_folder);
        setIntroInterval(String(c.recurring_intro_interval_secs));
        setIntroDuck(String(c.recurring_intro_duck_volume));
        setNowPlayingPath(c.now_playing_path);
        setConflictPolicy(c.conflict_policy);
      } catch (e) {
        console.error("Failed to load config:", e);
      }
    })();
  }, []);

  const showSaved = () => {
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  };

  // ── Save handlers per tab ──

  const saveCrossfade = async () => {
    const secs = parseFloat(fadeSecs);
    if (isNaN(secs) || secs < 0) return;
    setSaving(true);
    try {
      await invoke("set_crossfade", { secs });
      showSaved();
    } catch (e) {
      console.error("Failed to set crossfade:", e);
    } finally {
      setSaving(false);
    }
  };

  const saveSilence = async () => {
    const t = parseFloat(silenceThreshold);
    const d = parseFloat(silenceDuration);
    if (isNaN(t) || isNaN(d) || t < 0 || d < 0) return;
    setSaving(true);
    try {
      await invoke("set_silence_detection", { threshold: t, durationSecs: d });
      showSaved();
    } catch (e) {
      console.error("Failed to set silence detection:", e);
    } finally {
      setSaving(false);
    }
  };

  const disableSilence = async () => {
    setSaving(true);
    try {
      await invoke("set_silence_detection", { threshold: 0, durationSecs: 0 });
      setSilenceThreshold("0");
      setSilenceDuration("0");
      showSaved();
    } catch (e) {
      console.error("Failed to disable silence detection:", e);
    } finally {
      setSaving(false);
    }
  };

  const browseIntrosFolder = async () => {
    try {
      const selected = await open({ directory: true });
      if (selected && typeof selected === "string") {
        setIntrosFolder(selected);
      }
    } catch (e) {
      console.error("Failed to open folder dialog:", e);
    }
  };

  const saveIntro = async () => {
    setSaving(true);
    try {
      await invoke("set_intros_folder", { path: introsFolder || null });
      const interval = parseFloat(introInterval);
      const duck = parseFloat(introDuck);
      if (!isNaN(interval) && !isNaN(duck)) {
        await invoke("set_recurring_intro", {
          intervalSecs: interval,
          duckVolume: duck,
        });
      }
      showSaved();
    } catch (e) {
      console.error("Failed to save intro settings:", e);
    } finally {
      setSaving(false);
    }
  };

  const disableIntro = async () => {
    setSaving(true);
    try {
      await invoke("set_intros_folder", { path: null });
      await invoke("set_recurring_intro", { intervalSecs: 0, duckVolume: 0.3 });
      setIntrosFolder(null);
      setIntroInterval("0");
      setIntroDuck("0.3");
      showSaved();
    } catch (e) {
      console.error("Failed to disable intros:", e);
    } finally {
      setSaving(false);
    }
  };

  const browseNowPlaying = async () => {
    try {
      const selected = await open({
        filters: [{ name: "XML Files", extensions: ["xml"] }],
      });
      if (selected && typeof selected === "string") {
        setNowPlayingPath(selected);
      }
    } catch (e) {
      console.error("Failed to open file dialog:", e);
    }
  };

  const saveNowPlaying = async () => {
    setSaving(true);
    try {
      await invoke("set_nowplaying_path", { path: nowPlayingPath || null });
      showSaved();
    } catch (e) {
      console.error("Failed to save now-playing path:", e);
    } finally {
      setSaving(false);
    }
  };

  const disableNowPlaying = async () => {
    setSaving(true);
    try {
      await invoke("set_nowplaying_path", { path: null });
      setNowPlayingPath(null);
      showSaved();
    } catch (e) {
      console.error("Failed to disable now-playing:", e);
    } finally {
      setSaving(false);
    }
  };

  const saveConflict = async () => {
    setSaving(true);
    try {
      await invoke("set_conflict_policy", { policy: conflictPolicy });
      showSaved();
    } catch (e) {
      console.error("Failed to set conflict policy:", e);
    } finally {
      setSaving(false);
    }
  };

  // ── Per-tab save dispatch ──

  const handleSave = () => {
    switch (activeTab) {
      case "crossfade": return saveCrossfade();
      case "silence": return saveSilence();
      case "intro": return saveIntro();
      case "nowplaying": return saveNowPlaying();
      case "conflict": return saveConflict();
    }
  };

  // ── Tab content renderers ──

  const silenceEnabled = parseFloat(silenceDuration) > 0 && parseFloat(silenceThreshold) > 0;
  const introEnabled = introsFolder !== null && introsFolder.length > 0;
  const introRecurring = parseFloat(introInterval) > 0;
  const nowPlayingEnabled = nowPlayingPath !== null && nowPlayingPath.length > 0;

  const renderDisableButton = () => {
    if (activeTab === "silence" && silenceEnabled) {
      return <button className="settings-btn settings-btn-danger" onClick={disableSilence} disabled={saving}>Disable</button>;
    }
    if (activeTab === "intro" && introEnabled) {
      return <button className="settings-btn settings-btn-danger" onClick={disableIntro} disabled={saving}>Disable</button>;
    }
    if (activeTab === "nowplaying" && nowPlayingEnabled) {
      return <button className="settings-btn settings-btn-danger" onClick={disableNowPlaying} disabled={saving}>Disable</button>;
    }
    return null;
  };

  if (!config) {
    return (
      <div className="settings-overlay" onClick={onClose}>
        <div className="settings-window" onClick={(e) => e.stopPropagation()}>
          <div className="settings-header">
            <h2>Settings</h2>
            <button className="settings-close" onClick={onClose}>{"\u00D7"}</button>
          </div>
          <div className="settings-body" style={{ padding: 32, textAlign: "center" }}>
            Loading...
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="settings-window" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>Settings</h2>
          <button className="settings-close" onClick={onClose}>{"\u00D7"}</button>
        </div>
        <div className="settings-window-body">
          <nav className="settings-tabs">
            {TABS.map((tab) => (
              <button
                key={tab.id}
                className={`settings-tab ${activeTab === tab.id ? "active" : ""}`}
                onClick={() => { setActiveTab(tab.id); setSaved(false); }}
              >
                {tab.label}
              </button>
            ))}
          </nav>
          <div className="settings-content">
            {activeTab === "crossfade" && (
              <div className="settings-body">
                <div className="settings-field">
                  <label className="settings-label">Fade Duration (seconds)</label>
                  <div className="settings-input-row">
                    <input
                      type="number"
                      className="settings-input"
                      min={0} max={30} step={0.5}
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
            )}

            {activeTab === "silence" && (
              <div className="settings-body">
                <div className="settings-status">
                  Status: <span className={silenceEnabled ? "status-enabled" : "status-disabled"}>
                    {silenceEnabled ? "Enabled" : "Disabled"}
                  </span>
                </div>
                <div className="settings-field">
                  <label className="settings-label">Silence Threshold</label>
                  <div className="settings-input-row">
                    <input
                      type="number"
                      className="settings-input"
                      min={0} max={1} step={0.005}
                      value={silenceThreshold}
                      onChange={(e) => setSilenceThreshold(e.target.value)}
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
                      min={0} max={300} step={1}
                      value={silenceDuration}
                      onChange={(e) => setSilenceDuration(e.target.value)}
                    />
                    <span className="settings-hint">0 = disabled</span>
                  </div>
                </div>
              </div>
            )}

            {activeTab === "intro" && (
              <div className="settings-body">
                <div className="settings-status">
                  Status: <span className={introEnabled ? "status-enabled" : "status-disabled"}>
                    {introEnabled ? "Enabled" : "Disabled"}
                  </span>
                </div>
                <div className="settings-field">
                  <label className="settings-label">Intros Folder</label>
                  <div className="settings-input-row">
                    <input
                      type="text"
                      className="settings-input settings-input-path"
                      value={introsFolder ?? ""}
                      readOnly
                      placeholder="No folder selected"
                    />
                    <button className="settings-btn settings-btn-browse" onClick={browseIntrosFolder}>
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
                      min={0} max={3600} step={1}
                      value={introInterval}
                      onChange={(e) => setIntroInterval(e.target.value)}
                    />
                    <span className="settings-hint">
                      {introRecurring ? `Re-play intro every ${introInterval}s` : "0 = disabled"}
                    </span>
                  </div>
                </div>
                <div className="settings-field">
                  <label className="settings-label">Duck Volume</label>
                  <div className="settings-input-row">
                    <input
                      type="number"
                      className="settings-input"
                      min={0} max={1} step={0.05}
                      value={introDuck}
                      onChange={(e) => setIntroDuck(e.target.value)}
                    />
                    <span className="settings-hint">Main track volume during recurring intro (0–1)</span>
                  </div>
                </div>
              </div>
            )}

            {activeTab === "nowplaying" && (
              <div className="settings-body">
                <div className="settings-status">
                  Status: <span className={nowPlayingEnabled ? "status-enabled" : "status-disabled"}>
                    {nowPlayingEnabled ? "Enabled" : "Disabled"}
                  </span>
                </div>
                <div className="settings-field">
                  <label className="settings-label">XML Output Path</label>
                  <div className="settings-input-row">
                    <input
                      type="text"
                      className="settings-input settings-input-path"
                      value={nowPlayingPath ?? ""}
                      readOnly
                      placeholder="No path set"
                    />
                    <button className="settings-btn settings-btn-browse" onClick={browseNowPlaying}>
                      Browse
                    </button>
                  </div>
                  <span className="settings-hint">XML file updated with current/next track info</span>
                </div>
              </div>
            )}

            {activeTab === "conflict" && (
              <div className="settings-body">
                <div className="settings-field">
                  <label className="settings-label">Conflict Resolution Policy</label>
                  <select
                    className="settings-select"
                    value={conflictPolicy}
                    onChange={(e) => setConflictPolicy(e.target.value)}
                  >
                    <option value="schedule-wins">Schedule Wins</option>
                    <option value="manual-wins">Manual Wins</option>
                  </select>
                  <span className="settings-hint">
                    {conflictPolicy === "schedule-wins"
                      ? "All scheduled events fire regardless of manual playback"
                      : "Only priority 7+ events fire during manual playback"}
                  </span>
                </div>
              </div>
            )}
          </div>
        </div>
        <div className="settings-footer">
          {renderDisableButton()}
          <button className="settings-btn settings-btn-save" onClick={handleSave} disabled={saving}>
            {saved ? "Saved!" : saving ? "Saving..." : "Save"}
          </button>
          <button className="settings-btn" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}

export default SettingsWindow;
