import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { RdsConfigResponse, RdsMessageInfo } from "./types";

const DAY_NAMES = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const DAY_SHORT = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MAX_RDS_TEXT = 64;

interface RdsConfigWindowProps {
  onClose: () => void;
}

function RdsConfigWindow({ onClose }: RdsConfigWindowProps) {
  const [config, setConfig] = useState<RdsConfigResponse | null>(null);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const [ip, setIp] = useState("");
  const [port, setPort] = useState(10001);
  const [defaultMessage, setDefaultMessage] = useState("");
  const [settingsSaved, setSettingsSaved] = useState(false);

  const loadConfig = useCallback(async () => {
    try {
      const result = await invoke<RdsConfigResponse>("get_rds_config");
      setConfig(result);
      setIp(result.ip);
      setPort(result.port);
      setDefaultMessage(result.default_message);
    } catch (e) {
      console.error("Failed to load RDS config:", e);
    }
  }, []);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  const messages = config?.messages ?? [];
  const selectedMsg = selectedIndex !== null ? messages.find((m) => m.index === selectedIndex) : null;

  const handleAdd = async () => {
    try {
      const idx = await invoke<number>("add_rds_message", { text: "New Message" });
      await loadConfig();
      setSelectedIndex(idx);
    } catch (e) {
      console.error("Failed to add RDS message:", e);
    }
  };

  const handleDelete = async () => {
    if (selectedIndex === null) return;
    try {
      await invoke("remove_rds_message", { index: selectedIndex });
      setSelectedIndex(null);
      await loadConfig();
    } catch (e) {
      console.error("Failed to remove RDS message:", e);
    }
  };

  const handleToggle = async (index: number) => {
    try {
      await invoke("toggle_rds_message", { index });
      await loadConfig();
    } catch (e) {
      console.error("Failed to toggle RDS message:", e);
    }
  };

  const handleUpdate = async (updates: Partial<RdsMessageInfo>) => {
    if (selectedIndex === null || !selectedMsg) return;
    const updated = { ...selectedMsg, ...updates };
    try {
      await invoke("update_rds_message", {
        index: selectedIndex,
        text: updated.text,
        enabled: updated.enabled,
        duration: updated.duration,
        scheduled: updated.scheduled,
        days: updated.days,
        hours: updated.hours,
      });
      await loadConfig();
    } catch (e) {
      console.error("Failed to update RDS message:", e);
    }
  };

  const handleMoveUp = async () => {
    if (selectedIndex === null || selectedIndex === 0) return;
    try {
      await invoke("reorder_rds_message", { from: selectedIndex, to: selectedIndex - 1 });
      setSelectedIndex(selectedIndex - 1);
      await loadConfig();
    } catch (e) {
      console.error("Failed to reorder RDS message:", e);
    }
  };

  const handleMoveDown = async () => {
    if (selectedIndex === null || selectedIndex >= messages.length - 1) return;
    try {
      await invoke("reorder_rds_message", { from: selectedIndex, to: selectedIndex + 1 });
      setSelectedIndex(selectedIndex + 1);
      await loadConfig();
    } catch (e) {
      console.error("Failed to reorder RDS message:", e);
    }
  };

  const handleSaveSettings = async () => {
    try {
      await invoke("update_rds_settings", {
        ip,
        port,
        defaultMessage,
      });
      setSettingsSaved(true);
      setTimeout(() => setSettingsSaved(false), 1500);
    } catch (e) {
      console.error("Failed to save RDS settings:", e);
    }
  };

  const toggleDay = (day: string) => {
    if (!selectedMsg) return;
    const newDays = selectedMsg.days.includes(day)
      ? selectedMsg.days.filter((d) => d !== day)
      : [...selectedMsg.days, day];
    handleUpdate({ days: newDays });
  };

  const toggleHour = (hour: number) => {
    if (!selectedMsg) return;
    const newHours = selectedMsg.hours.includes(hour)
      ? selectedMsg.hours.filter((h) => h !== hour)
      : [...selectedMsg.hours, hour];
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
      <div className="rds-config-window" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>RDS Configuration</h2>
          <button className="settings-close" onClick={onClose}>{"\u00D7"}</button>
        </div>

        {/* Connection settings bar */}
        <div className="rds-connection-bar">
          <div className="rds-conn-field">
            <label className="rds-conn-label">IP</label>
            <input
              type="text"
              className="settings-input rds-conn-input"
              value={ip}
              onChange={(e) => setIp(e.target.value)}
            />
          </div>
          <div className="rds-conn-field">
            <label className="rds-conn-label">Port</label>
            <input
              type="number"
              className="settings-input rds-conn-input rds-conn-port"
              value={port}
              min={1}
              max={65535}
              onChange={(e) => setPort(parseInt(e.target.value) || 10001)}
            />
          </div>
          <div className="rds-conn-field rds-conn-default">
            <label className="rds-conn-label">Default Message</label>
            <input
              type="text"
              className="settings-input rds-conn-input"
              value={defaultMessage}
              maxLength={MAX_RDS_TEXT}
              onChange={(e) => setDefaultMessage(e.target.value)}
            />
          </div>
          <button className="settings-btn rds-conn-save" onClick={handleSaveSettings}>
            {settingsSaved ? "Saved!" : "Save"}
          </button>
        </div>

        <div className="ad-config-body">
          {/* Left panel: message list */}
          <div className="ad-list-panel">
            <div className="ad-list-scroll">
              {messages.length === 0 && (
                <div className="ad-list-empty">No RDS messages</div>
              )}
              {messages.map((msg) => (
                <div
                  key={msg.index}
                  className={`ad-list-item ${selectedIndex === msg.index ? "selected" : ""} ${!msg.enabled ? "disabled" : ""}`}
                  onClick={() => setSelectedIndex(msg.index)}
                >
                  <span
                    className={`ad-enabled-dot ${msg.enabled ? "on" : "off"}`}
                    onClick={(e) => { e.stopPropagation(); handleToggle(msg.index); }}
                  />
                  <span className="ad-list-name rds-list-text">{msg.text}</span>
                </div>
              ))}
            </div>
            <div className="ad-list-actions">
              <button className="settings-btn" onClick={handleMoveUp} disabled={selectedIndex === null || selectedIndex === 0} title="Move Up">{"\u25B2"}</button>
              <button className="settings-btn" onClick={handleMoveDown} disabled={selectedIndex === null || selectedIndex >= messages.length - 1} title="Move Down">{"\u25BC"}</button>
              <button className="settings-btn" onClick={handleAdd} title="Add New">+</button>
              <button className="settings-btn settings-btn-danger" onClick={handleDelete} disabled={selectedIndex === null} title="Delete">{"\u00D7"}</button>
            </div>
          </div>

          {/* Right panel: detail editor */}
          <div className="ad-detail-panel">
            {selectedMsg ? (
              <div className="settings-body">
                <div className="settings-field">
                  <label className="settings-label">
                    Message Text
                    <span className={`rds-char-count ${selectedMsg.text.length > MAX_RDS_TEXT ? "over" : ""}`}>
                      {selectedMsg.text.length}/{MAX_RDS_TEXT}
                    </span>
                  </label>
                  <input
                    type="text"
                    className="settings-input rds-text-input"
                    value={selectedMsg.text}
                    maxLength={MAX_RDS_TEXT}
                    onChange={(e) => handleUpdate({ text: e.target.value })}
                    placeholder="Message text (use {artist} and {title} for placeholders)"
                  />
                  <div className="rds-placeholder-hint">
                    Placeholders: <code>{"{artist}"}</code> (UPPERCASE), <code>{"{title}"}</code> (as-is)
                  </div>
                </div>

                <div className="settings-field">
                  <label className="settings-label">
                    <input
                      type="checkbox"
                      checked={selectedMsg.enabled}
                      onChange={(e) => handleUpdate({ enabled: e.target.checked })}
                    />
                    {" "}Enabled
                  </label>
                </div>

                <div className="settings-field">
                  <label className="settings-label">Duration (seconds)</label>
                  <input
                    type="number"
                    className="settings-input rds-duration-input"
                    value={selectedMsg.duration}
                    min={1}
                    max={60}
                    onChange={(e) => handleUpdate({ duration: Math.max(1, Math.min(60, parseInt(e.target.value) || 10)) })}
                  />
                  <span className="settings-hint">How long this message displays before rotating (1–60s)</span>
                </div>

                <div className="settings-field">
                  <label className="settings-label">
                    <input
                      type="checkbox"
                      checked={selectedMsg.scheduled}
                      onChange={(e) => handleUpdate({ scheduled: e.target.checked })}
                    />
                    {" "}Scheduled (restrict to specific days/hours)
                  </label>
                </div>

                <div className={`ad-schedule-section ${!selectedMsg.scheduled ? "disabled-section" : ""}`}>
                  <div className="settings-field">
                    <label className="settings-label">
                      Days
                      <span className="ad-select-actions">
                        <button className="ad-select-link" onClick={() => handleUpdate({ days: [...DAY_NAMES] })} disabled={!selectedMsg.scheduled}>All</button>
                        <button className="ad-select-link" onClick={() => handleUpdate({ days: [] })} disabled={!selectedMsg.scheduled}>Clear</button>
                      </span>
                    </label>
                    <div className="ad-day-grid">
                      {DAY_NAMES.map((day, i) => (
                        <button
                          key={day}
                          className={`schedule-day-btn ${selectedMsg.days.includes(day) ? "active" : ""}`}
                          onClick={() => toggleDay(day)}
                          disabled={!selectedMsg.scheduled}
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
                        <button className="ad-select-link" onClick={() => handleUpdate({ hours: Array.from({ length: 24 }, (_, i) => i) })} disabled={!selectedMsg.scheduled}>All</button>
                        <button className="ad-select-link" onClick={() => handleUpdate({ hours: [] })} disabled={!selectedMsg.scheduled}>Clear</button>
                      </span>
                    </label>
                    <div className="ad-hour-grid">
                      {Array.from({ length: 24 }, (_, h) => (
                        <button
                          key={h}
                          className={`schedule-day-btn ${selectedMsg.hours.includes(h) ? "active" : ""}`}
                          onClick={() => toggleHour(h)}
                          disabled={!selectedMsg.scheduled}
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
                {messages.length > 0 ? "Select a message to edit" : "Click + to add an RDS message"}
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

export default RdsConfigWindow;
