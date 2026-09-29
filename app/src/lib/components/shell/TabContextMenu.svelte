<script lang="ts">
	import ContextMenu from '$lib/components/ContextMenu.svelte';
	import type { ContextMenuItem } from '$lib/components/ContextMenu.svelte';
	import { panes } from '$lib/stores/panes.svelte';
	import * as m from '$lib/paraglide/messages.js';

	let {
		x,
		y,
		paneIndex,
		tabIndex,
		onclose
	}: {
		x: number;
		y: number;
		paneIndex: number;
		tabIndex: number;
		onclose: () => void;
	} = $props();

	function tabMenuItems(): ContextMenuItem[] {
		const pane = panes.list[paneIndex];
		const tab = pane.tabs[tabIndex];
		return [
			{
				label: tab?.pinned ? m.tab_unpin() : m.tab_pin(),
				onclick: () => panes.togglePin(paneIndex, tabIndex)
			},
			{ label: m.tab_close(), onclick: () => panes.closeTab(paneIndex, tabIndex) },
			{
				label: m.tab_close_others(),
				onclick: () => panes.closeOtherTabs(paneIndex, tabIndex),
				disabled: pane.tabs.length <= 1
			},
			{ label: m.tab_close_all(), onclick: () => panes.closeAllTabs(paneIndex) },
			{
				label: m.tab_reopen_closed(),
				onclick: () => {
					panes.reopenClosedTab();
				},
				disabled: !panes.canReopenClosedTab
			}
		];
	}
</script>

<ContextMenu {x} {y} items={tabMenuItems()} {onclose} />
