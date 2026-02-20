import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { cleanPath } from "./pathUtils";
import type { PlaylistInfo, PlaylistProfileInfo, TrackInfo } from "./types";
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
import AudioEditorModal from "./editor/AudioEditorModal";

const AUDIO_EXTENSIONS = ["mp3", "wav", "flac", "ogg", "aac", "m4a"];

function getInitialTheme(): "dark" | "light" {
  const stored = localStorage.getItem("signalflow-theme");
  if (stored === "light" || stored === "dark") return stored;
  return "dark";
}

function App() {
  const [playlists, setPlaylists] = useState<PlaylistInfo[]>([]);
  const [selectedPlaylist, setSelectedPlaylist] = useState<string | null>(null);
  const selectedPlaylistRef = useRef<string | null>(null);
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
  const [profiles, setProfiles] = useState<PlaylistProfileInfo[]>([]);
  const [selectedProfile, setSelectedProfile] = useState<string>("");
  const [findRequestToken, setFindRequestToken] = useState(0);
  const [editorPath, setEditorPath] = useState<string | null>(null);
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

  const requestOpenFind = useCallback(() => {
    setFindRequestToken((v) => v + 1);
  }, []);

  const loadProfiles = useCallback(async () => {
    try {
      const rows = await invoke<PlaylistProfileInfo[]>("get_playlist_profiles");
      setProfiles(rows);
      setSelectedProfile((prev) => {
        if (!rows.length) return "";
        if (prev && rows.some((p) => p.name === prev)) return prev;
        return rows[0].name;
      });
    } catch (e) {
      console.error("Failed to load playlist profiles:", e);
    }
  }, []);

  const handleSaveProfile = useCallback(async () => {
    const name = prompt("Profile name:", selectedProfile || "");
    if (!name || !name.trim()) return;
    try {
      await invoke("save_playlist_profile", { name: name.trim() });
      await loadProfiles();
      setSelectedProfile(name.trim());
    } catch (e) {
      console.error("Failed to save profile:", e);
    }
  }, [loadProfiles, selectedProfile]);

  const handleDeleteProfile = useCallback(async () => {
    if (!selectedProfile) return;
    if (!confirm(`Delete profile "${selectedProfile}"?`)) return;
    try {
      await invoke("delete_playlist_profile", { name: selectedProfile });
      await loadProfiles();
    } catch (e) {
      console.error("Failed to delete profile:", e);
    }
  }, [selectedProfile, loadProfiles]);

  // Keep ref in sync so loadPlaylists can read the current value without being
  // listed as a dependency (avoiding extra fetches on every tab switch).
  useEffect(() => {
    selectedPlaylistRef.current = selectedPlaylist;
  }, [selectedPlaylist]);

  const loadPlaylists = useCallback(async () => {
    try {
      const pls = await invoke<PlaylistInfo[]>("get_playlists");
      setPlaylists(pls);
      // Auto-select active playlist, or first, or null
      const active = pls.find((p) => p.is_active);
      const target = active ?? pls[0];
      const current = selectedPlaylistRef.current;
      const hasCurrentSelection =
        current !== null &&
        pls.some((playlist) => playlist.name === current);
      if (target && !hasCurrentSelection) {
        setSelectedPlaylist(target.name);
        // If no playlist was active on the backend, activate the auto-selected one
        if (!active) {
          await invoke("set_active_playlist", { name: target.name });
        }
      }
      if (!target) {
        setSelectedPlaylist(null);
      }
    } catch (e) {
      console.error("Failed to load playlists:", e);
    }
  }, []);

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
    loadProfiles();
  }, [loadPlaylists, loadProfiles]);

  useEffect(() => {
    loadTracks();
  }, [loadTracks]);

  useEffect(() => {
    if (renamingTab && renameInputRef.current) {
      renameInputRef.current.focus();
      renameInputRef.current.select();
    }
  }, [renamingTab]);

  const handleLoadProfile = useCallback(async () => {
    if (!selectedProfile) return;
    try {
      await invoke("load_playlist_profile", { name: selectedProfile });
      setSelectedIndices(new Set());
      setSelectedPlaylist(null);
      await loadPlaylists();
    } catch (e) {
      console.error("Failed to load profile:", e);
    }
  }, [selectedProfile, loadPlaylists]);

  useEffect(() => {
    const onGlobalKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
        event.preventDefault();
        requestOpenFind();
      }
    };

    window.addEventListener("keydown", onGlobalKeyDown, true);
    return () => window.removeEventListener("keydown", onGlobalKeyDown, true);
  }, [requestOpenFind]);

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
          const count = clipboard.sourceIndices.length;
          // For cut: copy from source, paste at destination, then remove from source
          await invoke("copy_paste_tracks", {
            fromPlaylist: clipboard.sourcePlaylist,
            indices: clipboard.sourceIndices,
            toPlaylist: selectedPlaylist,
            at: insertAt,
          });
          // When cutting within the same playlist, insertion shifts all indices
          // at or after insertAt by `count` positions — adjust before removing.
          const removeIndices =
            clipboard.sourcePlaylist === selectedPlaylist
              ? clipboard.sourceIndices.map((i) =>
                  i >= insertAt ? i + count : i,
                )
              : clipboard.sourceIndices;
          await invoke("remove_tracks", {
            playlist: clipboard.sourcePlaylist,
            indices: removeIndices,
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
      const audioPaths = paths
        .map(cleanPath)
        .filter((p) => {
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
    setFileSearchSeed(filename.replace(/\.[^.]+$/, ""));
  }, []);

  const handleOpenPlaylistFile = useCallback(async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Playlist Files", extensions: ["m3u", "m3u8"] }],
      });
      if (!selected || Array.isArray(selected)) return;
      const importedName = await invoke<string>("import_m3u_playlist", {
        filePath: cleanPath(selected),
      });
      await loadPlaylists();
      setSelectedPlaylist(importedName);
    } catch (e) {
      console.error("Failed to open playlist file:", e);
    }
  }, [loadPlaylists]);

  const handleSavePlaylistFile = useCallback(async () => {
    if (!selectedPlaylist) return;
    try {
      await invoke<string>("export_playlist_to_m3u", {
        playlistName: selectedPlaylist,
        filePath: null,
      });
      await loadPlaylists();
    } catch (e) {
      console.error("Failed to save playlist file:", e);
    }
  }, [selectedPlaylist, loadPlaylists]);

  const handleSavePlaylistFileAs = useCallback(async () => {
    if (!selectedPlaylist) return;
    try {
      const target = await save({
        filters: [{ name: "Playlist Files", extensions: ["m3u", "m3u8"] }],
        defaultPath: `${selectedPlaylist}.m3u8`,
      });
      if (!target) return;
      await invoke<string>("export_playlist_to_m3u", {
        playlistName: selectedPlaylist,
        filePath: cleanPath(target),
      });
      await loadPlaylists();
    } catch (e) {
      console.error("Failed to save playlist as:", e);
    }
  }, [selectedPlaylist, loadPlaylists]);

  return (
    <div className="app">
      <header className="header">
        <h1>signalFlow</h1>
        <nav className="menu-bar" aria-label="Main menu">
          <button className="menu-btn" onClick={handleOpenPlaylistFile}>Open Playlist…</button>
          <button className="menu-btn" onClick={handleSavePlaylistFile} disabled={!selectedPlaylist}>Save Playlist</button>
          <button className="menu-btn" onClick={handleSavePlaylistFileAs} disabled={!selectedPlaylist}>Save Playlist As…</button>
        </nav>
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
        <div className="header-actions">
          <select
            className="profile-select"
            value={selectedProfile}
            onChange={(e) => setSelectedProfile(e.target.value)}
            title="Saved profiles"
          >
            <option value="">Profiles</option>
            {profiles.map((profile) => (
              <option key={profile.name} value={profile.name}>
                {profile.name}
              </option>
            ))}
          </select>
          <button className="header-btn" onClick={handleSaveProfile} title="Save profile">
            Save Profile
          </button>
          <button
            className="header-btn"
            onClick={handleLoadProfile}
            disabled={!selectedProfile}
            title="Load profile"
          >
            Load
          </button>
          <button
            className="header-btn"
            onClick={handleDeleteProfile}
            disabled={!selectedProfile}
            title="Delete selected profile"
          >
            Delete
          </button>
          <button className="header-btn" onClick={toggleTheme} title={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}>
            {theme === "dark" ? "☀" : "🌙"}
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
              <span className="sidebar-icon">📂</span><span className="sidebar-label">Files</span>
            </button>
            <button
              className={`sidebar-btn ${showSchedulePane ? "active" : ""}`}
              onClick={() => setShowSchedulePane((v) => !v)}
              title="Toggle schedule"
            >
              <span className="sidebar-icon">⏰</span><span className="sidebar-label">Schedule</span>
            </button>
            <button
              className="sidebar-btn"
              onClick={() => setShowAdConfig(true)}
              title="Ad Configuration"
            >
              <span className="sidebar-icon">📢</span><span className="sidebar-label">Ads</span>
            </button>
            <button
              className="sidebar-btn"
              onClick={() => setShowAdStats(true)}
              title="Ad Statistics"
            >
              <span className="sidebar-icon">📊</span><span className="sidebar-label">Stats</span>
            </button>
            <button
              className="sidebar-btn"
              onClick={() => setShowRdsConfig(true)}
              title="RDS Configuration"
            >
              <span className="sidebar-icon">📻</span><span className="sidebar-label">RDS</span>
            </button>
            <button
              className="sidebar-btn"
              onClick={requestOpenFind}
              title="Find in playlist (Ctrl+F)"
            >
              <span className="sidebar-icon">🔍</span><span className="sidebar-label">Find</span>
            </button>
            <button
              className="sidebar-btn"
              onClick={() => setShowSettings(true)}
              title="Options / Settings"
            >
              <span className="sidebar-icon">⚙</span><span className="sidebar-label">Settings</span>
            </button>
          </aside>
          {showFileBrowser && (
            <FileBrowserPane
              onAddFiles={handleFileDrop}
              onSearchFilename={handleSearchFilename}
              searchSeed={fileSearchSeed}
              onEditAudio={setEditorPath}
            />
          )}
          {showSchedulePane && (
            <div className="side-pane side-pane-left">
              <SchedulePane onClose={() => setShowSchedulePane(false)} />
              <LogPane />
            </div>
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
                findRequestToken={findRequestToken}
                onEditAudio={setEditorPath}
              />
            ) : (
              <div className="no-playlist">
                <p>No playlists available</p>
              </div>
            )}
          </div>
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
      {editorPath && (
        <AudioEditorModal
          path={editorPath}
          onClose={() => setEditorPath(null)}
        />
      )}
    </div>
  );
}

export default App;
