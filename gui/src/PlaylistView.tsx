import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { TrackInfo } from "./types";

interface PlaylistViewProps {
  tracks: TrackInfo[];
  currentIndex: number | null;
  playlistName: string;
  onReorder: (fromIndex: number, toIndex: number) => void;
  onAddFiles: () => void;
  onFileDrop: (paths: string[]) => void;
  onTracksChanged: () => void;
}

interface EditingCell {
  trackIndex: number;
  field: "artist" | "title";
}

function PlaylistView({ tracks, currentIndex, playlistName, onReorder, onAddFiles, onFileDrop, onTracksChanged }: PlaylistViewProps) {
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dropTarget, setDropTarget] = useState<number | null>(null);
  const [isDroppingFiles, setIsDroppingFiles] = useState(false);
  const [editingCell, setEditingCell] = useState<EditingCell | null>(null);
  const [editValue, setEditValue] = useState("");
  const editInputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Listen for Tauri file drop events
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    async function setupDropListener() {
      try {
        const { listen } = await import("@tauri-apps/api/event");

        const unlistenDrop = await listen<{ paths: string[] }>("tauri://drag-drop", (event) => {
          setIsDroppingFiles(false);
          if (event.payload.paths && event.payload.paths.length > 0) {
            onFileDrop(event.payload.paths);
          }
        });

        const unlistenHover = await listen("tauri://drag-enter", () => {
          setIsDroppingFiles(true);
        });

        const unlistenLeave = await listen("tauri://drag-leave", () => {
          setIsDroppingFiles(false);
        });

        unlisten = () => {
          unlistenDrop();
          unlistenHover();
          unlistenLeave();
        };
      } catch (e) {
        console.error("Failed to setup drop listener:", e);
      }
    }

    setupDropListener();
    return () => {
      if (unlisten) unlisten();
    };
  }, [onFileDrop]);

  // Focus input when editing starts
  useEffect(() => {
    if (editingCell && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingCell]);

  const handleStartEdit = useCallback((trackIndex: number, field: "artist" | "title", currentValue: string) => {
    setEditingCell({ trackIndex, field });
    setEditValue(currentValue);
  }, []);

  const handleCancelEdit = useCallback(() => {
    setEditingCell(null);
    setEditValue("");
  }, []);

  const handleCommitEdit = useCallback(async () => {
    if (!editingCell) return;
    const newValue = editValue.trim();
    const track = tracks.find((t) => t.index === editingCell.trackIndex);
    if (!track) {
      handleCancelEdit();
      return;
    }
    const oldValue = editingCell.field === "artist" ? track.artist : track.title;
    if (!newValue || newValue === oldValue) {
      handleCancelEdit();
      return;
    }
    try {
      const params: { playlist: string; trackIndex: number; artist?: string; title?: string } = {
        playlist: playlistName,
        trackIndex: editingCell.trackIndex,
      };
      if (editingCell.field === "artist") {
        params.artist = newValue;
      } else {
        params.title = newValue;
      }
      await invoke("edit_track_metadata", params);
      onTracksChanged();
    } catch (e) {
      console.error("Failed to edit track metadata:", e);
    }
    setEditingCell(null);
    setEditValue("");
  }, [editingCell, editValue, tracks, playlistName, onTracksChanged, handleCancelEdit]);

  const handleEditKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      handleCommitEdit();
    } else if (e.key === "Escape") {
      handleCancelEdit();
    }
  }, [handleCommitEdit, handleCancelEdit]);

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
      <div
        ref={containerRef}
        className={`playlist-empty${isDroppingFiles ? " drop-zone-active" : ""}`}
      >
        <div className="empty-content">
          <p>Playlist "{playlistName}" is empty</p>
          <button className="add-files-btn" onClick={onAddFiles}>
            Add Files
          </button>
          <p className="drop-hint">or drag audio files here</p>
        </div>
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className={`playlist-view${isDroppingFiles ? " drop-zone-active" : ""}`}
    >
      {isDroppingFiles && (
        <div className="drop-overlay">
          <span>Drop audio files to add to playlist</span>
        </div>
      )}
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
            const isEditingArtist = editingCell?.trackIndex === track.index && editingCell?.field === "artist";
            const isEditingTitle = editingCell?.trackIndex === track.index && editingCell?.field === "title";
            let className = "track-row";
            if (isCurrent) className += " current";
            if (isDragging) className += " dragging";
            if (isDropTarget) className += " drop-target";
            return (
              <tr
                key={track.index}
                className={className}
                draggable={!editingCell}
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
                <td
                  className="col-artist editable-cell"
                  onDoubleClick={() => handleStartEdit(track.index, "artist", track.artist)}
                >
                  {isEditingArtist ? (
                    <input
                      ref={editInputRef}
                      className="cell-edit-input"
                      value={editValue}
                      onChange={(e) => setEditValue(e.target.value)}
                      onBlur={handleCommitEdit}
                      onKeyDown={handleEditKeyDown}
                      onClick={(e) => e.stopPropagation()}
                    />
                  ) : (
                    track.artist
                  )}
                </td>
                <td
                  className="col-title editable-cell"
                  onDoubleClick={() => handleStartEdit(track.index, "title", track.title)}
                >
                  {isEditingTitle ? (
                    <input
                      ref={editInputRef}
                      className="cell-edit-input"
                      value={editValue}
                      onChange={(e) => setEditValue(e.target.value)}
                      onBlur={handleCommitEdit}
                      onKeyDown={handleEditKeyDown}
                      onClick={(e) => e.stopPropagation()}
                    />
                  ) : (
                    track.title
                  )}
                </td>
                <td className="col-duration">{track.duration_display}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <div className="playlist-toolbar">
        <button className="add-files-btn" onClick={onAddFiles}>
          + Add Files
        </button>
      </div>
    </div>
  );
}

export default PlaylistView;
