import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface PlaybackState {
  isPlaying: boolean;
  positionSecs: number;
}

interface UseEditorPlaybackReturn {
  isPlaying: boolean;
  positionSecs: number;
  play: (startSecs: number) => Promise<void>;
  pause: () => Promise<void>;
  stop: () => Promise<void>;
  seek: (secs: number) => Promise<void>;
  togglePlay: (currentPos: number) => Promise<void>;
}

export function useEditorPlayback(
  path: string | null,
  durationSecs: number,
): UseEditorPlaybackReturn {
  const [state, setState] = useState<PlaybackState>({
    isPlaying: false,
    positionSecs: 0,
  });
  const rafRef = useRef<number | null>(null);
  const isPlayingRef = useRef(false);

  // Poll editor_status via requestAnimationFrame while playing
  const startPolling = useCallback(() => {
    const poll = async () => {
      try {
        const status = await invoke<{ is_playing: boolean; position_secs: number }>(
          "editor_status",
        );
        setState({
          isPlaying: status.is_playing,
          positionSecs: Math.min(status.position_secs, durationSecs),
        });
        isPlayingRef.current = status.is_playing;
        if (status.is_playing) {
          rafRef.current = requestAnimationFrame(poll);
        } else {
          rafRef.current = null;
        }
      } catch {
        rafRef.current = null;
      }
    };
    rafRef.current = requestAnimationFrame(poll);
  }, [durationSecs]);

  const stopPolling = useCallback(() => {
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
  }, []);

  // Stop playback and cleanup on unmount or path change
  useEffect(() => {
    return () => {
      stopPolling();
      invoke("editor_stop").catch(() => undefined);
    };
  }, [path, stopPolling]);

  const play = useCallback(
    async (startSecs: number) => {
      if (!path) return;
      try {
        await invoke("editor_play", { path, startSecs });
        isPlayingRef.current = true;
        setState((prev) => ({ ...prev, isPlaying: true, positionSecs: startSecs }));
        startPolling();
      } catch (e) {
        console.error("editor_play failed:", e);
      }
    },
    [path, startPolling],
  );

  const pause = useCallback(async () => {
    try {
      stopPolling();
      await invoke("editor_stop");
      // Position is preserved by editor_stop (it accumulates elapsed time)
      const status = await invoke<{ is_playing: boolean; position_secs: number }>(
        "editor_status",
      );
      setState({ isPlaying: false, positionSecs: status.position_secs });
    } catch (e) {
      console.error("editor_stop failed:", e);
    }
  }, [stopPolling]);

  const stop = useCallback(async () => {
    try {
      await invoke("editor_stop");
      stopPolling();
      setState({ isPlaying: false, positionSecs: 0 });
      // Reset the backend position too
      await invoke("editor_seek", { positionSecs: 0 });
    } catch (e) {
      console.error("editor_stop failed:", e);
    }
  }, [stopPolling]);

  const seek = useCallback(
    async (secs: number) => {
      try {
        const clamped = Math.max(0, Math.min(secs, durationSecs));
        await invoke("editor_seek", { positionSecs: clamped });
        setState((prev) => ({ ...prev, positionSecs: clamped }));
      } catch (e) {
        console.error("editor_seek failed:", e);
      }
    },
    [durationSecs],
  );

  const togglePlay = useCallback(
    async (currentPos: number) => {
      if (isPlayingRef.current) {
        await pause();
      } else {
        await play(currentPos);
      }
    },
    [play, pause],
  );

  return {
    isPlaying: state.isPlaying,
    positionSecs: state.positionSecs,
    play,
    pause,
    stop,
    seek,
    togglePlay,
  };
}
