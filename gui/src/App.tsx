import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { PlaylistInfo, TrackInfo } from "./types";
import PlaylistView from "./PlaylistView";
import TransportBar from "./TransportBar";
import CrossfadeSettings from "./CrossfadeSettings";
import SilenceSettings from "./SilenceSettings";
import IntroSettings from "./IntroSettings";
import SchedulePane from "./SchedulePane";
import LogPane from "./LogPane";

const AUDIO_EXTENSIONS = ["mp3", "wav", "flac", "ogg", "aac", "m4a"];

function App() {
  const [playlists, setPlaylists] = useState<PlaylistInfo[]>([]);
  const [selectedPlaylist, setSelectedPlaylist] = useState<string | null>(null);
  const [tracks, setTracks] = useState<TrackInfo[]>([]);
  const [currentIndex, setCurrentIndex] = useState<number | null>(null);
  const [renamingTab, setRenamingTab] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [showCrossfadeSettings, setShowCrossfadeSettings] = useState(false);
  const [showSilenceSettings, setShowSilenceSettings] = useState(false);
  const [showIntroSettings, setShowIntroSettings] = useState(false);
  const [showSettingsMenu, setShowSettingsMenu] = useState(false);
  const [showSchedulePane, setShowSchedulePane] = useState(false);
  const settingsMenuRef = useRef<HTMLDivElement>(null);
  const renameInputRef = useRef<HTMLInputElement>(null);

  const loadPlaylists = useCallback(async () => {
    try {
      const pls = await invoke<PlaylistInfo[]>("get_playlists");
      setPlaylists(pls);
      // Auto-select active playlist, or first, or null
      const active = pls.find((p) => p.is_active);
      const target = active ?? pls[0];
      if (target && selectedPlaylist === null) {
        setSelectedPlaylist(target.name);
      }
    } catch (e) {
      console.error("Failed to load playlists:", e);
    }
  }, [selectedPlaylist]);

  const loadTracks = useCallback(async () => {
    if (!selectedPlaylist) {
      setTracks([]);
      setCurrentIndex(null);
      return;
    }
    try {
      const t = await invoke<TrackInfo[]>("get_playlist_tracks", { name: selectedPlaylist });
      setTracks(t);
      const pl = playlists.find((p) => p.name === selectedPlaylist);
      setCurrentIndex(pl?.current_index ?? null);
    } catch (e) {
      console.error("Failed to load tracks:", e);
      setTracks([]);
    }
  }, [selectedPlaylist, playlists]);

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

  useEffect(() => {
    if (!showSettingsMenu) return;
    const handleClick = (e: MouseEvent) => {
      if (settingsMenuRef.current && !settingsMenuRef.current.contains(e.target as Node)) {
        setShowSettingsMenu(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [showSettingsMenu]);

  const handlePlaylistSelect = async (name: string) => {
    setSelectedPlaylist(name);
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

  const handleReorder = useCallback(async (fromIndex: number, toIndex: number) => {
    if (!selectedPlaylist) return;
    try {
      await invoke("reorder_track", { playlist: selectedPlaylist, from: fromIndex, to: toIndex });
      await loadTracks();
    } catch (e) {
      console.error("Failed to reorder track:", e);
    }
  }, [selectedPlaylist, loadTracks]);

  const handleAddFiles = useCallback(async () => {
    if (!selectedPlaylist) return;
    try {
      const selected = await open({
        multiple: true,
        filters: [{
          name: "Audio Files",
          extensions: AUDIO_EXTENSIONS,
        }],
      });
      if (!selected) return;
      // open() returns string | string[] | null
      const paths = Array.isArray(selected) ? selected : [selected];
      if (paths.length === 0) return;
      await invoke("add_tracks", { playlist: selectedPlaylist, paths });
      await loadTracks();
      await loadPlaylists(); // refresh track counts
    } catch (e) {
      console.error("Failed to add files:", e);
    }
  }, [selectedPlaylist, loadTracks, loadPlaylists]);

  const handleFileDrop = useCallback(async (paths: string[]) => {
    if (!selectedPlaylist || paths.length === 0) return;
    // Filter to audio files only
    const audioPaths = paths.filter((p) => {
      const ext = p.split(".").pop()?.toLowerCase() ?? "";
      return AUDIO_EXTENSIONS.includes(ext);
    });
    if (audioPaths.length === 0) return;
    try {
      await invoke("add_tracks", { playlist: selectedPlaylist, paths: audioPaths });
      await loadTracks();
      await loadPlaylists();
    } catch (e) {
      console.error("Failed to add dropped files:", e);
    }
  }, [selectedPlaylist, loadTracks, loadPlaylists]);

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
        <button
            className={`header-schedule-btn ${showSchedulePane ? "active" : ""}`}
            onClick={() => setShowSchedulePane((v) => !v)}
            title="Toggle schedule"
          >
            {"\u23F0"}
          </button>
        <div className="settings-menu-wrapper" ref={settingsMenuRef}>
          <button
            className="header-settings-btn"
            onClick={() => setShowSettingsMenu((v) => !v)}
            title="Settings"
          >
            {"\u2699"}
          </button>
          {showSettingsMenu && (
            <div className="settings-dropdown">
              <button className="settings-dropdown-item" onClick={() => { setShowCrossfadeSettings(true); setShowSettingsMenu(false); }}>
                Crossfade
              </button>
              <button className="settings-dropdown-item" onClick={() => { setShowSilenceSettings(true); setShowSettingsMenu(false); }}>
                Silence Detection
              </button>
              <button className="settings-dropdown-item" onClick={() => { setShowIntroSettings(true); setShowSettingsMenu(false); }}>
                Auto-Intro
              </button>
            </div>
          )}
        </div>
      </header>
      <main className="main">
        <div className={`main-content ${showSchedulePane ? "with-schedule" : ""}`}>
          <div className="playlist-area">
            {selectedPlaylist ? (
              <PlaylistView
                tracks={tracks}
                currentIndex={currentIndex}
                playlistName={selectedPlaylist}
                onReorder={handleReorder}
                onAddFiles={handleAddFiles}
                onFileDrop={handleFileDrop}
                onTracksChanged={loadTracks}
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
      <TransportBar onTrackChange={loadTracks} />
      {showCrossfadeSettings && (
        <CrossfadeSettings onClose={() => setShowCrossfadeSettings(false)} />
      )}
      {showSilenceSettings && (
        <SilenceSettings onClose={() => setShowSilenceSettings(false)} />
      )}
      {showIntroSettings && (
        <IntroSettings onClose={() => setShowIntroSettings(false)} />
      )}
    </div>
  );
}

export default App;
