import type { Editor } from '@tiptap/core';

export type SyncStatus = 'idle' | 'syncing' | 'synced' | 'error';

export interface SyncProgress {
	total: number;
	done: number;
}

interface EditorState {
	syncStatus: SyncStatus;
	syncProgress: SyncProgress | null;
	/**
	 * Why the last sync failed, for display on hover.
	 *
	 * The status enum alone could only ever say *that* sync broke, never why —
	 * the cause went to `console.error` and was then swallowed by the background
	 * runner, so from the UI a failed sync and a misconfigured one looked
	 * identical. Kept as a plain string so the status bar can surface it in a
	 * `title` without plumbing the error object through the view layer.
	 */
	syncError: string | null;
	cursorLine: number;
	cursorCol: number;
	dirty: boolean;
	localChangeDuringSync: boolean;
}

const state = $state<EditorState>({
	syncStatus: 'idle',
	syncProgress: null,
	syncError: null,
	cursorLine: 1,
	cursorCol: 1,
	dirty: false,
	localChangeDuringSync: false
});

let tiptapInstance = $state<Editor | null>(null);

export const editor = {
	get syncStatus() {
		return state.syncStatus;
	},
	get syncProgress() {
		return state.syncProgress;
	},
	get syncError() {
		return state.syncError;
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

	/**
	 * `reason` is only meaningful for `'error'`; any other status clears it, so a
	 * stale message can never outlive the failure it describes.
	 */
	setSyncStatus(status: SyncStatus, reason?: string) {
		if (status === 'synced' && state.localChangeDuringSync) {
			state.localChangeDuringSync = false;
			state.syncStatus = 'idle';
		} else {
			state.syncStatus = status;
		}
		if (status === 'syncing') {
			state.localChangeDuringSync = false;
		}
		if (status !== 'syncing') {
			state.syncProgress = null;
		}
		state.syncError = status === 'error' ? (reason ?? null) : null;
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
		if (dirty && state.syncStatus === 'synced') {
			state.syncStatus = 'idle';
		}
		if (dirty && state.syncStatus === 'syncing') {
			state.localChangeDuringSync = true;
		}
	},
	markLocalChange() {
		if (state.syncStatus === 'synced') {
			state.syncStatus = 'idle';
		}
		if (state.syncStatus === 'syncing') {
			state.localChangeDuringSync = true;
		}
	},
	setTiptap(instance: Editor | null) {
		tiptapInstance = instance;
	}
};
