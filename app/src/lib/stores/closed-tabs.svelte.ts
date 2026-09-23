import type { Tab } from '$lib/stores/panes.svelte';

const MAX_CLOSED_HISTORY = 10;

let closed = $state<string[]>([]);

export const closedTabs = {
	get count(): number {
		return closed.length;
	},

	push(tab: Tab): void {
		closed = [tab.path, ...closed.filter((p) => p !== tab.path)].slice(0, MAX_CLOSED_HISTORY);
	},

	take(): string | undefined {
		const [first, ...rest] = closed;
		closed = rest;
		return first;
	},

	clear(): void {
		closed = [];
	}
};
