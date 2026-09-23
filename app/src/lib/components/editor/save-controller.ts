import { writeFileBytes } from '$lib/fs/bridge';
import { saveSnapshot } from '$lib/history/bridge';
import { editor as editorStore } from '$lib/stores/editor.svelte';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';

const SAVE_DEBOUNCE_MS = 400;
const SNAPSHOT_INTERVAL_MS = 5 * 60 * 1000;

export interface SaveControllerHost {
	path(): string;
	vaultPath(): string | null;
	isAlive(): boolean;
	onsave(): ((content: string) => void) | undefined;
}

export class SaveController {
	private readonly host: SaveControllerHost;
	lastSavedText: string | null;
	private pendingText: string | null = null;
	private timer: ReturnType<typeof setTimeout> | undefined;
	private lastSnapshotTime = 0;
	private lastSnapshotMd: string | null = null;

	constructor(host: SaveControllerHost, initialText: string | null) {
		this.host = host;
		this.lastSavedText = initialText;
	}

	saveNow(text: string): void {
		if (!this.host.isAlive()) return;
		this.host.onsave()?.(text);
		const departing = this.lastSavedText;
		if (
			departing !== null &&
			departing !== text &&
			Date.now() - this.lastSnapshotTime >= SNAPSHOT_INTERVAL_MS
		) {
			this.snapshot(departing);
		}
		writeFileBytes(this.host.path(), new TextEncoder().encode(text))
			.then(() => {
				editorStore.setDirty(false);
				this.lastSavedText = text;
			})
			.catch((err) => {
				console.error('Save failed:', err);
				toast.error(m.toast_save_file_failed());
			});
	}

	schedule(text: string): void {
		this.pendingText = text;
		clearTimeout(this.timer);
		this.timer = setTimeout(() => {
			this.timer = undefined;
			const text = this.pendingText;
			this.pendingText = null;
			if (text != null) this.saveNow(text);
		}, SAVE_DEBOUNCE_MS);
	}

	flush(): void {
		clearTimeout(this.timer);
		this.timer = undefined;
		const text = this.pendingText;
		this.pendingText = null;
		if (text != null) this.saveNow(text);
	}

	cancel(): void {
		clearTimeout(this.timer);
		this.timer = undefined;
		this.pendingText = null;
	}

	snapshot(text: string): void {
		const vaultPath = this.host.vaultPath();
		if (!vaultPath || text === this.lastSnapshotMd) return;
		this.lastSnapshotTime = Date.now();
		this.lastSnapshotMd = text;
		saveSnapshot(vaultPath, this.host.path(), new TextEncoder().encode(text)).catch((err) => {
			console.warn('Snapshot save failed:', err);
			toast.error(m.toast_save_snapshot_failed());
		});
	}
}
