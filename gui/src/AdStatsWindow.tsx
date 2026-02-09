import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { AdStatsResponse, AdDailyCount, AdFailure } from "./types";

interface AdStatsWindowProps {
  onClose: () => void;
}

type SortField = "name" | "play_count";
type SortDir = "asc" | "desc";
type Tab = "stats" | "failures";

function AdStatsWindow({ onClose }: AdStatsWindowProps) {
  const [tab, setTab] = useState<Tab>("stats");
  const [stats, setStats] = useState<AdStatsResponse | null>(null);
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const [sortField, setSortField] = useState<SortField>("play_count");
  const [sortDir, setSortDir] = useState<SortDir>("desc");
  const [expandedAd, setExpandedAd] = useState<string | null>(null);
  const [dailyCounts, setDailyCounts] = useState<AdDailyCount[]>([]);
  const [failures, setFailures] = useState<AdFailure[]>([]);
  const [exporting, setExporting] = useState(false);
  const [exportMsg, setExportMsg] = useState("");

  const loadStats = useCallback(async () => {
    try {
      const args: { start?: string; end?: string } = {};
      if (dateFrom) args.start = dateFrom;
      if (dateTo) args.end = dateTo;
      const result = await invoke<AdStatsResponse>("get_ad_stats", args);
      setStats(result);
    } catch (e) {
      console.error("Failed to load ad stats:", e);
    }
  }, [dateFrom, dateTo]);

  const loadFailures = useCallback(async () => {
    try {
      const result = await invoke<AdFailure[]>("get_ad_failures");
      setFailures(result);
    } catch (e) {
      console.error("Failed to load ad failures:", e);
    }
  }, []);

  useEffect(() => {
    loadStats();
  }, [loadStats]);

  useEffect(() => {
    if (tab === "failures") loadFailures();
  }, [tab, loadFailures]);

  const handleExpandAd = async (adName: string) => {
    if (expandedAd === adName) {
      setExpandedAd(null);
      setDailyCounts([]);
      return;
    }
    setExpandedAd(adName);
    try {
      const counts = await invoke<AdDailyCount[]>("get_ad_daily_counts", { adName });
      setDailyCounts(counts);
    } catch (e) {
      console.error("Failed to load daily counts:", e);
      setDailyCounts([]);
    }
  };

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortField(field);
      setSortDir(field === "play_count" ? "desc" : "asc");
    }
  };

  const sortedAds = stats
    ? [...stats.per_ad].sort((a, b) => {
        const mul = sortDir === "asc" ? 1 : -1;
        if (sortField === "name") return mul * a.name.localeCompare(b.name);
        return mul * (a.play_count - b.play_count);
      })
    : [];

  const handleExport = async () => {
    if (!dateFrom || !dateTo) {
      setExportMsg("Set date range first");
      return;
    }
    try {
      const dir = await open({ directory: true, multiple: false });
      if (!dir || typeof dir !== "string") return;
      setExporting(true);
      setExportMsg("");
      const files = await invoke<string[]>("generate_ad_report", {
        start: dateFrom,
        end: dateTo,
        outputDir: dir,
      });
      if (files.length === 0) {
        setExportMsg("No plays found in range");
      } else {
        setExportMsg(`Generated ${files.length} file(s)`);
      }
    } catch (e) {
      console.error("Report generation failed:", e);
      setExportMsg("Export failed");
    } finally {
      setExporting(false);
    }
  };

  const sortIndicator = (field: SortField) => {
    if (sortField !== field) return "";
    return sortDir === "asc" ? " \u25B2" : " \u25BC";
  };

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="ad-stats-window" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>Ad Statistics</h2>
          <button className="settings-close" onClick={onClose}>{"\u00D7"}</button>
        </div>

        <div className="ad-stats-tabs">
          <button
            className={`ad-stats-tab ${tab === "stats" ? "active" : ""}`}
            onClick={() => setTab("stats")}
          >
            Play Stats
          </button>
          <button
            className={`ad-stats-tab ${tab === "failures" ? "active" : ""}`}
            onClick={() => setTab("failures")}
          >
            Failures
          </button>
        </div>

        <div className="ad-stats-body">
          {tab === "stats" && (
            <>
              <div className="ad-stats-filters">
                <label className="settings-label">Date Range (MM-DD-YY)</label>
                <div className="ad-stats-date-row">
                  <input
                    type="text"
                    className="settings-input ad-stats-date"
                    placeholder="From"
                    value={dateFrom}
                    onChange={(e) => setDateFrom(e.target.value)}
                  />
                  <span className="ad-stats-date-sep">to</span>
                  <input
                    type="text"
                    className="settings-input ad-stats-date"
                    placeholder="To"
                    value={dateTo}
                    onChange={(e) => setDateTo(e.target.value)}
                  />
                  <button className="settings-btn" onClick={loadStats}>Filter</button>
                </div>
              </div>

              {stats && (
                <div className="ad-stats-summary">
                  <span className="ad-stats-total">Total plays: <strong>{stats.total_plays}</strong></span>
                  <span className="ad-stats-count">{stats.per_ad.length} ad(s)</span>
                </div>
              )}

              <div className="ad-stats-table-wrap">
                <table className="ad-stats-table">
                  <thead>
                    <tr>
                      <th className="ad-stats-th clickable" onClick={() => handleSort("name")}>
                        Ad Name{sortIndicator("name")}
                      </th>
                      <th className="ad-stats-th clickable" onClick={() => handleSort("play_count")}>
                        Plays{sortIndicator("play_count")}
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {sortedAds.length === 0 && (
                      <tr>
                        <td colSpan={2} className="ad-stats-empty">No play data</td>
                      </tr>
                    )}
                    {sortedAds.map((ad) => (
                      <>
                        <tr
                          key={ad.name}
                          className={`ad-stats-row ${expandedAd === ad.name ? "expanded" : ""}`}
                          onClick={() => handleExpandAd(ad.name)}
                        >
                          <td className="ad-stats-td">
                            <span className="ad-stats-expand">{expandedAd === ad.name ? "\u25BC" : "\u25B6"}</span>
                            {ad.name}
                          </td>
                          <td className="ad-stats-td ad-stats-num">{ad.play_count}</td>
                        </tr>
                        {expandedAd === ad.name && dailyCounts.length > 0 && (
                          dailyCounts.map((dc) => (
                            <tr key={`${ad.name}-${dc.date}`} className="ad-stats-detail-row">
                              <td className="ad-stats-td ad-stats-detail-date">{dc.date}</td>
                              <td className="ad-stats-td ad-stats-num">{dc.count}</td>
                            </tr>
                          ))
                        )}
                        {expandedAd === ad.name && dailyCounts.length === 0 && (
                          <tr className="ad-stats-detail-row">
                            <td colSpan={2} className="ad-stats-td ad-stats-detail-date">No daily data</td>
                          </tr>
                        )}
                      </>
                    ))}
                  </tbody>
                </table>
              </div>

              <div className="ad-stats-export">
                <button
                  className="settings-btn"
                  onClick={handleExport}
                  disabled={exporting}
                >
                  {exporting ? "Generating..." : "Export Reports (CSV + PDF)"}
                </button>
                {exportMsg && <span className="ad-stats-export-msg">{exportMsg}</span>}
              </div>
            </>
          )}

          {tab === "failures" && (
            <div className="ad-stats-failures">
              {failures.length === 0 && (
                <div className="ad-stats-empty">No failures recorded</div>
              )}
              {failures.slice().reverse().map((f, i) => (
                <div key={i} className="ad-failure-item">
                  <span className="ad-failure-time">{f.timestamp}</span>
                  <span className="ad-failure-ads">{f.ads.join(", ")}</span>
                  <span className="ad-failure-err">{f.error}</span>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="settings-footer">
          <button className="settings-btn" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}

export default AdStatsWindow;
