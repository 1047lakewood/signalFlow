import { useState, useCallback } from "react";
import type { TrackInfo } from "./types";

interface PlaylistViewProps {
  tracks: TrackInfo[];
  currentIndex: number | null;
  playlistName: string;
  onReorder: (fromIndex: number, toIndex: number) => void;
}

function PlaylistView({ tracks, currentIndex, playlistName, onReorder }: PlaylistViewProps) {
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dropTarget, setDropTarget] = useState<number | null>(null);

  const handleDragStart = useCallback((e: React.DragEvent, index: number) => {
    setDragIndex(index);
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", String(index));
    // Add drag styling after a tick so the ghost image captures the original row
    requestAnimationFrame(() => {
      const row = e.currentTarget as HTMLElement;
      row.classList.add("dragging");
    });
  }, []);

  const handleDragEnd = useCallback((e: React.DragEvent) => {
    (e.currentTarget as HTMLElement).classList.remove("dragging");
    setDragIndex(null);
    setDropTarget(null);
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent, index: number) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    setDropTarget(index);
  }, []);

  const handleDragLeave = useCallback(() => {
    setDropTarget(null);
  }, []);

  const handleDrop = useCallback((e: React.DragEvent, toIndex: number) => {
    e.preventDefault();
    const fromIndex = dragIndex;
    setDragIndex(null);
    setDropTarget(null);
    if (fromIndex !== null && fromIndex !== toIndex) {
      onReorder(fromIndex, toIndex);
    }
  }, [dragIndex, onReorder]);

  if (tracks.length === 0) {
    return (
      <div className="playlist-empty">
        <p>Playlist "{playlistName}" is empty</p>
      </div>
    );
  }

  return (
    <div className="playlist-view">
      <table className="track-table">
        <thead>
          <tr>
            <th className="col-num">#</th>
            <th className="col-status"></th>
            <th className="col-artist">Artist</th>
            <th className="col-title">Title</th>
            <th className="col-duration">Duration</th>
          </tr>
        </thead>
        <tbody>
          {tracks.map((track) => {
            const isCurrent = track.index === currentIndex;
            const isDragging = track.index === dragIndex;
            const isDropTarget = track.index === dropTarget && dropTarget !== dragIndex;
            let className = "track-row";
            if (isCurrent) className += " current";
            if (isDragging) className += " dragging";
            if (isDropTarget) className += " drop-target";
            return (
              <tr
                key={track.index}
                className={className}
                draggable
                onDragStart={(e) => handleDragStart(e, track.index)}
                onDragEnd={handleDragEnd}
                onDragOver={(e) => handleDragOver(e, track.index)}
                onDragLeave={handleDragLeave}
                onDrop={(e) => handleDrop(e, track.index)}
              >
                <td className="col-num">{track.index + 1}</td>
                <td className="col-status">
                  {isCurrent && <span className="playing-indicator">{"\u25B6"}</span>}
                  {track.has_intro && <span className="intro-dot" title="Has intro">{"\u2022"}</span>}
                </td>
                <td className="col-artist">{track.artist}</td>
                <td className="col-title">{track.title}</td>
                <td className="col-duration">{track.duration_display}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export default PlaylistView;
