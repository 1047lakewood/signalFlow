import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  ConfigResponse,
  FileBrowserEntry,
  FileSearchResult,
} from "./types";

function formatDriveLabel(path: string): string {
  const drive = path.match(/^([A-Za-z]):[/\\]?/);
  return drive ? `${drive[1].toUpperCase()}:\\` : path;
}

function normalizeFavoritePath(path: string): string {
  return path.trim().toLowerCase();
}

function getParentPath(path: string): string | null {
  const normalized = path.replace(/[\\/]+$/, "");
  if (!normalized) return null;

  const windowsDriveRoot = normalized.match(/^[A-Za-z]:$/);
  if (windowsDriveRoot) return null;
  if (normalized === "/") return null;

  const uncRoot = normalized.match(/^\\\\[^\\]+\\[^\\]+$/);
  if (uncRoot) return null;

  const next = normalized.replace(/[\\/][^\\/]+$/, "");
  if (!next || next === normalized) return null;
  return next;
}

function compareEntries(a: FileBrowserEntry, b: FileBrowserEntry): number {
  if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
  return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
}

function favoriteDriveKey(path: string): string {
  return `drive:${path}`;
}

const AUDIO_EXTENSIONS = new Set(["mp3", "wav", "flac", "ogg", "aac", "m4a"]);

interface FileBrowserPaneProps {
  onAddFiles: (paths: string[]) => void;
  onSearchFilename: (filename: string) => void;
  searchSeed: string;
  onEditAudio?: (path: string) => void;
}

function FileBrowserPane({
  onAddFiles,
  onSearchFilename,
  searchSeed,
  onEditAudio,
}: FileBrowserPaneProps) {
  const [indexedLocations, setIndexedLocations] = useState<string[]>([]);
  const [availableDrives, setAvailableDrives] = useState<string[]>([]);
  const [favoriteFolders, setFavoriteFolders] = useState<string[]>([]);
  const [currentPath, setCurrentPath] = useState<string | null>(null);
  const [entries, setEntries] = useState<FileBrowserEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<FileSearchResult[]>([]);
  const [favoritesCollapsed, setFavoritesCollapsed] = useState(true);
  const [favoritesDropActive, setFavoritesDropActive] = useState(false);
  const [isLoadingDirectory, setIsLoadingDirectory] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [searchError, setSearchError] = useState<string | null>(null);
  const normalizedQuery = searchQuery.trim();
  const atRootLocation =
    currentPath !== null &&
    (indexedLocations.some((loc) => loc === currentPath) ||
      availableDrives.some(
        (d) =>
          d.replace(/[\\/]+$/, "") === currentPath.replace(/[\\/]+$/, ""),
      ));

  const loadConfig = useCallback(async () => {
    const cfg = await invoke<ConfigResponse>("get_config");
    setIndexedLocations(cfg.indexed_locations);
    setFavoriteFolders(cfg.favorite_folders);
    setCurrentPath((prev) => prev ?? cfg.indexed_locations[0] ?? null);
  }, []);

  const loadDirectory = useCallback(async (path: string | null) => {
    setIsLoadingDirectory(true);
    setLoadError(null);
    try {
      const rows = await invoke<FileBrowserEntry[]>("list_directory", { path });
      const sorted = [...rows].sort(compareEntries);
      setEntries(sorted);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setLoadError(msg || "Failed to load directory");
      setEntries([]);
    } finally {
      setIsLoadingDirectory(false);
    }
  }, []);

  useEffect(() => {
    loadConfig().catch((e) =>
      console.error("Failed to load file browser config", e),
    );
    invoke<string[]>("list_available_drives")
      .then(setAvailableDrives)
      .catch((e) => console.error("Failed to list drives", e));
  }, [loadConfig]);

  // Only reset currentPath when indexed locations themselves change (e.g. user
  // removes a location from settings). Do NOT depend on currentPath so that
  // navigating to non-indexed paths doesn't immediately snap back.
  useEffect(() => {
    if (indexedLocations.length === 0) return;
    setCurrentPath((prev) => {
      if (!prev) return indexedLocations[0] ?? null;
      if (indexedLocations.includes(prev)) return prev;
      if (favoriteFolders.some((f) => prev.startsWith(f))) return prev;
      return indexedLocations[0] ?? null;
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [indexedLocations]);

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
    let cancelled = false;
    const run = async () => {
      const q = normalizedQuery;
      if (q.length < 2) {
        setSearchResults([]);
        setSearchError(null);
        return;
      }
      try {
        const rows = await invoke<FileSearchResult[]>("search_indexed_files", {
          query: q,
        });
        if (!cancelled) {
          const sorted = [...rows].sort((a, b) =>
            a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
          );
          setSearchResults(sorted);
          setSearchError(null);
        }
      } catch (e) {
        if (cancelled) return;
        const msg = e instanceof Error ? e.message : String(e);
        setSearchResults([]);
        setSearchError(msg || "Search failed");
      }
    };
    const id = setTimeout(
      () => run().catch((e) => console.error("Search failed", e)),
      280,
    );
    return () => {
      clearTimeout(id);
      cancelled = true;
    };
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

  const breadcrumbs = useMemo(() => {
    if (!currentPath) return [] as Array<{ label: string; path: string }>;

    const windowsDrive = currentPath.match(/^([A-Za-z]:)([\\/].*)?$/);
    if (windowsDrive) {
      const drive = windowsDrive[1];
      const rest = (windowsDrive[2] ?? "").split(/[\\/]+/).filter(Boolean);
      const crumbs = [{ label: `${drive}\\`, path: drive }];
      let acc = drive;
      for (const part of rest) {
        acc = `${acc}\\${part}`;
        crumbs.push({ label: part, path: acc });
      }
      return crumbs;
    }

    const parts = currentPath.split(/[\\/]+/).filter(Boolean);
    if (currentPath.startsWith("/")) {
      const crumbs = [{ label: "/", path: "/" }];
      let acc = "";
      for (const part of parts) {
        acc = `${acc}/${part}`;
        crumbs.push({ label: part, path: acc || "/" });
      }
      return crumbs;
    }

    let acc = "";
    return parts.map((part) => {
      acc = acc ? `${acc}/${part}` : part;
      return { label: part, path: acc };
    });
  }, [currentPath]);

  const persistFavorites = useCallback(async (nextFolders: string[]) => {
    await invoke("set_favorite_folders", { folders: nextFolders });
    setFavoriteFolders(nextFolders);
  }, []);

  const handleFavoriteDrop = useCallback(
    async (event: React.DragEvent<HTMLDivElement>) => {
      event.preventDefault();
      setFavoritesDropActive(false);
      const droppedPath = event.dataTransfer.getData("text/signalflow-folder-path");
      if (!droppedPath) return;
      const normalized = droppedPath.trim();
      if (!normalized) return;
      const normalizedTarget = normalizeFavoritePath(normalized);
      const alreadyFavorite = favoriteFolders.some(
        (folder) => normalizeFavoritePath(folder) === normalizedTarget,
      );
      if (alreadyFavorite) return;
      const nextFolders = [...favoriteFolders, normalized];
      try {
        await persistFavorites(nextFolders);
      } catch (e) {
        console.error("Failed to add favorite folder", e);
      }
    },
    [favoriteFolders, persistFavorites],
  );

  const handleRemoveFavorite = useCallback(
    async (folderToRemove: string) => {
      const nextFolders = favoriteFolders.filter(
        (folder) => folder !== folderToRemove,
      );
      try {
        await persistFavorites(nextFolders);
      } catch (e) {
        console.error("Failed to remove favorite folder", e);
      }
    },
    [favoriteFolders, persistFavorites],
  );

  return (
    <aside className="file-browser-pane">
      <div
        className={`favorites-pane ${favoritesCollapsed ? "collapsed" : ""} ${favoritesDropActive ? "drop-active" : ""}`}
        onMouseLeave={() => setFavoritesCollapsed(true)}
        onFocus={() => setFavoritesCollapsed(false)}
        onDragOver={(event) => {
          event.preventDefault();
          setFavoritesCollapsed(false);
          setFavoritesDropActive(true);
        }}
        onDragLeave={() => setFavoritesDropActive(false)}
        onDrop={handleFavoriteDrop}
      >
        <div className="favorites-title">★</div>
        <div className="favorites-section-divider" />
        {availableDrives.map((loc) => (
          <div
            key={favoriteDriveKey(loc)}
            className="favorite-item"
            title={`${loc}${indexedLocations.includes(loc) ? " (indexed)" : ""}`}
            onMouseEnter={() => setFavoritesCollapsed(false)}
          >
            <button
              className={`favorite-open-btn ${currentPath?.toUpperCase().startsWith(loc.toUpperCase()) ? "active" : ""}`}
              onClick={() => setCurrentPath(loc)}
            >
              <span className="favorite-icon">
                {indexedLocations.includes(loc) ? "💽" : "💿"}
              </span>
              {!favoritesCollapsed && (
                <span className="favorite-label">{formatDriveLabel(loc)}</span>
              )}
            </button>
          </div>
        ))}
        <div className="favorites-section-divider" />
        {favoriteFolders.map((folder) => (
          <div
            key={folder}
            className="favorite-item"
            title={folder}
            onMouseEnter={() => setFavoritesCollapsed(false)}
          >
            <button
              className={`favorite-open-btn ${currentPath === folder ? "active" : ""}`}
              onClick={() => setCurrentPath(folder)}
            >
              <span className="favorite-icon">📁</span>
              {!favoritesCollapsed && (
                <span className="favorite-label">{folder}</span>
              )}
            </button>
            {!favoritesCollapsed && (
              <button
                className="favorite-remove-btn"
                onClick={(event) => {
                  event.stopPropagation();
                  handleRemoveFavorite(folder).catch(() => undefined);
                }}
                aria-label={`Remove ${folder} from favorites`}
                title="Remove favorite"
              >
                ×
              </button>
            )}
          </div>
        ))}
      </div>
      <div className="file-browser-content">
        <div className="file-browser-toolbar">
          <div className="file-browser-drives">
            {availableDrives.map((loc) => (
              <button
                key={loc}
                className={`drive-btn ${currentPath?.toUpperCase().startsWith(loc.toUpperCase()) ? "active" : ""}`}
                title={`${loc}${indexedLocations.includes(loc) ? " (indexed)" : ""}`}
                onClick={() => setCurrentPath(loc)}
              >
                {formatDriveLabel(loc)}
              </button>
            ))}
            <button
              className="drive-btn"
              title="Parent folder"
              onClick={() => {
                if (atRootLocation || !currentPath) return;
                setCurrentPath(getParentPath(currentPath));
              }}
              disabled={atRootLocation || !currentPath || !getParentPath(currentPath)}
            >
              ↑
            </button>
            <button
              className="drive-btn"
              title="Refresh folder"
              onClick={() => loadDirectory(currentPath)}
              disabled={isLoadingDirectory}
            >
              ↻
            </button>
          </div>
          {breadcrumbs.length > 0 && (
            <div className="file-browser-breadcrumbs" aria-label="Current folder breadcrumbs">
              {breadcrumbs.map((crumb, index) => (
                <button
                  key={crumb.path}
                  className={`breadcrumb-btn ${index === breadcrumbs.length - 1 ? "active" : ""}`}
                  onClick={() => setCurrentPath(crumb.path)}
                  title={crumb.path}
                >
                  {crumb.label}
                </button>
              ))}
            </div>
          )}
          <div className="file-browser-search-row">
            <input
              className="file-browser-search"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Escape") {
                  setSearchQuery("");
                }
                if (event.key === "Enter" && normalizedQuery.length >= 2) {
                  const top = visible[0];
                  if (top && !top.is_dir) {
                    onAddFiles([top.path]);
                  }
                }
              }}
              placeholder="Search indexed files (2+ chars)..."
            />
            {searchQuery.length > 0 && (
              <button
                className="search-clear-btn"
                onClick={() => setSearchQuery("")}
                title="Clear search"
                aria-label="Clear search"
              >
                ×
              </button>
            )}
          </div>
          {normalizedQuery.length >= 2 && (
            <div className="file-browser-search-status">
              {searchError
                ? `Search failed: ${searchError}`
                : `${searchResults.length} result${searchResults.length === 1 ? "" : "s"}`}
            </div>
          )}
        </div>
        <div className="file-browser-list">
          {loadError ? (
            <div className="file-browser-empty-state">
              <div>Failed to load folder: {loadError}</div>
              <button className="drive-btn" onClick={() => loadDirectory(currentPath)}>
                Retry
              </button>
            </div>
          ) : isLoadingDirectory ? (
            <div className="file-browser-empty-state">Loading…</div>
          ) : visible.length === 0 ? (
            <div className="file-browser-empty-state">
              {normalizedQuery.length >= 2
                ? "No files match this search"
                : "This folder is empty"}
            </div>
          ) : (
            visible.map((entry) => (
              <div
                key={entry.path}
                className="file-row"
                draggable={entry.is_dir}
                onDragStart={(event) => {
                  if (!entry.is_dir) return;
                  event.dataTransfer.setData("text/signalflow-folder-path", entry.path);
                  event.dataTransfer.effectAllowed = "copy";
                }}
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
                    {onEditAudio && AUDIO_EXTENSIONS.has(entry.name.split(".").pop()?.toLowerCase() ?? "") && (
                      <button
                        className="file-row-action"
                        onClick={() => onEditAudio(entry.path)}
                        title="Edit audio"
                      >
                        ✎
                      </button>
                    )}
                    <button
                      className="file-row-action"
                      onClick={() => onSearchFilename(entry.name.replace(/\.[^.]+$/, ""))}
                    >
                      🔎
                    </button>
                  </>
                )}
              </div>
            ))
          )}
        </div>
      </div>
    </aside>
  );
}

export default FileBrowserPane;
