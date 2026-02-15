import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { cleanPath } from "./pathUtils";
import type { ScheduleEventInfo } from "./types";

const AUDIO_EXTENSIONS = ["mp3", "wav", "flac", "ogg", "aac", "m4a"];
const MODES = ["overlay", "stop", "insert"];
const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

interface SchedulePaneProps {
  onClose: () => void;
}

function SchedulePane({ onClose }: SchedulePaneProps) {
  const [events, setEvents] = useState<ScheduleEventInfo[]>([]);
  const [showAddForm, setShowAddForm] = useState(false);

  // Add form state
  const [newTime, setNewTime] = useState("12:00");
  const [newMode, setNewMode] = useState("overlay");
  const [newFile, setNewFile] = useState("");
  const [newPriority, setNewPriority] = useState(5);
  const [newLabel, setNewLabel] = useState("");
  const [newDays, setNewDays] = useState<number[]>([]);
  const [addError, setAddError] = useState<string | null>(null);

  const loadEvents = useCallback(async () => {
    try {
      const evts = await invoke<ScheduleEventInfo[]>("get_schedule");
      setEvents(evts);
    } catch (e) {
      console.error("Failed to load schedule:", e);
    }
  }, []);

  useEffect(() => {
    loadEvents();
  }, [loadEvents]);

  const handleToggle = async (id: number) => {
    try {
      await invoke("toggle_schedule_event", { id });
      await loadEvents();
    } catch (e) {
      console.error("Failed to toggle event:", e);
    }
  };

  const handleRemove = async (id: number) => {
    try {
      await invoke("remove_schedule_event", { id });
      await loadEvents();
    } catch (e) {
      console.error("Failed to remove event:", e);
    }
  };

  const handleBrowseFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: "Audio Files",
          extensions: AUDIO_EXTENSIONS,
        }],
      });
      if (selected && typeof selected === "string") {
        setNewFile(cleanPath(selected));
      }
    } catch (e) {
      console.error("Failed to open file dialog:", e);
    }
  };

  const handleDayToggle = (day: number) => {
    setNewDays((prev) =>
      prev.includes(day) ? prev.filter((d) => d !== day) : [...prev, day].sort()
    );
  };

  const handleAdd = async () => {
    setAddError(null);
    if (!newTime.trim()) {
      setAddError("Time is required");
      return;
    }
    if (!newFile.trim()) {
      setAddError("File is required");
      return;
    }
    try {
      await invoke("add_schedule_event", {
        time: newTime.trim(),
        mode: newMode,
        file: newFile.trim(),
        priority: newPriority,
        label: newLabel.trim() || null,
        days: newDays.length > 0 ? newDays : null,
      });
      // Reset form
      setNewTime("12:00");
      setNewMode("overlay");
      setNewFile("");
      setNewPriority(5);
      setNewLabel("");
      setNewDays([]);
      setShowAddForm(false);
      await loadEvents();
    } catch (e) {
      setAddError(String(e));
    }
  };

  const modeClass = (mode: string) => {
    switch (mode.toLowerCase()) {
      case "stop": return "sched-mode-stop";
      case "insert": return "sched-mode-insert";
      default: return "sched-mode-overlay";
    }
  };

  return (
    <div className="schedule-pane">
      <div className="schedule-pane-header">
        <h2>Schedule</h2>
        <div className="schedule-pane-actions">
          <button
            className="schedule-add-btn"
            onClick={() => setShowAddForm((v) => !v)}
            title="Add event"
          >
            +
          </button>
          <button
            className="settings-close"
            onClick={onClose}
            title="Close schedule"
          >
            {"\u00D7"}
          </button>
        </div>
      </div>

      {showAddForm && (
        <div className="schedule-add-form">
          <div className="settings-field">
            <label className="settings-label">Time (HH:MM or HH:MM:SS)</label>
            <input
              className="settings-input"
              type="text"
              value={newTime}
              onChange={(e) => setNewTime(e.target.value)}
              placeholder="14:00"
            />
          </div>
          <div className="settings-field">
            <label className="settings-label">Mode</label>
            <select
              className="settings-select schedule-mode-select"
              value={newMode}
              onChange={(e) => setNewMode(e.target.value)}
            >
              {MODES.map((m) => (
                <option key={m} value={m}>{m.charAt(0).toUpperCase() + m.slice(1)}</option>
              ))}
            </select>
          </div>
          <div className="settings-field">
            <label className="settings-label">Audio File</label>
            <div className="settings-input-row">
              <input
                className="settings-input settings-input-path"
                type="text"
                value={newFile}
                onChange={(e) => setNewFile(e.target.value)}
                placeholder="Path to audio file..."
                readOnly
              />
              <button className="settings-btn settings-btn-browse" onClick={handleBrowseFile}>
                Browse
              </button>
            </div>
          </div>
          <div className="settings-field">
            <label className="settings-label">Priority (1–9)</label>
            <input
              className="settings-input"
              type="number"
              min={1}
              max={9}
              value={newPriority}
              onChange={(e) => setNewPriority(Number(e.target.value))}
            />
          </div>
          <div className="settings-field">
            <label className="settings-label">Label (optional)</label>
            <input
              className="settings-input schedule-label-input"
              type="text"
              value={newLabel}
              onChange={(e) => setNewLabel(e.target.value)}
              placeholder="e.g. Afternoon news"
            />
          </div>
          <div className="settings-field">
            <label className="settings-label">Days (empty = daily)</label>
            <div className="schedule-days-row">
              {DAY_LABELS.map((label, i) => (
                <button
                  key={i}
                  className={`schedule-day-btn ${newDays.includes(i) ? "active" : ""}`}
                  onClick={() => handleDayToggle(i)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
          {addError && <div className="schedule-error">{addError}</div>}
          <div className="schedule-add-form-actions">
            <button className="settings-btn settings-btn-save" onClick={handleAdd}>
              Add Event
            </button>
            <button className="settings-btn" onClick={() => setShowAddForm(false)}>
              Cancel
            </button>
          </div>
        </div>
      )}

      <div className="schedule-event-list">
        {events.length === 0 ? (
          <div className="schedule-empty">No scheduled events</div>
        ) : (
          events.map((evt) => (
            <div key={evt.id} className={`schedule-event ${evt.enabled ? "" : "disabled"}`}>
              <div className="schedule-event-main">
                <span className="schedule-event-time">{evt.time}</span>
                <span className={`schedule-event-mode ${modeClass(evt.mode)}`}>{evt.mode}</span>
                <span className="schedule-event-label" title={evt.file}>
                  {evt.label || evt.file.split(/[/\\]/).pop() || evt.file}
                </span>
              </div>
              <div className="schedule-event-meta">
                <span className="schedule-event-priority" title="Priority">P{evt.priority}</span>
                {evt.days && <span className="schedule-event-days">{evt.days}</span>}
              </div>
              <div className="schedule-event-actions">
                <button
                  className={`schedule-toggle-btn ${evt.enabled ? "on" : "off"}`}
                  onClick={() => handleToggle(evt.id)}
                  title={evt.enabled ? "Disable" : "Enable"}
                >
                  {evt.enabled ? "ON" : "OFF"}
                </button>
                <button
                  className="schedule-remove-btn"
                  onClick={() => handleRemove(evt.id)}
                  title="Remove event"
                >
                  {"\u00D7"}
                </button>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export default SchedulePane;
