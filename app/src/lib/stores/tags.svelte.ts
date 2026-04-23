import { listAllTags, type TagInfo } from "$lib/fs/bridge";

interface TagsState {
  items: TagInfo[];
  loading: boolean;
  vaultPath: string | null;
}

let state = $state<TagsState>({
  items: [],
  loading: false,
  vaultPath: null,
});

export const tags = {
  get items() {
    return state.items;
  },
  get loading() {
    return state.loading;
  },

  async load(vaultPath: string): Promise<TagInfo[]> {
    if (state.vaultPath === vaultPath && state.items.length > 0) {
      return state.items;
    }
    state.loading = true;
    try {
      state.items = await listAllTags(vaultPath);
      state.vaultPath = vaultPath;
      return state.items;
    } catch (err) {
      console.warn("Failed to load tags:", err);
      return state.items;
    } finally {
      state.loading = false;
    }
  },

  async refresh(vaultPath: string): Promise<TagInfo[]> {
    state.vaultPath = null;
    return this.load(vaultPath);
  },

  clear() {
    state.items = [];
    state.vaultPath = null;
    state.loading = false;
  },
};
