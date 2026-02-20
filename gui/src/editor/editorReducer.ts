import type { EditorAction, EditorState, UndoRedoState } from "./editorTypes";
import { defaultOps } from "./editorTypes";

// Compute effective trim duration for total_duration_secs
function computeTotalDuration(ops: EditorState["ops"]): number {
  const start = ops.trim_start_secs;
  const end = ops.trim_end_secs;
  const base = end - start;
  // Cut regions reduce total duration
  const cutTotal = ops.cuts
    .filter((c) => c.start_secs >= start && c.end_secs <= end)
    .reduce((acc, c) => acc + (c.end_secs - c.start_secs), 0);
  return Math.max(0, base - cutTotal);
}

function withTotalDuration(state: EditorState): EditorState {
  return {
    ...state,
    ops: {
      ...state.ops,
      total_duration_secs: computeTotalDuration(state.ops),
    },
  };
}

// Push present into past, clear future (new branch)
function pushHistory(
  current: UndoRedoState,
  next: EditorState,
): UndoRedoState {
  return {
    past: [...current.past, current.present],
    present: withTotalDuration(next),
    future: [],
  };
}

export function editorReducer(
  state: UndoRedoState,
  action: EditorAction,
): UndoRedoState {
  const { present } = state;

  switch (action.type) {
    case "RESET": {
      const newState: EditorState = {
        path: action.path,
        ops: defaultOps(action.duration),
        selectionStart: null,
        selectionEnd: null,
        markers: [],
      };
      return { past: [], present: newState, future: [] };
    }

    case "UNDO": {
      if (state.past.length === 0) return state;
      const prev = state.past[state.past.length - 1];
      return {
        past: state.past.slice(0, -1),
        present: prev,
        future: [present, ...state.future],
      };
    }

    case "REDO": {
      if (state.future.length === 0) return state;
      const next = state.future[0];
      return {
        past: [...state.past, present],
        present: next,
        future: state.future.slice(1),
      };
    }

    case "SET_SELECTION":
      return {
        ...state,
        present: {
          ...present,
          selectionStart: action.start,
          selectionEnd: action.end,
        },
      };

    case "SET_TRIM_START":
      return pushHistory(state, {
        ...present,
        ops: { ...present.ops, trim_start_secs: action.secs },
      });

    case "SET_TRIM_END":
      return pushHistory(state, {
        ...present,
        ops: { ...present.ops, trim_end_secs: action.secs },
      });

    case "SET_VOLUME":
      return pushHistory(state, {
        ...present,
        ops: { ...present.ops, volume_db: action.db },
      });

    case "SET_SPEED":
      return pushHistory(state, {
        ...present,
        ops: { ...present.ops, speed: action.speed },
      });

    case "SET_PITCH":
      return pushHistory(state, {
        ...present,
        ops: { ...present.ops, pitch_semitones: action.semitones },
      });

    case "SET_FADE_IN":
      return pushHistory(state, {
        ...present,
        ops: { ...present.ops, fade_in_secs: action.secs },
      });

    case "SET_FADE_OUT":
      return pushHistory(state, {
        ...present,
        ops: { ...present.ops, fade_out_secs: action.secs },
      });

    case "SET_NORMALIZE":
      return pushHistory(state, {
        ...present,
        ops: { ...present.ops, normalize: action.enabled },
      });

    case "ADD_CUT":
      return pushHistory(state, {
        ...present,
        ops: { ...present.ops, cuts: [...present.ops.cuts, action.region] },
      });

    case "REMOVE_CUT":
      return pushHistory(state, {
        ...present,
        ops: {
          ...present.ops,
          cuts: present.ops.cuts.filter((_, i) => i !== action.index),
        },
      });

    case "TRIM_TO_SELECTION": {
      const { selectionStart, selectionEnd } = present;
      if (selectionStart === null || selectionEnd === null) return state;
      const start = Math.min(selectionStart, selectionEnd);
      const end = Math.max(selectionStart, selectionEnd);
      return pushHistory(state, {
        ...present,
        ops: { ...present.ops, trim_start_secs: start, trim_end_secs: end },
        selectionStart: null,
        selectionEnd: null,
      });
    }

    case "CUT_SELECTION": {
      const { selectionStart, selectionEnd } = present;
      if (selectionStart === null || selectionEnd === null) return state;
      const start = Math.min(selectionStart, selectionEnd);
      const end = Math.max(selectionStart, selectionEnd);
      return pushHistory(state, {
        ...present,
        ops: {
          ...present.ops,
          cuts: [...present.ops.cuts, { start_secs: start, end_secs: end }],
        },
        selectionStart: null,
        selectionEnd: null,
      });
    }

    case "ADD_MARKER":
      return {
        ...state,
        present: {
          ...present,
          markers: [...present.markers, action.marker],
        },
      };

    case "REMOVE_MARKER":
      return {
        ...state,
        present: {
          ...present,
          markers: present.markers.filter((m) => m.id !== action.id),
        },
      };

    case "RENAME_MARKER":
      return {
        ...state,
        present: {
          ...present,
          markers: present.markers.map((m) =>
            m.id === action.id ? { ...m, label: action.label } : m,
          ),
        },
      };

    default:
      return state;
  }
}

export function makeInitialState(path: string, duration: number): UndoRedoState {
  return {
    past: [],
    present: {
      path,
      ops: defaultOps(duration),
      selectionStart: null,
      selectionEnd: null,
      markers: [],
    },
    future: [],
  };
}
