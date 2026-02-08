import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PlaylistInfo, TrackInfo } from "./types";
import PlaylistView from "./PlaylistView";

function App() {
  const [playlists, setPlaylists] = useState<PlaylistInfo[]>([]);
  const [selectedPlaylist, setSelectedPlaylist] = useState<string | null>(null);
  const [tracks, setTracks] = useState<TrackInfo[]>([]);
  const [currentIndex, setCurrentIndex] = useState<number | null>(null);
  const [renamingTab, setRenamingTab] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
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
        {selectedPlaylist ? (
          <PlaylistView
            tracks={tracks}
            currentIndex={currentIndex}
            playlistName={selectedPlaylist}
          />
        ) : (
          <div className="no-playlist">
            <p>No playlists available</p>
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
