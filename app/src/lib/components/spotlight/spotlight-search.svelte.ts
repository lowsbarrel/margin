import { searchFiles, searchIndex, type FsEntry, type SearchHit } from '$lib/fs/bridge';

export const MAX_CONTENT_RESULTS = 100;
const MAX_NAME_RESULTS = 8;
const DEBOUNCE_MS = 90;

export class SpotlightSearch {
	names = $state<FsEntry[]>([]);
	contents = $state<SearchHit[]>([]);
	searching = $state(false);

	#onResults: () => void;
	#timer: ReturnType<typeof setTimeout> | undefined;
	#generation = 0;

	constructor(onResults: () => void) {
		this.#onResults = onResults;
	}

	clear() {
		this.#generation++;
		this.names = [];
		this.contents = [];
		this.searching = false;
	}

	input(vaultPath: string | null, query: string) {
		const trimmed = query.trim();
		if (!vaultPath || !trimmed || trimmed.startsWith('#') || trimmed.startsWith('?')) {
			this.clear();
			return;
		}
		clearTimeout(this.#timer);
		this.searching = true;
		this.#timer = setTimeout(() => void this.rerun(vaultPath, trimmed), DEBOUNCE_MS);
	}

	async rerun(vaultPath: string | null, query: string) {
		if (this.#timer) {
			clearTimeout(this.#timer);
			this.#timer = undefined;
		}
		const trimmed = query.trim();
		if (!vaultPath || !trimmed) {
			this.clear();
			return;
		}
		await this.#run(vaultPath, trimmed);
	}

	async #run(vaultPath: string, trimmed: string) {
		const generation = ++this.#generation;
		this.searching = true;
		try {
			const [names, hits] = await Promise.all([
				searchFiles(vaultPath, trimmed),
				searchIndex(vaultPath, trimmed, MAX_CONTENT_RESULTS).catch(() => [] as SearchHit[])
			]);
			if (generation !== this.#generation) return;
			this.names = names.slice(0, MAX_NAME_RESULTS);
			this.contents = hits;
			this.#onResults();
		} catch (err) {
			if (generation !== this.#generation) return;
			console.warn('Spotlight search failed:', err);
			this.names = [];
			this.contents = [];
		} finally {
			if (generation === this.#generation) this.searching = false;
		}
	}

	dispose() {
		clearTimeout(this.#timer);
		this.#timer = undefined;
	}
}
