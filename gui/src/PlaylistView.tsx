import { useState, useCallback, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";
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
  playtime: number;
  duration: number;
}

interface ResizeState {
  colKey: keyof ColWidths;
  startX: number;
  startWidth: number;
  sign: number;
}

const COL_WIDTHS_KEY = "signalflow-col-widths";
const DEFAULT_COL_WIDTHS: ColWidths = {
  num: 40,
  status: 36,
  artist: 220,
  path: 260,
  playtime: 120,
  duration: 70,
};
const MIN_COL_WIDTHS: ColWidths = {
  num: 30,
  status: 24,
  artist: 60,
  path: 120,
  playtime: 90,
  duration: 50,
};

function formatTrackPathForDisplay(path: string): string {
  // Strip verbatim UNC prefix: \\?\UNC\server\share\... → \\server\share\...
  if (path.startsWith("\\\\?\\UNC\\")) {
    return "\\\\" + path.slice(8);
  }

  // Strip verbatim local prefix: \\?\C:\... → C:\...
  if (path.startsWith("\\\\?\\")) {
    return path.slice(4);
  }

  // Convert admin shares: \\server\C$\... → C:\...
  const uncAdminShare = path.match(/^\\\\[^\\]+\\([A-Za-z])\$\\(.*)$/);
  if (uncAdminShare) {
    return `${uncAdminShare[1].toUpperCase()}:\\${uncAdminShare[2]}`;
  }

  return path;
}

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
  onSearchFilename: (filename: string) => void;
  findRequestToken: number;
  onEditAudio?: (path: string) => void;
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

function PlaylistView({
  tracks,
  currentIndex,
  playlistName,
  selectedIndices,
  clipboard,
  onSelectTracks,
  onPlayTrack,
  onReorder,
  onCopyTracks,
  onCutTracks,
  onPasteTracks,
  onAddFiles,
  onFileDrop,
  onTracksChanged,
  onSearchFilename,
  findRequestToken,
  onEditAudio,
}: PlaylistViewProps) {
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dropTarget, setDropTarget] = useState<number | null>(null);
  const [isDroppingFiles, setIsDroppingFiles] = useState(false);
  const [editingCell, setEditingCell] = useState<EditingCell | null>(null);
  const [editValue, setEditValue] = useState("");
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [anchorIndex, setAnchorIndex] = useState<number | null>(null);
  const [renameDialog, setRenameDialog] = useState<{
    trackIndex: number;
    currentPath: string;
  } | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [convertConfirm, setConvertConfirm] = useState<{
    indices: number[];
  } | null>(null);
  const [processingMsg, setProcessingMsg] = useState<string | null>(null);
  const [findQuery, setFindQuery] = useState("");
  const [findBarOpen, setFindBarOpen] = useState(false);
  const [findCurrentMatch, setFindCurrentMatch] = useState(0);
  const editInputRef = useRef<HTMLInputElement>(null);
  const findInputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const normalizedQuery = findQuery.trim().toLowerCase();
  const findMatchIndices = useMemo(() => {
    if (!normalizedQuery) return [];
    return tracks
      .filter((track) => {
        const haystack = [
          String(track.index + 1),
          track.artist,
          track.title,
          track.path,
          formatTrackPathForDisplay(track.path),
          track.start_time_display ?? "",
          track.duration_display,
        ]
          .join(" ")
          .toLowerCase();
        return haystack.includes(normalizedQuery);
      })
      .map((track) => track.index);
  }, [tracks, normalizedQuery]);

  // Column resize state
  const [colWidths, setColWidths] = useState<ColWidths>(() => {
    try {
      const saved = localStorage.getItem(COL_WIDTHS_KEY);
      if (saved) return { ...DEFAULT_COL_WIDTHS, ...JSON.parse(saved) };
    } catch {
      /* ignore */
    }
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
        try {
          localStorage.setItem(COL_WIDTHS_KEY, JSON.stringify(prev));
        } catch {
          /* ignore */
        }
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

  const handleResizeMouseDown = useCallback(
    (e: React.MouseEvent, colKey: keyof ColWidths, sign: number = 1) => {
      e.preventDefault();
      e.stopPropagation();
      setResizeState({
        colKey,
        startX: e.clientX,
        startWidth: colWidths[colKey],
        sign,
      });
    },
    [colWidths],
  );

  // Listen for Tauri file drop events
  useEffect(() => {
    let mounted = true;
    const unlisteners: Array<() => void> = [];

    async function setupDropListener() {
      try {
        const { listen } = await import("@tauri-apps/api/event");

        const unlistenDrop = await listen<{ paths: string[] }>(
          "tauri://drag-drop",
          (event) => {
            setIsDroppingFiles(false);
            if (event.payload.paths && event.payload.paths.length > 0) {
              onFileDrop(event.payload.paths);
            }
          },
        );

        const unlistenHover = await listen("tauri://drag-enter", () => {
          setIsDroppingFiles(true);
        });

        const unlistenLeave = await listen("tauri://drag-leave", () => {
          setIsDroppingFiles(false);
        });

        if (!mounted) {
          // Component unmounted while we were setting up — clean up immediately
          unlistenDrop();
          unlistenHover();
          unlistenLeave();
          return;
        }

        unlisteners.push(unlistenDrop, unlistenHover, unlistenLeave);
      } catch (e) {
        console.error("Failed to setup drop listener:", e);
      }
    }

    setupDropListener();
    return () => {
      mounted = false;
      unlisteners.forEach((fn) => fn());
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

  const handleStartEdit = useCallback(
    (trackIndex: number, field: "artist" | "title", currentValue: string) => {
      setEditingCell({ trackIndex, field });
      setEditValue(currentValue);
    },
    [],
  );

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
    const oldValue =
      editingCell.field === "artist" ? track.artist : track.title;
    if (!newValue || newValue === oldValue) {
      handleCancelEdit();
      return;
    }
    try {
      const params: {
        playlist: string;
        trackIndex: number;
        artist?: string;
        title?: string;
      } = {
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
      setEditingCell(null);
      setEditValue("");
    } catch (e) {
      console.error("Failed to edit track metadata:", e);
      // Keep editor open so the user can retry or cancel manually
    }
  }, [
    editingCell,
    editValue,
    tracks,
    playlistName,
    onTracksChanged,
    handleCancelEdit,
  ]);

  const handleEditKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        handleCommitEdit();
      } else if (e.key === "Escape") {
        handleCancelEdit();
      }
    },
    [handleCommitEdit, handleCancelEdit],
  );

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

  const handleDrop = useCallback(
    (e: React.DragEvent, toIndex: number) => {
      e.preventDefault();
      const fromIndex = dragIndex;
      setDragIndex(null);
      setDropTarget(null);
      if (fromIndex !== null && fromIndex !== toIndex) {
        onReorder(fromIndex, toIndex);
      }
    },
    [dragIndex, onReorder],
  );

  const handleRowClick = useCallback(
    (e: React.MouseEvent, trackIndex: number) => {
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
    },
    [anchorIndex, selectedIndices, tracks, onSelectTracks],
  );

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, trackIndex: number) => {
      e.preventDefault();
      // If right-clicked row isn't in current selection, select just that row
      if (!selectedIndices.has(trackIndex)) {
        onSelectTracks(new Set([trackIndex]));
        setAnchorIndex(trackIndex);
      }
      setContextMenu({ x: e.clientX, y: e.clientY, trackIndex });
    },
    [selectedIndices, onSelectTracks],
  );

  const handleContextMenuPlay = useCallback(() => {
    if (!contextMenu) return;
    onPlayTrack(contextMenu.trackIndex);
    setContextMenu(null);
  }, [contextMenu, onPlayTrack]);

  const handleContextMenuCopy = useCallback(() => {
    if (!contextMenu) return;
    const indices =
      selectedIndices.size > 0
        ? Array.from(selectedIndices).sort((a, b) => a - b)
        : [contextMenu.trackIndex];
    onCopyTracks(indices);
    setContextMenu(null);
  }, [contextMenu, selectedIndices, onCopyTracks]);

  const handleContextMenuCut = useCallback(() => {
    if (!contextMenu) return;
    const indices =
      selectedIndices.size > 0
        ? Array.from(selectedIndices).sort((a, b) => a - b)
        : [contextMenu.trackIndex];
    onCutTracks(indices);
    setContextMenu(null);
  }, [contextMenu, selectedIndices, onCutTracks]);

  const handleContextMenuPaste = useCallback(() => {
    if (!contextMenu) return;
    onPasteTracks(contextMenu.trackIndex);
    setContextMenu(null);
  }, [contextMenu, onPasteTracks]);

  const handleContextMenuSearchFilename = useCallback(() => {
    if (!contextMenu) return;
    const track = tracks.find((t) => t.index === contextMenu.trackIndex);
    if (!track) return;
    const filename = track.path.split(/[/\\]/).pop() ?? track.title;
    onSearchFilename(filename);
    setContextMenu(null);
  }, [contextMenu, tracks, onSearchFilename]);

  // ── New context menu handlers ────────────────────────────────────────────

  const handleContextMenuDelete = useCallback(async () => {
    const indices =
      selectedIndices.size > 0
        ? Array.from(selectedIndices).sort((a, b) => a - b)
        : contextMenu
          ? [contextMenu.trackIndex]
          : [];
    if (indices.length === 0) return;
    setContextMenu(null);
    try {
      await invoke("remove_tracks", { playlist: playlistName, indices });
      onTracksChanged();
    } catch (e) {
      console.error("Delete failed:", e);
    }
  }, [contextMenu, selectedIndices, playlistName, onTracksChanged]);

  const handleDeleteKey = useCallback(
    async (e: KeyboardEvent) => {
      if (e.key !== "Delete") return;
      // Don't delete while editing a cell or rename dialog is open
      if (editingCell || renameDialog) return;
      const active = document.activeElement;
      if (
        active instanceof HTMLInputElement ||
        active instanceof HTMLTextAreaElement
      )
        return;
      if (selectedIndices.size === 0) return;
      const indices = Array.from(selectedIndices).sort((a, b) => a - b);
      try {
        await invoke("remove_tracks", { playlist: playlistName, indices });
        onTracksChanged();
      } catch (e) {
        console.error("Delete failed:", e);
      }
    },
    [editingCell, renameDialog, selectedIndices, playlistName, onTracksChanged],
  );

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    el.addEventListener("keydown", handleDeleteKey);
    return () => el.removeEventListener("keydown", handleDeleteKey);
  }, [handleDeleteKey]);

  const handleContextMenuOpenLocation = useCallback(async () => {
    if (!contextMenu) return;
    const track = tracks.find((t) => t.index === contextMenu.trackIndex);
    setContextMenu(null);
    if (!track) return;
    try {
      await invoke("open_file_location", { path: track.path });
    } catch (e) {
      console.error("Open file location failed:", e);
    }
  }, [contextMenu, tracks]);

  const handleContextMenuOpenAudacity = useCallback(async () => {
    if (!contextMenu) return;
    const track = tracks.find((t) => t.index === contextMenu.trackIndex);
    setContextMenu(null);
    if (!track) return;
    try {
      await invoke("open_in_audacity", { path: track.path });
    } catch (e) {
      alert(`Failed to open in Audacity: ${e}`);
    }
  }, [contextMenu, tracks]);

  const handleContextMenuEditAudio = useCallback(() => {
    if (!contextMenu) return;
    const track = tracks.find((t) => t.index === contextMenu.trackIndex);
    setContextMenu(null);
    if (!track || !onEditAudio) return;
    onEditAudio(track.path);
  }, [contextMenu, tracks, onEditAudio]);

  const handleContextMenuConvertMp3 = useCallback(() => {
    if (!contextMenu) return;
    const indices =
      selectedIndices.size > 0
        ? Array.from(selectedIndices).sort((a, b) => a - b)
        : [contextMenu.trackIndex];
    setContextMenu(null);
    setConvertConfirm({ indices });
  }, [contextMenu, selectedIndices]);

  const handleConvertConfirm = useCallback(async () => {
    if (!convertConfirm) return;
    const { indices } = convertConfirm;
    setConvertConfirm(null);
    setProcessingMsg("Converting to MP3…");
    try {
      const [converted, skipped, failed] = (await invoke(
        "convert_tracks_to_mp3",
        { playlist: playlistName, indices },
      )) as [number, number, number];
      onTracksChanged();
      alert(
        `Conversion complete:\n• Converted: ${converted}\n• Skipped (already MP3): ${skipped}\n• Failed: ${failed}`,
      );
    } catch (e) {
      alert(`Conversion failed: ${e}`);
    } finally {
      setProcessingMsg(null);
    }
  }, [convertConfirm, playlistName, onTracksChanged]);

  const handleContextMenuRenamePath = useCallback(() => {
    if (!contextMenu) return;
    const track = tracks.find((t) => t.index === contextMenu.trackIndex);
    setContextMenu(null);
    if (!track) return;
    setRenameValue(track.path);
    setRenameDialog({ trackIndex: track.index, currentPath: track.path });
  }, [contextMenu, tracks]);

  const handleRenameSubmit = useCallback(async () => {
    if (!renameDialog) return;
    const newPath = renameValue.trim();
    if (!newPath || newPath === renameDialog.currentPath) {
      setRenameDialog(null);
      return;
    }
    setRenameDialog(null);
    setProcessingMsg("Renaming…");
    try {
      await invoke("rename_track_file", {
        playlist: playlistName,
        trackIndex: renameDialog.trackIndex,
        newPath,
      });
      onTracksChanged();
    } catch (e) {
      alert(`Rename failed: ${e}`);
    } finally {
      setProcessingMsg(null);
    }
  }, [renameDialog, renameValue, playlistName, onTracksChanged]);

  const handleContextMenuRenameBrowse = useCallback(async () => {
    if (!contextMenu) return;
    const track = tracks.find((t) => t.index === contextMenu.trackIndex);
    setContextMenu(null);
    if (!track) return;
    try {
      const selected = await dialogOpen({
        title: "Select New Audio File",
        defaultPath: track.path,
        filters: [
          {
            name: "Audio",
            extensions: ["mp3", "wav", "flac", "aac", "ogg", "m4a"],
          },
          { name: "All Files", extensions: ["*"] },
        ],
        multiple: false,
      });
      if (!selected || Array.isArray(selected)) return;
      await invoke("update_track_path", {
        playlist: playlistName,
        trackIndex: track.index,
        newPath: selected,
      });
      onTracksChanged();
    } catch (e) {
      console.error("Rename by browsing failed:", e);
    }
  }, [contextMenu, tracks, playlistName, onTracksChanged]);

  const handleContextMenuReplaceMacro = useCallback(async () => {
    if (!contextMenu) return;
    const track = tracks.find((t) => t.index === contextMenu.trackIndex);
    setContextMenu(null);
    if (!track) return;
    setProcessingMsg("Waiting for macro output file…");
    try {
      await invoke("replace_from_macro_output", {
        playlist: playlistName,
        trackIndex: track.index,
      });
      onTracksChanged();
    } catch (e) {
      alert(`Replace from Macro Output failed: ${e}`);
    } finally {
      setProcessingMsg(null);
    }
  }, [contextMenu, tracks, playlistName, onTracksChanged]);

  const handleContextMenuAddAm = useCallback(async () => {
    if (!contextMenu) return;
    const track = tracks.find((t) => t.index === contextMenu.trackIndex);
    setContextMenu(null);
    if (!track) return;
    try {
      await invoke("add_am_to_filename", {
        playlist: playlistName,
        trackIndex: track.index,
      });
      onTracksChanged();
    } catch (e) {
      alert(`Add AM to Filename failed: ${e}`);
    }
  }, [contextMenu, tracks, playlistName, onTracksChanged]);

  const scrollToTrackIndex = useCallback((trackIndex: number) => {
    requestAnimationFrame(() => {
      containerRef.current
        ?.querySelector<HTMLElement>(
          `tr[data-track-index="${trackIndex}"]`,
        )
        ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    });
  }, []);

  // When query changes, reset to first match and scroll there
  useEffect(() => {
    setFindCurrentMatch(0);
    if (findMatchIndices.length > 0) {
      scrollToTrackIndex(findMatchIndices[0]);
    }
  }, [findMatchIndices, scrollToTrackIndex]);

  const handleFindNext = useCallback(() => {
    if (findMatchIndices.length === 0) return;
    const next = (findCurrentMatch + 1) % findMatchIndices.length;
    setFindCurrentMatch(next);
    scrollToTrackIndex(findMatchIndices[next]);
  }, [findMatchIndices, findCurrentMatch, scrollToTrackIndex]);

  const handleFindPrev = useCallback(() => {
    if (findMatchIndices.length === 0) return;
    const prev =
      (findCurrentMatch - 1 + findMatchIndices.length) %
      findMatchIndices.length;
    setFindCurrentMatch(prev);
    scrollToTrackIndex(findMatchIndices[prev]);
  }, [findMatchIndices, findCurrentMatch, scrollToTrackIndex]);

  const handleFindClose = useCallback(() => {
    setFindBarOpen(false);
    setFindQuery("");
    setFindCurrentMatch(0);
  }, []);

  useEffect(() => {
    if (findRequestToken === 0) return;
    setFindBarOpen(true);
    requestAnimationFrame(() => findInputRef.current?.focus());
  }, [findRequestToken]);

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
      tabIndex={-1}
    >
      {isDroppingFiles && (
        <div className="drop-overlay">
          <span>Drop audio files to add to playlist</span>
        </div>
      )}
      <table className="track-table">
        <thead>
          <tr>
            <th className="col-num" style={{ width: colWidths.num }}>
              #
              <div
                className="col-resize-handle"
                onMouseDown={(e) => handleResizeMouseDown(e, "num")}
              />
            </th>
            <th className="col-status" style={{ width: colWidths.status }}>
              <div
                className="col-resize-handle"
                onMouseDown={(e) => handleResizeMouseDown(e, "status")}
              />
            </th>
            <th className="col-artist" style={{ width: colWidths.artist }}>
              Artist
              <div
                className="col-resize-handle"
                onMouseDown={(e) => handleResizeMouseDown(e, "artist")}
              />
            </th>
            <th className="col-title">
              Title
              <div
                className="col-resize-handle"
                onMouseDown={(e) => handleResizeMouseDown(e, "path", -1)}
              />
            </th>
            <th className="col-path" style={{ width: colWidths.path }}>
              File Path
              <div
                className="col-resize-handle"
                onMouseDown={(e) => handleResizeMouseDown(e, "path")}
              />
            </th>
            <th className="col-playtime" style={{ width: colWidths.playtime }}>
              Playtime
              <div
                className="col-resize-handle"
                onMouseDown={(e) => handleResizeMouseDown(e, "playtime")}
              />
            </th>
            <th className="col-duration" style={{ width: colWidths.duration }}>
              Duration
              <div
                className="col-resize-handle"
                onMouseDown={(e) => handleResizeMouseDown(e, "duration")}
              />
            </th>
          </tr>
        </thead>
        <tbody>
          {tracks.map((track) => {
            const isCurrent = track.index === currentIndex;
            const isSelected = selectedIndices.has(track.index);
            const isDragging = track.index === dragIndex;
            const isDropTarget =
              track.index === dropTarget && dropTarget !== dragIndex;
            const isEditingArtist =
              editingCell?.trackIndex === track.index &&
              editingCell?.field === "artist";
            const isEditingTitle =
              editingCell?.trackIndex === track.index &&
              editingCell?.field === "title";
            const displayPath = formatTrackPathForDisplay(track.path);
            const matchPos = findMatchIndices.indexOf(track.index);
            const isFindMatch = matchPos !== -1;
            const isFindCurrent = isFindMatch && matchPos === findCurrentMatch;
            let className = "track-row";
            if (isSelected) className += " selected";
            if (isCurrent) className += " current";
            if (isDragging) className += " dragging";
            if (isDropTarget) className += " drop-target";
            if (isFindMatch) className += " find-match";
            if (isFindCurrent) className += " find-current";
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
                  {isCurrent && (
                    <span className="playing-indicator">{"\u25B6"}</span>
                  )}
                  {track.has_intro && (
                    <span className="intro-dot" title="Has intro">
                      {"\u2022"}
                    </span>
                  )}
                </td>
                <td
                  className="col-artist editable-cell"
                  onDoubleClick={() =>
                    handleStartEdit(track.index, "artist", track.artist)
                  }
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
                  onDoubleClick={() =>
                    handleStartEdit(track.index, "title", track.title)
                  }
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
                <td className="col-path" title={displayPath}>
                  {displayPath}
                </td>
                <td className="col-playtime">{track.start_time_display ?? "—"}</td>
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
      {findBarOpen && (
        <div className="playlist-findbar">
          <input
            ref={findInputRef}
            className="playlist-find-input"
            value={findQuery}
            onChange={(e) => setFindQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                if (e.shiftKey) handleFindPrev();
                else handleFindNext();
              } else if (e.key === "Escape") {
                handleFindClose();
              }
            }}
            placeholder="Find in playlist..."
          />
          <span className="find-match-count">
            {normalizedQuery
              ? findMatchIndices.length > 0
                ? `${findCurrentMatch + 1} of ${findMatchIndices.length}`
                : "No matches"
              : ""}
          </span>
          <button
            className="find-nav-btn"
            onClick={handleFindPrev}
            disabled={findMatchIndices.length === 0}
            title="Previous match (Shift+Enter)"
          >
            {"\u25B2"}
          </button>
          <button
            className="find-nav-btn"
            onClick={handleFindNext}
            disabled={findMatchIndices.length === 0}
            title="Next match (Enter)"
          >
            {"\u25BC"}
          </button>
          <button
            className="find-close-btn"
            onClick={handleFindClose}
            title="Close (Escape)"
          >
            {"\u2715"}
          </button>
        </div>
      )}
      {contextMenu && (
        <div
          className="playlist-context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            className="playlist-context-item"
            onClick={handleContextMenuPlay}
          >
            Play from here
          </button>
          <div className="context-menu-divider" />
          <button
            className="playlist-context-item"
            onClick={handleContextMenuCut}
          >
            Cut{selectedIndices.size > 1 ? ` (${selectedIndices.size})` : ""}
          </button>
          <button
            className="playlist-context-item"
            onClick={handleContextMenuCopy}
          >
            Copy{selectedIndices.size > 1 ? ` (${selectedIndices.size})` : ""}
          </button>
          <button
            className={`playlist-context-item${!clipboard ? " disabled" : ""}`}
            onClick={handleContextMenuPaste}
            disabled={!clipboard}
          >
            Paste
            {clipboard
              ? ` (${clipboard.paths.length} track${clipboard.paths.length > 1 ? "s" : ""})`
              : ""}
          </button>
          <div className="context-menu-divider" />
          <button
            className="playlist-context-item"
            onClick={handleContextMenuDelete}
          >
            Delete
            {selectedIndices.size > 1 ? ` (${selectedIndices.size})` : ""}
          </button>
          <div className="context-menu-divider" />
          <button
            className={`playlist-context-item${selectedIndices.size > 1 ? " disabled" : ""}`}
            onClick={handleContextMenuOpenLocation}
            disabled={selectedIndices.size > 1}
          >
            Open File Location
          </button>
          <button
            className={`playlist-context-item${selectedIndices.size > 1 ? " disabled" : ""}`}
            onClick={handleContextMenuOpenAudacity}
            disabled={selectedIndices.size > 1}
          >
            Open in Audacity
          </button>
          {onEditAudio && (
            <button
              className={`playlist-context-item${selectedIndices.size > 1 ? " disabled" : ""}`}
              onClick={handleContextMenuEditAudio}
              disabled={selectedIndices.size > 1}
            >
              Edit Audio
            </button>
          )}
          <button
            className="playlist-context-item"
            onClick={handleContextMenuConvertMp3}
          >
            Convert to MP3
            {selectedIndices.size > 1 ? ` (${selectedIndices.size})` : ""}
          </button>
          <div className="context-menu-divider" />
          <button
            className={`playlist-context-item${selectedIndices.size > 1 ? " disabled" : ""}`}
            onClick={handleContextMenuRenamePath}
            disabled={selectedIndices.size > 1}
          >
            Rename File Path
          </button>
          <button
            className={`playlist-context-item${selectedIndices.size > 1 ? " disabled" : ""}`}
            onClick={handleContextMenuRenameBrowse}
            disabled={selectedIndices.size > 1}
          >
            Rename by Browsing
          </button>
          <button
            className={`playlist-context-item${selectedIndices.size > 1 ? " disabled" : ""}`}
            onClick={handleContextMenuReplaceMacro}
            disabled={selectedIndices.size > 1}
          >
            Replace from Macro Output
          </button>
          <button
            className={`playlist-context-item${selectedIndices.size > 1 ? " disabled" : ""}`}
            onClick={handleContextMenuAddAm}
            disabled={selectedIndices.size > 1}
          >
            Add AM to Filename
          </button>
          <div className="context-menu-divider" />
          <button
            className="playlist-context-item"
            onClick={handleContextMenuSearchFilename}
          >
            Search filename across index
          </button>
        </div>
      )}
      {/* Rename File Path dialog */}
      {renameDialog && (
        <div
          className="playlist-modal-backdrop"
          onClick={() => setRenameDialog(null)}
        >
          <div
            className="playlist-modal"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="playlist-modal-title">Rename File Path</div>
            <input
              className="playlist-modal-input"
              type="text"
              value={renameValue}
              onChange={(e) => setRenameValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleRenameSubmit();
                if (e.key === "Escape") setRenameDialog(null);
              }}
              autoFocus
            />
            <div className="playlist-modal-actions">
              <button
                className="playlist-modal-btn"
                onClick={handleRenameSubmit}
              >
                OK
              </button>
              <button
                className="playlist-modal-btn secondary"
                onClick={() => setRenameDialog(null)}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Convert to MP3 confirmation dialog */}
      {convertConfirm && (
        <div
          className="playlist-modal-backdrop"
          onClick={() => setConvertConfirm(null)}
        >
          <div
            className="playlist-modal"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="playlist-modal-title">Convert to MP3</div>
            <p className="playlist-modal-body">
              {convertConfirm.indices.length === 1
                ? "Convert 1 track to MP3?"
                : `Convert ${convertConfirm.indices.length} tracks to MP3?`}
              <br />
              Original files will be deleted after conversion.
            </p>
            <div className="playlist-modal-actions">
              <button
                className="playlist-modal-btn"
                onClick={handleConvertConfirm}
              >
                Convert
              </button>
              <button
                className="playlist-modal-btn secondary"
                onClick={() => setConvertConfirm(null)}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Processing overlay */}
      {processingMsg && (
        <div className="playlist-processing-overlay">
          <span>{processingMsg}</span>
        </div>
      )}
    </div>
  );
}

export default PlaylistView;
