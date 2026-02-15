import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { cleanPath } from "./pathUtils";
import type { AdInfo } from "./types";

const AUDIO_EXTENSIONS = ["mp3", "wav", "flac", "ogg", "aac", "m4a"];
const DAY_NAMES = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const DAY_SHORT = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

interface AdConfigWindowProps {
  onClose: () => void;
}

function AdConfigWindow({ onClose }: AdConfigWindowProps) {
  const [ads, setAds] = useState<AdInfo[]>([]);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);

  const loadAds = useCallback(async () => {
    try {
      const result = await invoke<AdInfo[]>("get_ads");
      setAds(result);
    } catch (e) {
      console.error("Failed to load ads:", e);
    }
  }, []);

  useEffect(() => {
    loadAds();
  }, [loadAds]);

  const selectedAd = selectedIndex !== null ? ads.find((a) => a.index === selectedIndex) : null;

  const handleAdd = async () => {
    try {
      const idx = await invoke<number>("add_ad", { name: "New Ad", mp3File: "" });
      await loadAds();
      setSelectedIndex(idx);
    } catch (e) {
      console.error("Failed to add ad:", e);
    }
  };

  const handleDelete = async () => {
    if (selectedIndex === null) return;
    try {
      await invoke("remove_ad", { index: selectedIndex });
      setSelectedIndex(null);
      await loadAds();
    } catch (e) {
      console.error("Failed to remove ad:", e);
    }
  };

  const handleToggle = async (index: number) => {
    try {
      await invoke("toggle_ad", { index });
      await loadAds();
    } catch (e) {
      console.error("Failed to toggle ad:", e);
    }
  };

  const handleUpdate = async (updates: Partial<AdInfo>) => {
    if (selectedIndex === null || !selectedAd) return;
    const updated = { ...selectedAd, ...updates };
    try {
      await invoke("update_ad", {
        index: selectedIndex,
        name: updated.name,
        enabled: updated.enabled,
        mp3File: updated.mp3_file,
        scheduled: updated.scheduled,
        days: updated.days,
        hours: updated.hours,
      });
      await loadAds();
    } catch (e) {
      console.error("Failed to update ad:", e);
    }
  };

  const handleMoveUp = async () => {
    if (selectedIndex === null || selectedIndex === 0) return;
    try {
      await invoke("reorder_ad", { from: selectedIndex, to: selectedIndex - 1 });
      setSelectedIndex(selectedIndex - 1);
      await loadAds();
    } catch (e) {
      console.error("Failed to reorder ad:", e);
    }
  };

  const handleMoveDown = async () => {
    if (selectedIndex === null || selectedIndex >= ads.length - 1) return;
    try {
      await invoke("reorder_ad", { from: selectedIndex, to: selectedIndex + 1 });
      setSelectedIndex(selectedIndex + 1);
      await loadAds();
    } catch (e) {
      console.error("Failed to reorder ad:", e);
    }
  };

  const handleBrowse = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Audio Files", extensions: AUDIO_EXTENSIONS }],
      });
      if (selected && typeof selected === "string") {
        handleUpdate({ mp3_file: cleanPath(selected) });
      }
    } catch (e) {
      console.error("Failed to open file dialog:", e);
    }
  };

  const toggleDay = (day: string) => {
    if (!selectedAd) return;
    const newDays = selectedAd.days.includes(day)
      ? selectedAd.days.filter((d) => d !== day)
      : [...selectedAd.days, day];
    handleUpdate({ days: newDays });
  };

  const toggleHour = (hour: number) => {
    if (!selectedAd) return;
    const newHours = selectedAd.hours.includes(hour)
      ? selectedAd.hours.filter((h) => h !== hour)
      : [...selectedAd.hours, hour];
    handleUpdate({ hours: newHours });
  };

  const formatHour = (h: number): string => {
    if (h === 0) return "12 AM";
    if (h < 12) return `${h} AM`;
    if (h === 12) return "12 PM";
    return `${h - 12} PM`;
  };

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="ad-config-window" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>Ad Configuration</h2>
          <button className="settings-close" onClick={onClose}>{"\u00D7"}</button>
        </div>
        <div className="ad-config-body">
          {/* Left panel: ad list */}
          <div className="ad-list-panel">
            <div className="ad-list-scroll">
              {ads.length === 0 && (
                <div className="ad-list-empty">No ads configured</div>
              )}
              {ads.map((ad) => (
                <div
                  key={ad.index}
                  className={`ad-list-item ${selectedIndex === ad.index ? "selected" : ""} ${!ad.enabled ? "disabled" : ""}`}
                  onClick={() => setSelectedIndex(ad.index)}
                >
                  <span className={`ad-enabled-dot ${ad.enabled ? "on" : "off"}`} onClick={(e) => { e.stopPropagation(); handleToggle(ad.index); }} />
                  <span className="ad-list-name">{ad.name}</span>
                </div>
              ))}
            </div>
            <div className="ad-list-actions">
              <button className="settings-btn" onClick={handleMoveUp} disabled={selectedIndex === null || selectedIndex === 0} title="Move Up">{"\u25B2"}</button>
              <button className="settings-btn" onClick={handleMoveDown} disabled={selectedIndex === null || selectedIndex >= ads.length - 1} title="Move Down">{"\u25BC"}</button>
              <button className="settings-btn" onClick={handleAdd} title="Add New">+</button>
              <button className="settings-btn settings-btn-danger" onClick={handleDelete} disabled={selectedIndex === null} title="Delete">{"\u00D7"}</button>
            </div>
          </div>

          {/* Right panel: detail editor */}
          <div className="ad-detail-panel">
            {selectedAd ? (
              <div className="settings-body">
                <div className="settings-field">
                  <label className="settings-label">Name</label>
                  <input
                    type="text"
                    className="settings-input ad-name-input"
                    value={selectedAd.name}
                    onChange={(e) => handleUpdate({ name: e.target.value })}
                  />
                </div>

                <div className="settings-field">
                  <label className="settings-label">
                    <input
                      type="checkbox"
                      checked={selectedAd.enabled}
                      onChange={(e) => handleUpdate({ enabled: e.target.checked })}
                    />
                    {" "}Enabled
                  </label>
                </div>

                <div className="settings-field">
                  <label className="settings-label">MP3 File</label>
                  <div className="settings-input-row">
                    <input
                      type="text"
                      className="settings-input settings-input-path"
                      value={selectedAd.mp3_file}
                      readOnly
                      placeholder="No file selected"
                    />
                    <button className="settings-btn settings-btn-browse" onClick={handleBrowse}>Browse</button>
                  </div>
                </div>

                <div className="settings-field">
                  <label className="settings-label">
                    <input
                      type="checkbox"
                      checked={selectedAd.scheduled}
                      onChange={(e) => handleUpdate({ scheduled: e.target.checked })}
                    />
                    {" "}Scheduled (restrict to specific days/hours)
                  </label>
                </div>

                <div className={`ad-schedule-section ${!selectedAd.scheduled ? "disabled-section" : ""}`}>
                  <div className="settings-field">
                    <label className="settings-label">
                      Days
                      <span className="ad-select-actions">
                        <button className="ad-select-link" onClick={() => handleUpdate({ days: [...DAY_NAMES] })} disabled={!selectedAd.scheduled}>All</button>
                        <button className="ad-select-link" onClick={() => handleUpdate({ days: [] })} disabled={!selectedAd.scheduled}>Clear</button>
                      </span>
                    </label>
                    <div className="ad-day-grid">
                      {DAY_NAMES.map((day, i) => (
                        <button
                          key={day}
                          className={`schedule-day-btn ${selectedAd.days.includes(day) ? "active" : ""}`}
                          onClick={() => toggleDay(day)}
                          disabled={!selectedAd.scheduled}
                        >
                          {DAY_SHORT[i]}
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className="settings-field">
                    <label className="settings-label">
                      Hours
                      <span className="ad-select-actions">
                        <button className="ad-select-link" onClick={() => handleUpdate({ hours: Array.from({ length: 24 }, (_, i) => i) })} disabled={!selectedAd.scheduled}>All</button>
                        <button className="ad-select-link" onClick={() => handleUpdate({ hours: [] })} disabled={!selectedAd.scheduled}>Clear</button>
                      </span>
                    </label>
                    <div className="ad-hour-grid">
                      {Array.from({ length: 24 }, (_, h) => (
                        <button
                          key={h}
                          className={`schedule-day-btn ${selectedAd.hours.includes(h) ? "active" : ""}`}
                          onClick={() => toggleHour(h)}
                          disabled={!selectedAd.scheduled}
                        >
                          {formatHour(h)}
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
              </div>
            ) : (
              <div className="ad-detail-empty">
                {ads.length > 0 ? "Select an ad to edit" : "Click + to add an ad"}
              </div>
            )}
          </div>
        </div>
        <div className="settings-footer">
          <button className="settings-btn" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}

export default AdConfigWindow;
