export type ClipboardOp = "copy" | "cut";

interface ClipboardData {
  paths: string[];
  isDirs: boolean[];
  operation: ClipboardOp;
}

let state = $state<ClipboardData | null>(null);

export const clipboard = {
  get paths() {
    return state?.paths ?? [];
  },
  get isDirs() {
    return state?.isDirs ?? [];
  },
  get operation() {
    return state?.operation ?? null;
  },
  get hasItems() {
    return state !== null && state.paths.length > 0;
  },

  copy(paths: string[], isDirs: boolean[]) {
    state = { paths: [...paths], isDirs: [...isDirs], operation: "copy" };
  },

  cut(paths: string[], isDirs: boolean[]) {
    state = { paths: [...paths], isDirs: [...isDirs], operation: "cut" };
  },

  clear() {
    state = null;
  },

  /** Return clipboard data and clear if it was a cut operation. */
  consume(): ClipboardData | null {
    const s = state;
    if (s?.operation === "cut") {
      state = null;
    }
    return s;
  },
};
