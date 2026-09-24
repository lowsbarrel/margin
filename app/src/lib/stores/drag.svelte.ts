export type DragItem =
	| { kind: 'file'; path: string; label: string; isDir: boolean }
	| { kind: 'tab'; paneIndex: number; tabIndex: number; label: string; pinned: boolean };

interface DragState {
	item: DragItem;
	x: number;
	y: number;
}

export interface PendingInsert {
	path: string;
	x: number;
	y: number;
}

export interface StripTarget {
	paneIndex: number;
	index: number;
}

let state = $state<DragState | null>(null);
let insertState = $state<PendingInsert | null>(null);
let stripState = $state<StripTarget | null>(null);

let _nativeDragActive = false;

// An OS drag starts no pointer drag, so nothing else marks the hovered row.
let _externalDropTarget = $state<string | null>(null);

export const drag = {
	get active() {
		return state !== null;
	},
	get item() {
		return state?.item ?? null;
	},
	get x() {
		return state?.x ?? 0;
	},
	get y() {
		return state?.y ?? 0;
	},
	get pendingInsert() {
		return insertState;
	},
	get stripTarget() {
		return stripState;
	},
	setStripTarget(paneIndex: number, index: number) {
		stripState = { paneIndex, index };
	},
	clearStripTarget() {
		stripState = null;
	},
	start(item: DragItem, x: number, y: number) {
		state = { item, x, y };
	},
	move(x: number, y: number) {
		if (state) {
			state.x = x;
			state.y = y;
		}
	},
	end() {
		state = null;
		stripState = null;
	},
	requestInsertAtCoords(path: string, x: number, y: number) {
		insertState = { path, x, y };
	},
	clearPendingInsert() {
		insertState = null;
	},
	startNativeDrag() {
		_nativeDragActive = true;
	},
	endNativeDrag() {
		_nativeDragActive = false;
	},
	get nativeDragActive() {
		return _nativeDragActive;
	},
	get externalDropTarget() {
		return _externalDropTarget;
	},
	setExternalDropTarget(path: string | null) {
		_externalDropTarget = path;
	}
};
