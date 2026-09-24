import type { EditorView } from '@codemirror/view';

export type SyncStatus = 'idle' | 'syncing' | 'synced' | 'error';

export interface SyncProgress {
	total: number;
	done: number;
}

interface EditorState {
	syncStatus: SyncStatus;
	syncProgress: SyncProgress | null;
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

let activeView = $state<EditorView | null>(null);

function noteLocalEdit() {
	if (state.syncStatus === 'synced') {
		state.syncStatus = 'idle';
	}
	if (state.syncStatus === 'syncing') {
		state.localChangeDuringSync = true;
	}
}

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
	get view() {
		return activeView;
	},

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
		if (dirty) noteLocalEdit();
	},
	markLocalChange() {
		noteLocalEdit();
	},
	setView(instance: EditorView | null) {
		activeView = instance;
	},
	// A hidden editor deactivating must not unregister the one that replaced it.
	releaseView(instance: EditorView | null) {
		if (instance && activeView === instance) activeView = null;
	}
};
