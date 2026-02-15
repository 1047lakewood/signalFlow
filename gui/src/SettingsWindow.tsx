import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { cleanPath } from "./pathUtils";
import type { ConfigResponse } from "./types";

interface SettingsWindowProps {
  onClose: () => void;
  initialTab?: string;
}

type TabId =
  | "library"
  | "audio"
  | "crossfade"
  | "silence"
  | "intro"
  | "nowplaying"
  | "streaming"
  | "recording"
  | "conflict";

const TABS: { id: TabId; label: string }[] = [
  { id: "library", label: "Library" },
  { id: "audio", label: "Audio Output" },
  { id: "crossfade", label: "Crossfade" },
  { id: "silence", label: "Silence Detection" },
  { id: "intro", label: "Auto-Intro" },
  { id: "nowplaying", label: "Now-Playing XML" },
  { id: "streaming", label: "Streaming" },
  { id: "recording", label: "Recording" },
  { id: "conflict", label: "Conflict Policy" },
];

function SettingsWindow({ onClose, initialTab }: SettingsWindowProps) {
  const [activeTab, setActiveTab] = useState<TabId>(
    (initialTab as TabId) || "library",
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

  // Streaming
  const [streamOutputEnabled, setStreamOutputEnabled] = useState(false);
  const [streamOutputUrl, setStreamOutputUrl] = useState("");

  // Recording
  const [recordingEnabled, setRecordingEnabled] = useState(false);
  const [recordingOutputDir, setRecordingOutputDir] = useState<string | null>(
    null,
  );

  // Conflict
  const [conflictPolicy, setConflictPolicy] = useState("schedule-wins");

  // Library
  const [indexedLocations, setIndexedLocations] = useState<string[]>([]);
  const [favoriteFolders, setFavoriteFolders] = useState<string[]>([]);

  // Audio Output
  const [outputDevices, setOutputDevices] = useState<string[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<string | null>(null);

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
        setStreamOutputEnabled(c.stream_output_enabled);
        setStreamOutputUrl(c.stream_output_url);
        setRecordingEnabled(c.recording_enabled);
        setRecordingOutputDir(c.recording_output_dir);
        setConflictPolicy(c.conflict_policy);
        setIndexedLocations(c.indexed_locations || []);
        setFavoriteFolders(c.favorite_folders || []);
        setSelectedDevice(c.output_device_name ?? null);
        try {
          const devices = await invoke<string[]>("list_output_devices");
          setOutputDevices(devices);
        } catch (e2) {
          console.error("Failed to list output devices:", e2);
        }
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
        setIntrosFolder(cleanPath(selected));
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
        setNowPlayingPath(cleanPath(selected));
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

  const saveStreaming = async () => {
    setSaving(true);
    try {
      await invoke("set_stream_output", {
        enabled: streamOutputEnabled,
        endpointUrl: streamOutputUrl,
      });
      showSaved();
    } catch (e) {
      console.error("Failed to save stream output:", e);
    } finally {
      setSaving(false);
    }
  };

  const disableStreaming = async () => {
    setSaving(true);
    try {
      await invoke("set_stream_output", {
        enabled: false,
        endpointUrl: streamOutputUrl,
      });
      setStreamOutputEnabled(false);
      showSaved();
    } catch (e) {
      console.error("Failed to disable stream output:", e);
    } finally {
      setSaving(false);
    }
  };

  const browseRecordingDir = async () => {
    try {
      const selected = await open({ directory: true });
      if (selected && typeof selected === "string") {
        setRecordingOutputDir(cleanPath(selected));
      }
    } catch (e) {
      console.error("Failed to browse recording directory:", e);
    }
  };

  const saveRecording = async () => {
    setSaving(true);
    try {
      await invoke("set_recording", {
        enabled: recordingEnabled,
        outputDir: recordingOutputDir,
      });
      showSaved();
    } catch (e) {
      console.error("Failed to save recording config:", e);
    } finally {
      setSaving(false);
    }
  };

  const disableRecording = async () => {
    setSaving(true);
    try {
      await invoke("set_recording", {
        enabled: false,
        outputDir: recordingOutputDir,
      });
      setRecordingEnabled(false);
      showSaved();
    } catch (e) {
      console.error("Failed to disable recording:", e);
    } finally {
      setSaving(false);
    }
  };

  const browseAndAddIndexedLocation = async () => {
    try {
      const selected = await open({ directory: true });
      if (selected && typeof selected === "string") {
        const cleaned = cleanPath(selected);
        if (!indexedLocations.includes(cleaned)) {
          setIndexedLocations((prev) => [...prev, cleaned]);
        }
      }
    } catch (e) {
      console.error("Failed to browse indexed location:", e);
    }
  };

  const browseAndAddFavoriteFolder = async () => {
    try {
      const selected = await open({ directory: true });
      if (selected && typeof selected === "string") {
        const cleaned = cleanPath(selected);
        if (!favoriteFolders.includes(cleaned)) {
          setFavoriteFolders((prev) => [...prev, cleaned]);
        }
      }
    } catch (e) {
      console.error("Failed to browse favorite folder:", e);
    }
  };

  const saveLibrary = async () => {
    setSaving(true);
    try {
      await invoke("set_indexed_locations", { locations: indexedLocations });
      await invoke("set_favorite_folders", { folders: favoriteFolders });
      showSaved();
    } catch (e) {
      console.error("Failed to save library settings:", e);
    } finally {
      setSaving(false);
    }
  };

  const saveAudioDevice = async () => {
    setSaving(true);
    try {
      await invoke("set_output_device", { name: selectedDevice || null });
      showSaved();
    } catch (e) {
      console.error("Failed to set output device:", e);
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
      case "library":
        return saveLibrary();
      case "audio":
        return saveAudioDevice();
      case "crossfade":
        return saveCrossfade();
      case "silence":
        return saveSilence();
      case "intro":
        return saveIntro();
      case "nowplaying":
        return saveNowPlaying();
      case "streaming":
        return saveStreaming();
      case "recording":
        return saveRecording();
      case "conflict":
        return saveConflict();
    }
  };

  // ── Tab content renderers ──

  const silenceEnabled =
    parseFloat(silenceDuration) > 0 && parseFloat(silenceThreshold) > 0;
  const introEnabled = introsFolder !== null && introsFolder.length > 0;
  const introRecurring = parseFloat(introInterval) > 0;
  const nowPlayingEnabled =
    nowPlayingPath !== null && nowPlayingPath.length > 0;

  const renderDisableButton = () => {
    if (activeTab === "silence" && silenceEnabled) {
      return (
        <button
          className="settings-btn settings-btn-danger"
          onClick={disableSilence}
          disabled={saving}
        >
          Disable
        </button>
      );
    }
    if (activeTab === "intro" && introEnabled) {
      return (
        <button
          className="settings-btn settings-btn-danger"
          onClick={disableIntro}
          disabled={saving}
        >
          Disable
        </button>
      );
    }
    if (activeTab === "nowplaying" && nowPlayingEnabled) {
      return (
        <button
          className="settings-btn settings-btn-danger"
          onClick={disableNowPlaying}
          disabled={saving}
        >
          Disable
        </button>
      );
    }
    if (activeTab === "streaming" && streamOutputEnabled) {
      return (
        <button
          className="settings-btn settings-btn-danger"
          onClick={disableStreaming}
          disabled={saving}
        >
          Disable
        </button>
      );
    }
    if (activeTab === "recording" && recordingEnabled) {
      return (
        <button
          className="settings-btn settings-btn-danger"
          onClick={disableRecording}
          disabled={saving}
        >
          Disable
        </button>
      );
    }
    return null;
  };

  if (!config) {
    return (
      <div className="settings-overlay" onClick={onClose}>
        <div className="settings-window" onClick={(e) => e.stopPropagation()}>
          <div className="settings-header">
            <h2>Options / Settings</h2>
            <button className="settings-close" onClick={onClose}>
              {"\u00D7"}
            </button>
          </div>
          <div
            className="settings-body"
            style={{ padding: 32, textAlign: "center" }}
          >
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
          <h2>Options / Settings</h2>
          <button className="settings-close" onClick={onClose}>
            {"\u00D7"}
          </button>
        </div>
        <div className="settings-window-body">
          <nav className="settings-tabs">
            {TABS.map((tab) => (
              <button
                key={tab.id}
                className={`settings-tab ${activeTab === tab.id ? "active" : ""}`}
                onClick={() => {
                  setActiveTab(tab.id);
                  setSaved(false);
                }}
              >
                {tab.label}
              </button>
            ))}
          </nav>
          <div className="settings-content">
            {activeTab === "library" && (
              <div className="settings-body">
                <div className="settings-field">
                  <label className="settings-label">
                    Indexed locations (instant search scope)
                  </label>
                  <div className="settings-list">
                    {indexedLocations.map((path) => (
                      <div key={path} className="settings-list-row">
                        <span>{path}</span>
                        <button
                          className="settings-btn settings-btn-danger"
                          onClick={() =>
                            setIndexedLocations((prev) =>
                              prev.filter((p) => p !== path),
                            )
                          }
                        >
                          Remove
                        </button>
                      </div>
                    ))}
                  </div>
                  <button
                    className="settings-btn settings-btn-browse"
                    onClick={browseAndAddIndexedLocation}
                  >
                    Add folder/drive
                  </button>
                </div>

                <div className="settings-field">
                  <label className="settings-label">
                    Favorite folders (file browser sidebar)
                  </label>
                  <div className="settings-list">
                    {favoriteFolders.map((path) => (
                      <div key={path} className="settings-list-row">
                        <span>{path}</span>
                        <button
                          className="settings-btn settings-btn-danger"
                          onClick={() =>
                            setFavoriteFolders((prev) =>
                              prev.filter((p) => p !== path),
                            )
                          }
                        >
                          Remove
                        </button>
                      </div>
                    ))}
                  </div>
                  <button
                    className="settings-btn settings-btn-browse"
                    onClick={browseAndAddFavoriteFolder}
                  >
                    Add favorite folder
                  </button>
                </div>
              </div>
            )}

            {activeTab === "audio" && (
              <div className="settings-body">
                <div className="settings-field">
                  <label className="settings-label">Output Device</label>
                  <select
                    className="settings-select"
                    value={selectedDevice ?? ""}
                    onChange={(e) =>
                      setSelectedDevice(e.target.value || null)
                    }
                  >
                    <option value="">System Default</option>
                    {outputDevices.map((name) => (
                      <option key={name} value={name}>
                        {name}
                      </option>
                    ))}
                  </select>
                  <span className="settings-hint">
                    {selectedDevice
                      ? `Using: ${selectedDevice}`
                      : "Using the system default audio device"}
                  </span>
                </div>
              </div>
            )}

            {activeTab === "crossfade" && (
              <div className="settings-body">
                <div className="settings-field">
                  <label className="settings-label">
                    Fade Duration (seconds)
                  </label>
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
                  <select
                    className="settings-select"
                    value={curveType}
                    disabled
                  >
                    <option value="linear">Linear</option>
                  </select>
                  <span className="settings-hint">
                    More curve types coming soon
                  </span>
                </div>
              </div>
            )}

            {activeTab === "silence" && (
              <div className="settings-body">
                <div className="settings-status">
                  Status:{" "}
                  <span
                    className={
                      silenceEnabled ? "status-enabled" : "status-disabled"
                    }
                  >
                    {silenceEnabled ? "Enabled" : "Disabled"}
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
                      value={silenceThreshold}
                      onChange={(e) => setSilenceThreshold(e.target.value)}
                    />
                    <span className="settings-hint">
                      RMS amplitude (0–1), e.g. 0.01
                    </span>
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
                  Status:{" "}
                  <span
                    className={
                      introEnabled ? "status-enabled" : "status-disabled"
                    }
                  >
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
                    <button
                      className="settings-btn settings-btn-browse"
                      onClick={browseIntrosFolder}
                    >
                      Browse
                    </button>
                  </div>
                  <span className="settings-hint">
                    Folder containing Artist.mp3 intro files
                  </span>
                </div>
                <div className="settings-field">
                  <label className="settings-label">
                    Recurring Interval (seconds)
                  </label>
                  <div className="settings-input-row">
                    <input
                      type="number"
                      className="settings-input"
                      min={0}
                      max={3600}
                      step={1}
                      value={introInterval}
                      onChange={(e) => setIntroInterval(e.target.value)}
                    />
                    <span className="settings-hint">
                      {introRecurring
                        ? `Re-play intro every ${introInterval}s`
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
                      value={introDuck}
                      onChange={(e) => setIntroDuck(e.target.value)}
                    />
                    <span className="settings-hint">
                      Main track volume during recurring intro (0–1)
                    </span>
                  </div>
                </div>
              </div>
            )}

            {activeTab === "nowplaying" && (
              <div className="settings-body">
                <div className="settings-status">
                  Status:{" "}
                  <span
                    className={
                      nowPlayingEnabled ? "status-enabled" : "status-disabled"
                    }
                  >
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
                    <button
                      className="settings-btn settings-btn-browse"
                      onClick={browseNowPlaying}
                    >
                      Browse
                    </button>
                  </div>
                  <span className="settings-hint">
                    XML file updated with current/next track info
                  </span>
                </div>
              </div>
            )}

            {activeTab === "streaming" && (
              <div className="settings-body">
                <div className="settings-status">
                  Status:{" "}
                  <span
                    className={
                      streamOutputEnabled ? "status-enabled" : "status-disabled"
                    }
                  >
                    {streamOutputEnabled ? "Enabled" : "Disabled"}
                  </span>
                </div>
                <div className="settings-field">
                  <label className="settings-label">
                    Enable streaming output
                  </label>
                  <label className="settings-checkbox-row">
                    <input
                      type="checkbox"
                      checked={streamOutputEnabled}
                      onChange={(e) => setStreamOutputEnabled(e.target.checked)}
                    />
                    <span>Relay playback to Icecast/Shoutcast endpoint</span>
                  </label>
                </div>
                <div className="settings-field">
                  <label className="settings-label">
                    Streaming endpoint URL
                  </label>
                  <input
                    type="text"
                    className="settings-input settings-input-path"
                    value={streamOutputUrl}
                    onChange={(e) => setStreamOutputUrl(e.target.value)}
                    placeholder="icecast://source:PASSWORD@host:8000/mount"
                  />
                  <span className="settings-hint">
                    Used by the stream relay process when enabled.
                  </span>
                </div>
              </div>
            )}

            {activeTab === "recording" && (
              <div className="settings-body">
                <div className="settings-status">
                  Status:{" "}
                  <span
                    className={
                      recordingEnabled ? "status-enabled" : "status-disabled"
                    }
                  >
                    {recordingEnabled ? "Enabled" : "Disabled"}
                  </span>
                </div>
                <div className="settings-field">
                  <label className="settings-label">
                    Enable daily recording
                  </label>
                  <label className="settings-checkbox-row">
                    <input
                      type="checkbox"
                      checked={recordingEnabled}
                      onChange={(e) => setRecordingEnabled(e.target.checked)}
                    />
                    <span>
                      Record playback output to one file per calendar day
                    </span>
                  </label>
                </div>
                <div className="settings-field">
                  <label className="settings-label">
                    Recording output folder
                  </label>
                  <div className="settings-input-row">
                    <input
                      type="text"
                      className="settings-input settings-input-path"
                      value={recordingOutputDir ?? ""}
                      readOnly
                      placeholder="No folder selected"
                    />
                    <button
                      className="settings-btn settings-btn-browse"
                      onClick={browseRecordingDir}
                    >
                      Browse
                    </button>
                  </div>
                </div>
              </div>
            )}

            {activeTab === "conflict" && (
              <div className="settings-body">
                <div className="settings-field">
                  <label className="settings-label">
                    Conflict Resolution Policy
                  </label>
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
          <button
            className="settings-btn settings-btn-save"
            onClick={handleSave}
            disabled={saving}
          >
            {saved ? "Saved!" : saving ? "Saving..." : "Save"}
          </button>
          <button className="settings-btn" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

export default SettingsWindow;
