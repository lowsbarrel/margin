import { files } from '$lib/stores/files.svelte';

export class TreeFocus {
	path = $state<string | null>(null);
	keyboard = $state(false);

	targetPath(): string | null {
		const rows = files.flatTree;
		if (this.path && rows.some((row) => row.path === this.path)) return this.path;
		return rows[0]?.path ?? null;
	}
}
