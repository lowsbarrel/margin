export type DragItem =
  | { kind: "file"; path: string; label: string; isDir: boolean }
  | { kind: "tab"; paneIndex: number; tabIndex: number; label: string };

interface DragState {
  item: DragItem;
  x: number;
  y: number;
}

export interface PendingInsert {
  path: string;
  x: number;
  y: number;
}

let state = $state<DragState | null>(null);
let insertState = $state<PendingInsert | null>(null);

/** Tracks whether a native OS drag is in progress (file dragged outside the window). */
let _nativeDragActive = false;
let _droppedBackInApp = false;

export const drag = {
  get active() {
    return state !== null;
  },
  get item() {
    return state?.item ?? null;
  },
  get x() {
    return state?.x ?? 0;
  },
  get y() {
    return state?.y ?? 0;
  },
  get pendingInsert() {
    return insertState;
  },
  start(item: DragItem, x: number, y: number) {
    state = { item, x, y };
  },
  move(x: number, y: number) {
    if (state) {
      state.x = x;
      state.y = y;
    }
  },
  end() {
    state = null;
  },
  requestInsertAtCoords(path: string, x: number, y: number) {
    insertState = { path, x, y };
  },
  clearPendingInsert() {
    insertState = null;
  },
  /** Mark that a native OS drag has started (file dragged outside the window). */
  startNativeDrag() {
    _nativeDragActive = true;
    _droppedBackInApp = false;
  },
  /** Mark that the native OS drag has ended. */
  endNativeDrag() {
    _nativeDragActive = false;
  },
  /** Whether a native OS drag is currently in progress. */
  get nativeDragActive() {
    return _nativeDragActive;
  },
  /** Signal that the dragged file was dropped back into the app. */
  markDroppedBackInApp() {
    _droppedBackInApp = true;
  },
  /** Whether the file was dropped back into the app window. */
  get droppedBackInApp() {
    return _droppedBackInApp;
  },
};
