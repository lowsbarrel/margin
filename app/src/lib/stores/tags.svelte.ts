import { listAllTags, type TagInfo } from '$lib/fs/bridge';

interface TagsState {
	loading: boolean;
	vaultPath: string | null;
}

const state = $state<TagsState>({
	loading: false,
	vaultPath: null
});

let items = $state.raw<TagInfo[]>([]);

export const tags = {
	get items() {
		return items;
	},
	get loading() {
		return state.loading;
	},

	async load(vaultPath: string): Promise<TagInfo[]> {
		if (state.vaultPath === vaultPath && items.length > 0) {
			return items;
		}
		state.loading = true;
		try {
			items = await listAllTags(vaultPath);
			state.vaultPath = vaultPath;
			return items;
		} catch (err) {
			console.warn('Failed to load tags:', err);
			return items;
		} finally {
			state.loading = false;
		}
	},

	async refresh(vaultPath: string): Promise<TagInfo[]> {
		state.vaultPath = null;
		return this.load(vaultPath);
	},

	clear() {
		items = [];
		state.vaultPath = null;
		state.loading = false;
	}
};
