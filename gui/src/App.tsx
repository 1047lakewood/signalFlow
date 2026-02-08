import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PlaylistInfo, TrackInfo } from "./types";
import PlaylistView from "./PlaylistView";

function App() {
  const [playlists, setPlaylists] = useState<PlaylistInfo[]>([]);
  const [selectedPlaylist, setSelectedPlaylist] = useState<string | null>(null);
  const [tracks, setTracks] = useState<TrackInfo[]>([]);
  const [currentIndex, setCurrentIndex] = useState<number | null>(null);

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

  const handlePlaylistSelect = (name: string) => {
    setSelectedPlaylist(name);
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
            >
              {pl.name}
              <span className="track-count">{pl.track_count}</span>
            </button>
          ))}
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
