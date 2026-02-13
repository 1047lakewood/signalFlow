import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { TrackInfo } from "./types";

export interface ClipboardData {
  paths: string[];
  sourcePlaylist: string;
  sourceIndices: number[];
  isCut: boolean;
}

interface ColWidths {
  num: number;
  status: number;
  artist: number;
  path: number;
  duration: number;
}

interface ResizeState {
  colKey: keyof ColWidths;
  startX: number;
  startWidth: number;
  sign: number;
}

const COL_WIDTHS_KEY = "signalflow-col-widths";
const DEFAULT_COL_WIDTHS: ColWidths = { num: 40, status: 36, artist: 220, path: 260, duration: 70 };
const MIN_COL_WIDTHS: ColWidths = { num: 30, status: 24, artist: 60, path: 120, duration: 50 };

interface PlaylistViewProps {
  tracks: TrackInfo[];
  currentIndex: number | null;
  playlistName: string;
  selectedIndices: Set<number>;
  clipboard: ClipboardData | null;
  onSelectTracks: (indices: Set<number>) => void;
  onPlayTrack: (index: number) => void;
  onReorder: (fromIndex: number, toIndex: number) => void;
  onCopyTracks: (indices: number[]) => void;
  onCutTracks: (indices: number[]) => void;
  onPasteTracks: (afterIndex: number) => void;
  onAddFiles: () => void;
  onFileDrop: (paths: string[]) => void;
  onTracksChanged: () => void;
}

interface ContextMenuState {
  x: number;
  y: number;
  trackIndex: number;
}

interface EditingCell {
  trackIndex: number;
  field: "artist" | "title";
}

function PlaylistView({ tracks, currentIndex, playlistName, selectedIndices, clipboard, onSelectTracks, onPlayTrack, onReorder, onCopyTracks, onCutTracks, onPasteTracks, onAddFiles, onFileDrop, onTracksChanged }: PlaylistViewProps) {
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dropTarget, setDropTarget] = useState<number | null>(null);
  const [isDroppingFiles, setIsDroppingFiles] = useState(false);
  const [editingCell, setEditingCell] = useState<EditingCell | null>(null);
  const [editValue, setEditValue] = useState("");
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [anchorIndex, setAnchorIndex] = useState<number | null>(null);
  const [findQuery, setFindQuery] = useState("");
  const [jumpRowInput, setJumpRowInput] = useState("");
  const editInputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const normalizedQuery = findQuery.trim().toLowerCase();
  const visibleTracks = normalizedQuery
    ? tracks.filter((track) => {
        const haystack = [
          String(track.index + 1),
          track.artist,
          track.title,
          track.path,
          track.duration_display,
        ]
          .join(" ")
          .toLowerCase();
        return haystack.includes(normalizedQuery);
      })
    : tracks;

  // Column resize state
  const [colWidths, setColWidths] = useState<ColWidths>(() => {
    try {
      const saved = localStorage.getItem(COL_WIDTHS_KEY);
      if (saved) return { ...DEFAULT_COL_WIDTHS, ...JSON.parse(saved) };
    } catch { /* ignore */ }
    return { ...DEFAULT_COL_WIDTHS };
  });
  const [resizeState, setResizeState] = useState<ResizeState | null>(null);

  // Column resize mouse tracking
  useEffect(() => {
    if (!resizeState) return;

    const handleMouseMove = (e: MouseEvent) => {
      const delta = (e.clientX - resizeState.startX) * resizeState.sign;
      const minWidth = MIN_COL_WIDTHS[resizeState.colKey];
      const newWidth = Math.max(minWidth, resizeState.startWidth + delta);
      setColWidths((prev) => ({ ...prev, [resizeState.colKey]: newWidth }));
    };

    const handleMouseUp = () => {
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      setResizeState(null);
      setColWidths((prev) => {
        try { localStorage.setItem(COL_WIDTHS_KEY, JSON.stringify(prev)); } catch { /* ignore */ }
        return prev;
      });
    };

    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [resizeState]);

  const handleResizeMouseDown = useCallback((e: React.MouseEvent, colKey: keyof ColWidths, sign: number = 1) => {
    e.preventDefault();
    e.stopPropagation();
    setResizeState({
      colKey,
      startX: e.clientX,
      startWidth: colWidths[colKey],
      sign,
    });
  }, [colWidths]);

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

  useEffect(() => {
    if (!contextMenu) return;

    const handleWindowClick = () => {
      setContextMenu(null);
    };

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setContextMenu(null);
      }
    };

    window.addEventListener("click", handleWindowClick);
    window.addEventListener("keydown", handleEscape);

    return () => {
      window.removeEventListener("click", handleWindowClick);
      window.removeEventListener("keydown", handleEscape);
    };
  }, [contextMenu]);

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

  const handleRowClick = useCallback((e: React.MouseEvent, trackIndex: number) => {
    if (e.shiftKey && anchorIndex !== null) {
      // Range select from anchor to clicked index
      const min = Math.min(anchorIndex, trackIndex);
      const max = Math.max(anchorIndex, trackIndex);
      const rangeSet = new Set<number>();
      for (let i = min; i <= max; i++) {
        // Only include indices that exist in the track list
        if (tracks.some((t) => t.index === i)) {
          rangeSet.add(i);
        }
      }
      // If Ctrl is also held, merge with existing selection
      if (e.ctrlKey || e.metaKey) {
        const merged = new Set(selectedIndices);
        for (const i of rangeSet) merged.add(i);
        onSelectTracks(merged);
      } else {
        onSelectTracks(rangeSet);
      }
      // Don't update anchor on shift-click
    } else if (e.ctrlKey || e.metaKey) {
      // Toggle individual row
      const next = new Set(selectedIndices);
      if (next.has(trackIndex)) {
        next.delete(trackIndex);
      } else {
        next.add(trackIndex);
      }
      onSelectTracks(next);
      setAnchorIndex(trackIndex);
    } else {
      // Normal click — single select
      onSelectTracks(new Set([trackIndex]));
      setAnchorIndex(trackIndex);
    }
  }, [anchorIndex, selectedIndices, tracks, onSelectTracks]);

  const handleContextMenu = useCallback((e: React.MouseEvent, trackIndex: number) => {
    e.preventDefault();
    // If right-clicked row isn't in current selection, select just that row
    if (!selectedIndices.has(trackIndex)) {
      onSelectTracks(new Set([trackIndex]));
      setAnchorIndex(trackIndex);
    }
    setContextMenu({ x: e.clientX, y: e.clientY, trackIndex });
  }, [selectedIndices, onSelectTracks]);

  const handleContextMenuPlay = useCallback(() => {
    if (!contextMenu) return;
    onPlayTrack(contextMenu.trackIndex);
    setContextMenu(null);
  }, [contextMenu, onPlayTrack]);

  const handleContextMenuCopy = useCallback(() => {
    if (!contextMenu) return;
    const indices = selectedIndices.size > 0 ? Array.from(selectedIndices).sort((a, b) => a - b) : [contextMenu.trackIndex];
    onCopyTracks(indices);
    setContextMenu(null);
  }, [contextMenu, selectedIndices, onCopyTracks]);

  const handleContextMenuCut = useCallback(() => {
    if (!contextMenu) return;
    const indices = selectedIndices.size > 0 ? Array.from(selectedIndices).sort((a, b) => a - b) : [contextMenu.trackIndex];
    onCutTracks(indices);
    setContextMenu(null);
  }, [contextMenu, selectedIndices, onCutTracks]);

  const handleContextMenuPaste = useCallback(() => {
    if (!contextMenu) return;
    onPasteTracks(contextMenu.trackIndex);
    setContextMenu(null);
  }, [contextMenu, onPasteTracks]);

  const handleJumpToRow = useCallback((e: React.FormEvent) => {
    e.preventDefault();
    const row = Number.parseInt(jumpRowInput, 10);
    if (!Number.isFinite(row) || row < 1) return;
    const targetIndex = row - 1;
    const targetExists = tracks.some((track) => track.index === targetIndex);
    if (!targetExists) return;

    if (normalizedQuery && !visibleTracks.some((track) => track.index === targetIndex)) {
      setFindQuery("");
    }

    onSelectTracks(new Set([targetIndex]));
    setAnchorIndex(targetIndex);

    requestAnimationFrame(() => {
      containerRef.current
        ?.querySelector<HTMLElement>(`tr[data-track-index=\"${targetIndex}\"]`)
        ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    });
  }, [jumpRowInput, tracks, normalizedQuery, visibleTracks, onSelectTracks]);

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
      <div className="playlist-findbar">
        <input
          className="playlist-find-input"
          value={findQuery}
          onChange={(e) => setFindQuery(e.target.value)}
          placeholder="Find in playlist..."
        />
        <form className="playlist-jump-form" onSubmit={handleJumpToRow}>
          <label htmlFor="playlist-jump-row">Row</label>
          <input
            id="playlist-jump-row"
            className="playlist-jump-input"
            value={jumpRowInput}
            onChange={(e) => setJumpRowInput(e.target.value.replace(/[^0-9]/g, ""))}
            placeholder="#"
          />
          <button type="submit" className="playlist-jump-btn">Go</button>
        </form>
      </div>
      <table className="track-table">
        <thead>
          <tr>
            <th className="col-num" style={{ width: colWidths.num }}>
              #
              <div className="col-resize-handle" onMouseDown={(e) => handleResizeMouseDown(e, "num")} />
            </th>
            <th className="col-status" style={{ width: colWidths.status }}>
              <div className="col-resize-handle" onMouseDown={(e) => handleResizeMouseDown(e, "status")} />
            </th>
            <th className="col-artist" style={{ width: colWidths.artist }}>
              Artist
              <div className="col-resize-handle" onMouseDown={(e) => handleResizeMouseDown(e, "artist")} />
            </th>
            <th className="col-title">
              Title
              <div className="col-resize-handle" onMouseDown={(e) => handleResizeMouseDown(e, "path", -1)} />
            </th>
            <th className="col-path" style={{ width: colWidths.path }}>
              File Path
              <div className="col-resize-handle" onMouseDown={(e) => handleResizeMouseDown(e, "path")} />
            </th>
            <th className="col-duration" style={{ width: colWidths.duration }}>
              Duration
              <div className="col-resize-handle" onMouseDown={(e) => handleResizeMouseDown(e, "duration")} />
            </th>
          </tr>
        </thead>
        <tbody>
          {visibleTracks.map((track) => {
            const isCurrent = track.index === currentIndex;
            const isSelected = selectedIndices.has(track.index);
            const isDragging = track.index === dragIndex;
            const isDropTarget = track.index === dropTarget && dropTarget !== dragIndex;
            const isEditingArtist = editingCell?.trackIndex === track.index && editingCell?.field === "artist";
            const isEditingTitle = editingCell?.trackIndex === track.index && editingCell?.field === "title";
            let className = "track-row";
            if (isSelected) className += " selected";
            if (isCurrent) className += " current";
            if (isDragging) className += " dragging";
            if (isDropTarget) className += " drop-target";
            return (
              <tr
                key={track.index}
                data-track-index={track.index}
                className={className}
                draggable={!editingCell}
                onClick={(e) => handleRowClick(e, track.index)}
                onDoubleClick={(e) => {
                  // Don't trigger play if double-clicking an editable cell (artist/title handle their own double-click)
                  const target = e.target as HTMLElement;
                  if (!target.closest(".editable-cell")) {
                    onPlayTrack(track.index);
                  }
                }}
                onDragStart={(e) => handleDragStart(e, track.index)}
                onDragEnd={handleDragEnd}
                onDragOver={(e) => handleDragOver(e, track.index)}
                onDragLeave={handleDragLeave}
                onDrop={(e) => handleDrop(e, track.index)}
                onContextMenu={(e) => handleContextMenu(e, track.index)}
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
                <td className="col-path" title={track.path}>{track.path}</td>
                <td className="col-duration">{track.duration_display}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {normalizedQuery && visibleTracks.length === 0 && (
        <div className="playlist-find-empty">No tracks match "{findQuery}".</div>
      )}
      <div className="playlist-toolbar">
        <button className="add-files-btn" onClick={onAddFiles}>
          + Add Files
        </button>
      </div>
      {contextMenu && (
        <div
          className="playlist-context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <button className="playlist-context-item" onClick={handleContextMenuPlay}>
            Play from here
          </button>
          <div className="context-menu-divider" />
          <button className="playlist-context-item" onClick={handleContextMenuCut}>
            Cut{selectedIndices.size > 1 ? ` (${selectedIndices.size})` : ""}
          </button>
          <button className="playlist-context-item" onClick={handleContextMenuCopy}>
            Copy{selectedIndices.size > 1 ? ` (${selectedIndices.size})` : ""}
          </button>
          <button
            className={`playlist-context-item${!clipboard ? " disabled" : ""}`}
            onClick={handleContextMenuPaste}
            disabled={!clipboard}
          >
            Paste{clipboard ? ` (${clipboard.paths.length} track${clipboard.paths.length > 1 ? "s" : ""})` : ""}
          </button>
        </div>
      )}
    </div>
  );
}

export default PlaylistView;
