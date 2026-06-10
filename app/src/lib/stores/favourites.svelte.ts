import { SvelteSet } from "svelte/reactivity";
import { vault } from "$lib/stores/vault.svelte";

interface FavouritesState {
  paths: SvelteSet<string>;
}

let state = $state<FavouritesState>({
  paths: new SvelteSet(),
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

// Absolute-path list/Set are recomputed only when state.paths (or the vault
// path they depend on) changes — SvelteSet mutations (add/delete/clear) are
// reactive, so reading state.paths inside these $derived re-runs them. Repeated
// reads in a render return a stable identity instead of allocating + remapping
// a fresh container every access.
const absoluteList = $derived([...state.paths].map(toAbsolute));
const absolutePaths = $derived(new Set(absoluteList));

export const favourites = {
  /** Absolute paths for display/use by components */
  get paths(): Set<string> {
    return absolutePaths;
  },

  /** Stable derived array of absolute paths for template iteration. */
  get list(): string[] {
    return absoluteList;
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
    persist();
  },

  add(path: string) {
    state.paths.add(toRelative(path));
    persist();
  },

  remove(path: string) {
    state.paths.delete(toRelative(path));
    persist();
  },

  renamePath(oldPath: string, newPath: string) {
    const oldRel = toRelative(oldPath);
    if (!state.paths.has(oldRel)) return;
    state.paths.delete(oldRel);
    state.paths.add(toRelative(newPath));
    persist();
  },

  removePath(path: string) {
    const rel = toRelative(path);
    if (!state.paths.has(rel)) return;
    state.paths.delete(rel);
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
          state.paths.clear();
          for (const p of items) {
            const norm = p.replaceAll("\\", "/");
            state.paths.add(
              vp && norm.startsWith(vp + "/") ? norm.slice(vp.length + 1) : norm,
            );
          }
          return;
        }
      }
    } catch { /* ignore */ }
    state.paths.clear();
  },

  clear() {
    state.paths.clear();
  },
};
