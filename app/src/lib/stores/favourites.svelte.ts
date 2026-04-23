import { vault } from "$lib/stores/vault.svelte";

interface FavouritesState {
  paths: Set<string>;
}

let state = $state<FavouritesState>({
  paths: new Set(),
});

function storageKey(): string | null {
  const id = vault.vaultId;
  return id ? `margin-favourites-${id}` : null;
}

function persist() {
  const key = storageKey();
  if (!key || typeof localStorage === "undefined") return;
  localStorage.setItem(key, JSON.stringify([...state.paths]));
}

function toRelative(absPath: string): string {
  const vp = vault.vaultPath;
  if (vp && absPath.startsWith(vp + "/")) {
    return absPath.slice(vp.length + 1);
  }
  return absPath;
}

function toAbsolute(relPath: string): string {
  const vp = vault.vaultPath;
  if (vp && !relPath.startsWith("/")) {
    return `${vp}/${relPath}`;
  }
  return relPath;
}

export const favourites = {
  /** Absolute paths for display/use by components */
  get paths(): Set<string> {
    return new Set([...state.paths].map(toAbsolute));
  },

  isFavourite(path: string): boolean {
    return state.paths.has(toRelative(path));
  },

  toggle(path: string) {
    const rel = toRelative(path);
    if (state.paths.has(rel)) {
      state.paths.delete(rel);
    } else {
      state.paths.add(rel);
    }
    state.paths = new Set(state.paths);
    persist();
  },

  add(path: string) {
    state.paths.add(toRelative(path));
    state.paths = new Set(state.paths);
    persist();
  },

  remove(path: string) {
    state.paths.delete(toRelative(path));
    state.paths = new Set(state.paths);
    persist();
  },

  renamePath(oldPath: string, newPath: string) {
    const oldRel = toRelative(oldPath);
    if (!state.paths.has(oldRel)) return;
    state.paths.delete(oldRel);
    state.paths.add(toRelative(newPath));
    state.paths = new Set(state.paths);
    persist();
  },

  removePath(path: string) {
    const rel = toRelative(path);
    if (!state.paths.has(rel)) return;
    state.paths.delete(rel);
    state.paths = new Set(state.paths);
    persist();
  },

  load() {
    const key = storageKey();
    if (!key || typeof localStorage === "undefined") return;
    try {
      const raw = localStorage.getItem(key);
      if (raw) {
        const arr = JSON.parse(raw);
        if (Array.isArray(arr)) {
          const items = arr.filter((p): p is string => typeof p === "string");
          const vp = vault.vaultPath;
          state.paths = new Set(
            items.map((p) => {
              const norm = p.replaceAll("\\", "/");
              return vp && norm.startsWith(vp + "/") ? norm.slice(vp.length + 1) : norm;
            }),
          );
          return;
        }
      }
    } catch { /* ignore */ }
    state.paths = new Set();
  },

  clear() {
    state.paths = new Set();
  },
};
