import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  ConfigResponse,
  FileBrowserEntry,
  FileSearchResult,
} from "./types";

interface FileBrowserPaneProps {
  onAddFiles: (paths: string[]) => void;
  onSearchFilename: (filename: string) => void;
  searchSeed: string;
}

function FileBrowserPane({
  onAddFiles,
  onSearchFilename,
  searchSeed,
}: FileBrowserPaneProps) {
  const [indexedLocations, setIndexedLocations] = useState<string[]>([]);
  const [favoriteFolders, setFavoriteFolders] = useState<string[]>([]);
  const [currentPath, setCurrentPath] = useState<string | null>(null);
  const [entries, setEntries] = useState<FileBrowserEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<FileSearchResult[]>([]);
  const [favoritesCollapsed, setFavoritesCollapsed] = useState(true);
  const normalizedQuery = searchQuery.trim();

  const loadConfig = useCallback(async () => {
    const cfg = await invoke<ConfigResponse>("get_config");
    setIndexedLocations(cfg.indexed_locations);
    setFavoriteFolders(cfg.favorite_folders);
    setCurrentPath((prev) => prev ?? cfg.indexed_locations[0] ?? null);
  }, []);

  const loadDirectory = useCallback(async (path: string | null) => {
    const rows = await invoke<FileBrowserEntry[]>("list_directory", { path });
    setEntries(rows);
  }, []);

  useEffect(() => {
    loadConfig().catch((e) =>
      console.error("Failed to load file browser config", e),
    );
  }, [loadConfig]);

  useEffect(() => {
    loadDirectory(currentPath).catch((e) =>
      console.error("Failed to load directory", e),
    );
  }, [currentPath, loadDirectory]);

  useEffect(() => {
    if (searchSeed) {
      setSearchQuery(searchSeed);
    }
  }, [searchSeed]);

  useEffect(() => {
    const run = async () => {
      const q = normalizedQuery;
      if (q.length < 2) {
        setSearchResults([]);
        return;
      }
      const rows = await invoke<FileSearchResult[]>("search_indexed_files", {
        query: q,
      });
      setSearchResults(rows);
    };
    const id = setTimeout(
      () => run().catch((e) => console.error("Search failed", e)),
      280,
    );
    return () => clearTimeout(id);
  }, [normalizedQuery]);

  const visible = useMemo(() => {
    if (normalizedQuery.length >= 2) {
      return searchResults.map((r) => ({
        path: r.path,
        name: r.name,
        is_dir: false,
      }));
    }
    return entries;
  }, [entries, normalizedQuery, searchResults]);

  return (
    <aside className="file-browser-pane">
      <div
        className={`favorites-pane ${favoritesCollapsed ? "collapsed" : ""}`}
        onMouseLeave={() => setFavoritesCollapsed(true)}
      >
        <div className="favorites-title">★</div>
        {favoriteFolders.map((folder) => (
          <button
            key={folder}
            className="favorite-item"
            title={folder}
            onMouseEnter={() => setFavoritesCollapsed(false)}
            onClick={() => setCurrentPath(folder)}
          >
            <span className="favorite-icon">📁</span>
            {!favoritesCollapsed && (
              <span className="favorite-label">{folder}</span>
            )}
          </button>
        ))}
      </div>
      <div className="file-browser-content">
        <div className="file-browser-toolbar">
          <select
            className="file-browser-select"
            value={currentPath ?? ""}
            onChange={(e) => setCurrentPath(e.target.value || null)}
          >
            <option value="">Current directory</option>
            {indexedLocations.map((loc) => (
              <option key={loc} value={loc}>
                {loc}
              </option>
            ))}
          </select>
          <input
            className="file-browser-search"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search indexed files (2+ chars)..."
          />
        </div>
        <div className="file-browser-list">
          {visible.map((entry) => (
            <div
              key={entry.path}
              className="file-row"
              onDoubleClick={() =>
                entry.is_dir
                  ? setCurrentPath(entry.path)
                  : onAddFiles([entry.path])
              }
            >
              <button
                className="file-row-main"
                onClick={() =>
                  entry.is_dir ? setCurrentPath(entry.path) : undefined
                }
              >
                <span>{entry.is_dir ? "📁" : "🎵"}</span>
                <span title={entry.path}>{entry.name}</span>
              </button>
              {!entry.is_dir && (
                <>
                  <button
                    className="file-row-action"
                    onClick={() => onAddFiles([entry.path])}
                  >
                    +
                  </button>
                  <button
                    className="file-row-action"
                    onClick={() => onSearchFilename(entry.name)}
                  >
                    🔎
                  </button>
                </>
              )}
            </div>
          ))}
        </div>
      </div>
    </aside>
  );
}

export default FileBrowserPane;
