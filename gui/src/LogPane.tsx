import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { LogEntry } from "./types";

function LogPane() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const listRef = useRef<HTMLDivElement>(null);
  const autoScroll = useRef(true);

  const loadLogs = useCallback(async () => {
    try {
      const entries = await invoke<LogEntry[]>("get_logs", {});
      setLogs(entries);
    } catch (e) {
      console.error("Failed to load logs:", e);
    }
  }, []);

  useEffect(() => {
    loadLogs();
    const interval = setInterval(loadLogs, 1000);
    return () => clearInterval(interval);
  }, [loadLogs]);

  useEffect(() => {
    if (autoScroll.current && listRef.current) {
      listRef.current.scrollTop = listRef.current.scrollHeight;
    }
  }, [logs]);

  const handleScroll = () => {
    if (!listRef.current) return;
    const el = listRef.current;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 30;
    autoScroll.current = atBottom;
  };

  const handleClear = async () => {
    try {
      await invoke("clear_logs");
      setLogs([]);
    } catch (e) {
      console.error("Failed to clear logs:", e);
    }
  };

  const levelClass = (level: string) => {
    switch (level) {
      case "error": return "log-level-error";
      case "warn": return "log-level-warn";
      default: return "log-level-info";
    }
  };

  return (
    <div className="log-pane">
      <div className="log-pane-header">
        <h3>Log</h3>
        <button className="log-clear-btn" onClick={handleClear} title="Clear logs">
          Clear
        </button>
      </div>
      <div className="log-list" ref={listRef} onScroll={handleScroll}>
        {logs.length === 0 ? (
          <div className="log-empty">No log entries</div>
        ) : (
          logs.map((entry, i) => (
            <div key={i} className="log-entry">
              <span className="log-timestamp">{entry.timestamp}</span>
              <span className={`log-level ${levelClass(entry.level)}`}>{entry.level.toUpperCase()}</span>
              <span className="log-message">{entry.message}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export default LogPane;
