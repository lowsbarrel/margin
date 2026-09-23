import { ptyKill, ptyKillAll } from '$lib/terminal/bridge';

export interface TerminalTab {
	/** Frontend id; also the key of this shell's PTY on the Rust side. */
	id: number;
	/** Creation ordinal, so the tab label stays locale-reactive. */
	n: number;
	/** Exit code once the shell has ended, `null` while it runs. */
	exited: number | null;
	/** Why the shell could not be started; `null` when it did. */
	error: string | null;
}

const MIN_HEIGHT = 120;
/** The editor keeps the larger share: the panel never takes more than this. */
const MAX_WINDOW_SHARE = 0.8;
const DEFAULT_HEIGHT = 280;

let _nextId = 0;
let _nextN = 0;

let _tabs = $state<TerminalTab[]>([]);
let _activeId = $state<number | null>(null);
let _open = $state(false);
let _height = $state(DEFAULT_HEIGHT);

function clampHeight(height: number): number {
	const max = Math.max(MIN_HEIGHT, Math.round(window.innerHeight * MAX_WINDOW_SHARE));
	return Math.min(Math.max(Math.round(height), MIN_HEIGHT), max);
}

export const terminals = {
	get tabs(): TerminalTab[] {
		return _tabs;
	},

	get activeId(): number | null {
		return _activeId;
	},

	get open(): boolean {
		return _open;
	},

	get height(): number {
		return _height;
	},

	set height(height: number) {
		_height = clampHeight(height);
	},

	/** Open a shell and make it the visible one. */
	create(): void {
		const tab: TerminalTab = { id: _nextId++, n: ++_nextN, exited: null, error: null };
		_tabs = [..._tabs, tab];
		_activeId = tab.id;
	},

	activate(id: number): void {
		if (_tabs.some((tab) => tab.id === id)) _activeId = id;
	},

	/** Close one shell and its PTY. The panel goes with the last tab. */
	close(id: number): void {
		const index = _tabs.findIndex((tab) => tab.id === id);
		if (index < 0) return;
		const closed = _tabs[index];
		_tabs = _tabs.filter((tab) => tab.id !== id);
		if (_activeId === id) {
			// The neighbour to the left, or the new first tab.
			_activeId = _tabs[Math.min(index, _tabs.length - 1)]?.id ?? null;
		}
		if (_tabs.length === 0) _open = false;
		if (closed.exited === null && closed.error === null) {
			ptyKill(id).catch((err) => console.warn('Failed to close the terminal:', err));
		}
	},

	toggle(): void {
		if (_open) {
			_open = false;
			return;
		}
		_open = true;
		if (_tabs.length === 0) terminals.create();
	},

	/** The shell ended on its own; keep the tab so its output stays readable. */
	markExited(id: number, code: number): void {
		_tabs = _tabs.map((tab) => (tab.id === id ? { ...tab, exited: code } : tab));
	},

	markFailed(id: number, error: string): void {
		_tabs = _tabs.map((tab) => (tab.id === id ? { ...tab, error } : tab));
	},

	/** Locking a vault must not leave shells behind in it. */
	reset(): void {
		if (_tabs.length > 0) {
			ptyKillAll().catch((err) => console.warn('Failed to close terminals:', err));
		}
		_tabs = [];
		_activeId = null;
		_open = false;
	}
};
