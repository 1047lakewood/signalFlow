import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { cleanPath } from "./pathUtils";
import type { PlaylistInfo, TrackInfo } from "./types";
import PlaylistView from "./PlaylistView";
import type { ClipboardData } from "./PlaylistView";
import TransportBar from "./TransportBar";
import SettingsWindow from "./SettingsWindow";
import AdConfigWindow from "./AdConfigWindow";
import AdStatsWindow from "./AdStatsWindow";
import RdsConfigWindow from "./RdsConfigWindow";
import SchedulePane from "./SchedulePane";
import LogPane from "./LogPane";
import FileBrowserPane from "./FileBrowserPane";

const AUDIO_EXTENSIONS = ["mp3", "wav", "flac", "ogg", "aac", "m4a"];

function getInitialTheme(): "dark" | "light" {
  const stored = localStorage.getItem("signalflow-theme");
  if (stored === "light" || stored === "dark") return stored;
  return "dark";
}

function App() {
  const [playlists, setPlaylists] = useState<PlaylistInfo[]>([]);
  const [selectedPlaylist, setSelectedPlaylist] = useState<string | null>(null);
  const [tracks, setTracks] = useState<TrackInfo[]>([]);
  const [currentIndex, setCurrentIndex] = useState<number | null>(null);
  const [renamingTab, setRenamingTab] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [showSettings, setShowSettings] = useState(false);
  const [showAdConfig, setShowAdConfig] = useState(false);
  const [showAdStats, setShowAdStats] = useState(false);
  const [showRdsConfig, setShowRdsConfig] = useState(false);
  const [selectedIndices, setSelectedIndices] = useState<Set<number>>(
    new Set(),
  );
  const [showSchedulePane, setShowSchedulePane] = useState(false);
  const [showFileBrowser, setShowFileBrowser] = useState(true);
  const [fileSearchSeed, setFileSearchSeed] = useState("");
  const [clipboard, setClipboard] = useState<ClipboardData | null>(null);
  const [theme, setTheme] = useState<"dark" | "light">(getInitialTheme);
  const renameInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("signalflow-theme", theme);
  }, [theme]);

  useEffect(() => {
    const suppressContextMenu = (event: MouseEvent) => {
      event.preventDefault();
    };

    window.addEventListener("contextmenu", suppressContextMenu);
    return () => {
      window.removeEventListener("contextmenu", suppressContextMenu);
    };
  }, []);
  const toggleTheme = () => {
    setTheme((t) => (t === "dark" ? "light" : "dark"));
  };

  const loadPlaylists = useCallback(async () => {
    try {
      const pls = await invoke<PlaylistInfo[]>("get_playlists");
      setPlaylists(pls);
      // Auto-select active playlist, or first, or null
      const active = pls.find((p) => p.is_active);
      const target = active ?? pls[0];
      if (target && selectedPlaylist === null) {
        setSelectedPlaylist(target.name);
        // If no playlist was active on the backend, activate the auto-selected one
        if (!active) {
          await invoke("set_active_playlist", { name: target.name });
        }
      }
    } catch (e) {
      console.error("Failed to load playlists:", e);
    }
  }, [selectedPlaylist]);

  const loadTracks = useCallback(async () => {
    if (!selectedPlaylist) {
      setTracks([]);
      return;
    }
    try {
      const t = await invoke<TrackInfo[]>("get_playlist_tracks", {
        name: selectedPlaylist,
      });
      setTracks(t);
    } catch (e) {
      console.error("Failed to load tracks:", e);
      setTracks([]);
    }
  }, [selectedPlaylist]);

  useEffect(() => {
    loadPlaylists();
  }, [loadPlaylists]);

  useEffect(() => {
    loadTracks();
  }, [loadTracks]);

  useEffect(() => {
    if (renamingTab && renameInputRef.current) {
      renameInputRef.current.focus();
      renameInputRef.current.select();
    }
  }, [renamingTab]);

  const handlePlaylistSelect = async (name: string) => {
    setSelectedPlaylist(name);
    setSelectedIndices(new Set());
    try {
      await invoke("set_active_playlist", { name });
    } catch (e) {
      console.error("Failed to set active playlist:", e);
    }
  };

  const handleAddPlaylist = async () => {
    const name = prompt("New playlist name:");
    if (!name || !name.trim()) return;
    try {
      await invoke("create_playlist", { name: name.trim() });
      setSelectedPlaylist(name.trim());
      await loadPlaylists();
    } catch (e) {
      console.error("Failed to create playlist:", e);
    }
  };

  const handleClosePlaylist = async (name: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await invoke("delete_playlist", { name });
      if (selectedPlaylist === name) {
        const remaining = playlists.filter((p) => p.name !== name);
        setSelectedPlaylist(remaining[0]?.name ?? null);
      }
      await loadPlaylists();
    } catch (e) {
      console.error("Failed to delete playlist:", e);
    }
  };

  const handleRenameStart = (name: string) => {
    setRenamingTab(name);
    setRenameValue(name);
  };

  const handleRenameCommit = async () => {
    if (!renamingTab) return;
    const newName = renameValue.trim();
    if (!newName || newName === renamingTab) {
      setRenamingTab(null);
      return;
    }
    try {
      await invoke("rename_playlist", { oldName: renamingTab, newName });
      if (selectedPlaylist === renamingTab) {
        setSelectedPlaylist(newName);
      }
      setRenamingTab(null);
      await loadPlaylists();
    } catch (e) {
      console.error("Failed to rename playlist:", e);
      setRenamingTab(null);
    }
  };

  const handleRenameKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      handleRenameCommit();
    } else if (e.key === "Escape") {
      setRenamingTab(null);
    }
  };

  const handlePlayingIndexChange = useCallback((index: number | null) => {
    setCurrentIndex(index);
  }, []);

  const handlePlayTrack = useCallback(
    async (trackIndex: number) => {
      try {
        await invoke("transport_play", { trackIndex });
        setCurrentIndex(trackIndex);
        await loadTracks();
      } catch (e) {
        console.error("Failed to play track:", e);
      }
    },
    [loadTracks],
  );

  const handleCopyTracks = useCallback(
    (indices: number[]) => {
      if (!selectedPlaylist) return;
      const paths = indices
        .map((i) => tracks.find((t) => t.index === i))
        .filter((t): t is TrackInfo => t !== undefined)
        .map((t) => t.path);
      if (paths.length === 0) return;
      setClipboard({
        paths,
        sourcePlaylist: selectedPlaylist,
        sourceIndices: indices,
        isCut: false,
      });
    },
    [selectedPlaylist, tracks],
  );

  const handleCutTracks = useCallback(
    (indices: number[]) => {
      if (!selectedPlaylist) return;
      const paths = indices
        .map((i) => tracks.find((t) => t.index === i))
        .filter((t): t is TrackInfo => t !== undefined)
        .map((t) => t.path);
      if (paths.length === 0) return;
      setClipboard({
        paths,
        sourcePlaylist: selectedPlaylist,
        sourceIndices: indices,
        isCut: true,
      });
    },
    [selectedPlaylist, tracks],
  );

  const handlePasteTrack = useCallback(
    async (afterIndex: number) => {
      if (!selectedPlaylist || !clipboard) return;
      try {
        // Insert position is after the clicked row
        const insertAt = afterIndex + 1;
        if (clipboard.isCut) {
          // For cut: copy from source, paste at destination, then remove from source
          await invoke("copy_paste_tracks", {
            fromPlaylist: clipboard.sourcePlaylist,
            indices: clipboard.sourceIndices,
            toPlaylist: selectedPlaylist,
            at: insertAt,
          });
          // Remove from source (only if same playlist, adjust for insertion)
          await invoke("remove_tracks", {
            playlist: clipboard.sourcePlaylist,
            indices: clipboard.sourceIndices,
          });
          setClipboard(null); // Cut is one-time
        } else {
          // For copy: just copy and paste
          await invoke("copy_paste_tracks", {
            fromPlaylist: clipboard.sourcePlaylist,
            indices: clipboard.sourceIndices,
            toPlaylist: selectedPlaylist,
            at: insertAt,
          });
        }
        await loadTracks();
        await loadPlaylists();
      } catch (e) {
        console.error("Failed to paste tracks:", e);
      }
    },
    [selectedPlaylist, clipboard, loadTracks, loadPlaylists],
  );

  const handleReorder = useCallback(
    async (fromIndex: number, toIndex: number) => {
      if (!selectedPlaylist) return;
      try {
        await invoke("reorder_track", {
          playlist: selectedPlaylist,
          from: fromIndex,
          to: toIndex,
        });
        await loadTracks();
      } catch (e) {
        console.error("Failed to reorder track:", e);
      }
    },
    [selectedPlaylist, loadTracks],
  );

  const handleAddFiles = useCallback(async () => {
    if (!selectedPlaylist) return;
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: "Audio Files",
            extensions: AUDIO_EXTENSIONS,
          },
        ],
      });
      if (!selected) return;
      // open() returns string | string[] | null
      const raw = Array.isArray(selected) ? selected : [selected];
      const paths = raw.map(cleanPath);
      if (paths.length === 0) return;
      await invoke("add_tracks", { playlist: selectedPlaylist, paths });
      await loadTracks();
      await loadPlaylists(); // refresh track counts
    } catch (e) {
      console.error("Failed to add files:", e);
    }
  }, [selectedPlaylist, loadTracks, loadPlaylists]);

  const handleFileDrop = useCallback(
    async (paths: string[]) => {
      if (!selectedPlaylist || paths.length === 0) return;
      // Filter to audio files only
      const audioPaths = paths.filter((p) => {
        const ext = p.split(".").pop()?.toLowerCase() ?? "";
        return AUDIO_EXTENSIONS.includes(ext);
      });
      if (audioPaths.length === 0) return;
      try {
        await invoke("add_tracks", {
          playlist: selectedPlaylist,
          paths: audioPaths,
        });
        await loadTracks();
        await loadPlaylists();
      } catch (e) {
        console.error("Failed to add dropped files:", e);
      }
    },
    [selectedPlaylist, loadTracks, loadPlaylists],
  );

  const handleSearchFilename = useCallback((filename: string) => {
    setShowFileBrowser(true);
    setFileSearchSeed(filename);
  }, []);

  return (
    <div className="app">
      <header className="header">
        <h1>signalFlow</h1>
        <div className="playlist-selector">
          {playlists.map((pl) => (
            <button
              key={pl.id}
              className={
                pl.name === selectedPlaylist
                  ? "playlist-tab active"
                  : "playlist-tab"
              }
              onClick={() => handlePlaylistSelect(pl.name)}
              onDoubleClick={() => handleRenameStart(pl.name)}
            >
              {renamingTab === pl.name ? (
                <input
                  ref={renameInputRef}
                  className="rename-input"
                  value={renameValue}
                  onChange={(e) => setRenameValue(e.target.value)}
                  onBlur={handleRenameCommit}
                  onKeyDown={handleRenameKeyDown}
                  onClick={(e) => e.stopPropagation()}
                />
              ) : (
                <>
                  {pl.name}
                  <span className="track-count">{pl.track_count}</span>
                  <span
                    className="tab-close"
                    onClick={(e) => handleClosePlaylist(pl.name, e)}
                    title="Close playlist"
                  >
                    {"\u00D7"}
                  </span>
                </>
              )}
            </button>
          ))}
          <button
            className="playlist-tab add-tab"
            onClick={handleAddPlaylist}
            title="Add playlist"
          >
            +
          </button>
        </div>
      </header>
      <main className="main">
        <div
          className={`main-content ${showSchedulePane ? "with-schedule" : ""}`}
        >
          <aside className="left-sidebar">
            <button
              className={`sidebar-btn ${showFileBrowser ? "active" : ""}`}
              onClick={() => setShowFileBrowser((v) => !v)}
              title="Toggle file browser"
            >
              📂
            </button>
            <button
              className={`sidebar-btn ${showSchedulePane ? "active" : ""}`}
              onClick={() => setShowSchedulePane((v) => !v)}
              title="Toggle schedule"
            >
              ⏰
            </button>
            <button
              className="sidebar-btn"
              onClick={() => setShowAdConfig(true)}
              title="Ad Configuration"
            >
              📢
            </button>
            <button
              className="sidebar-btn"
              onClick={() => setShowAdStats(true)}
              title="Ad Statistics"
            >
              📊
            </button>
            <button
              className="sidebar-btn"
              onClick={() => setShowRdsConfig(true)}
              title="RDS Configuration"
            >
              📻
            </button>
            <button
              className="sidebar-btn"
              onClick={toggleTheme}
              title={
                theme === "dark"
                  ? "Switch to light theme"
                  : "Switch to dark theme"
              }
            >
              {theme === "dark" ? "☀" : "🌙"}
            </button>
            <button
              className="sidebar-btn"
              onClick={() => setShowSettings(true)}
              title="Options / Settings"
            >
              ⚙
            </button>
          </aside>
          {showFileBrowser && (
            <FileBrowserPane
              onAddFiles={handleFileDrop}
              onSearchFilename={handleSearchFilename}
              searchSeed={fileSearchSeed}
            />
          )}
          <div className="playlist-area">
            {selectedPlaylist ? (
              <PlaylistView
                tracks={tracks}
                currentIndex={currentIndex}
                playlistName={selectedPlaylist}
                selectedIndices={selectedIndices}
                clipboard={clipboard}
                onSelectTracks={setSelectedIndices}
                onPlayTrack={handlePlayTrack}
                onReorder={handleReorder}
                onCopyTracks={handleCopyTracks}
                onCutTracks={handleCutTracks}
                onPasteTracks={handlePasteTrack}
                onAddFiles={handleAddFiles}
                onFileDrop={handleFileDrop}
                onTracksChanged={loadTracks}
                onSearchFilename={handleSearchFilename}
              />
            ) : (
              <div className="no-playlist">
                <p>No playlists available</p>
              </div>
            )}
          </div>
          {showSchedulePane && (
            <div className="side-pane">
              <SchedulePane onClose={() => setShowSchedulePane(false)} />
              <LogPane />
            </div>
          )}
        </div>
      </main>
      <TransportBar
        onTrackChange={loadTracks}
        selectedTrackIndex={
          selectedIndices.size > 0 ? Math.min(...selectedIndices) : null
        }
        onPlayingIndexChange={handlePlayingIndexChange}
      />
      {showSettings && (
        <SettingsWindow onClose={() => setShowSettings(false)} />
      )}
      {showAdConfig && (
        <AdConfigWindow onClose={() => setShowAdConfig(false)} />
      )}
      {showAdStats && <AdStatsWindow onClose={() => setShowAdStats(false)} />}
      {showRdsConfig && (
        <RdsConfigWindow onClose={() => setShowRdsConfig(false)} />
      )}
    </div>
  );
}

export default App;
