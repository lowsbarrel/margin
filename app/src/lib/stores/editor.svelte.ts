import type { Editor } from "@tiptap/core";

export type SyncStatus = "idle" | "syncing" | "synced" | "error";

export interface SyncProgress {
  /** Total number of actions to execute */
  total: number;
  /** Number of actions completed so far */
  done: number;
}

interface EditorState {
  syncStatus: SyncStatus;
  syncProgress: SyncProgress | null;
  cursorLine: number;
  cursorCol: number;
  dirty: boolean;
  /** True when a local change occurred while a sync is in flight. */
  localChangeDuringSync: boolean;
}

let state = $state<EditorState>({
  syncStatus: "idle",
  syncProgress: null,
  cursorLine: 1,
  cursorCol: 1,
  dirty: false,
  localChangeDuringSync: false,
});

let tiptapInstance = $state<Editor | null>(null);

export const editor = {
  get syncStatus() {
    return state.syncStatus;
  },
  get syncProgress() {
    return state.syncProgress;
  },
  get cursorLine() {
    return state.cursorLine;
  },
  get cursorCol() {
    return state.cursorCol;
  },
  get dirty() {
    return state.dirty;
  },
  get tiptap() {
    return tiptapInstance;
  },

  setSyncStatus(status: SyncStatus) {
    if (status === "synced" && state.localChangeDuringSync) {
      // A local change happened while syncing — stay 'idle' instead of 'synced'
      state.localChangeDuringSync = false;
      state.syncStatus = "idle";
    } else {
      state.syncStatus = status;
    }
    if (status === "syncing") {
      state.localChangeDuringSync = false;
    }
    if (status !== "syncing") {
      state.syncProgress = null;
    }
  },
  setSyncProgress(progress: SyncProgress | null) {
    state.syncProgress = progress;
  },
  setCursor(line: number, col: number) {
    state.cursorLine = line;
    state.cursorCol = col;
  },
  setDirty(dirty: boolean) {
    state.dirty = dirty;
    if (dirty && state.syncStatus === "synced") {
      state.syncStatus = "idle";
    }
    if (dirty && state.syncStatus === "syncing") {
      state.localChangeDuringSync = true;
    }
  },
  /** Mark that the vault has local changes (e.g. canvas edit, new file) without affecting the dirty flag. */
  markLocalChange() {
    if (state.syncStatus === "synced") {
      state.syncStatus = "idle";
    }
    if (state.syncStatus === "syncing") {
      state.localChangeDuringSync = true;
    }
  },
  setTiptap(instance: Editor | null) {
    tiptapInstance = instance;
  },
};
