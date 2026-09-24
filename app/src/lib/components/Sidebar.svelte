<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import ContextMenu from './ContextMenu.svelte';
	import FileTree from './FileTree.svelte';
	import SidebarHeader from './sidebar/SidebarHeader.svelte';
	import { files } from '$lib/stores/files.svelte';
	import { installExternalDropRouter } from '$lib/utils/external-drop';
	import { createSidebarKeyboard } from './sidebar/sidebar-keyboard';
	import { createTreeActions } from './sidebar/tree-actions';
	import { createTreeMenu } from './sidebar/tree-menu.svelte';

	interface Props {
		onfileselect: (path: string, searchText?: string) => void;
		onrenameentry: (from: string, to: string, isDir: boolean) => Promise<void>;
		ondeleteentry: (path: string, isDir: boolean) => Promise<void>;
		panelOpen: boolean;
		panelWidth?: number;
	}

	let {
		onfileselect,
		onrenameentry,
		ondeleteentry,
		panelOpen,
		panelWidth = $bindable(280)
	}: Props = $props();

	const actions = createTreeActions({
		renameEntry: (from, to, isDir) => onrenameentry(from, to, isDir),
		deleteEntry: (path, isDir) => ondeleteentry(path, isDir)
	});
	const menu = createTreeMenu(actions);
	const keyboard = createSidebarKeyboard({ isPanelOpen: () => panelOpen, actions });

	const PANEL_MIN = 180;
	const PANEL_MAX = 480;
	let resizing = $state(false);
	let unlistenExternalDrop: (() => void) | null = null;
	let dragDropDisposed = false;

	function onResizeStart(e: MouseEvent) {
		e.preventDefault();
		resizing = true;
		const startX = e.clientX;
		const startW = panelWidth;

		function onMove(ev: MouseEvent) {
			const raw = startW + (ev.clientX - startX);
			panelWidth = Math.max(PANEL_MIN, Math.min(PANEL_MAX, raw));
		}

		function onUp() {
			resizing = false;
			window.removeEventListener('mousemove', onMove);
			window.removeEventListener('mouseup', onUp);
		}

		window.addEventListener('mousemove', onMove);
		window.addEventListener('mouseup', onUp);
	}

	onMount(() => {
		window.addEventListener('keydown', keyboard.handleKeydown);
		installExternalDropRouter().then((unlisten) => {
			if (dragDropDisposed) unlisten();
			else unlistenExternalDrop = unlisten;
		});
	});

	onDestroy(() => {
		window.removeEventListener('keydown', keyboard.handleKeydown);
		dragDropDisposed = true;
		unlistenExternalDrop?.();
	});
</script>

{#if panelOpen}
	<aside
		class="relative flex flex-col overflow-hidden border-r border-border bg-background {resizing
			? 'select-none'
			: ''}"
		style="width:{panelWidth}px;min-width:{panelWidth}px"
	>
		<SidebarHeader />

		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="panel-content" data-drop-kind="tree-root" oncontextmenu={menu.openRoot}>
			<FileTree
				activeFile={files.activeFile}
				{onfileselect}
				oncontextmenuentry={menu.openEntry}
				onrename={actions.rename}
				onmoveentry={actions.moveEntry}
				ondeleterow={actions.deleteSelection}
			/>
		</div>

		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="absolute top-0 right-0 z-10 h-full w-[6px] cursor-col-resize transition-colors {resizing
				? 'bg-brand/50'
				: 'hover:bg-brand/25'}"
			onmousedown={onResizeStart}
		></div>
	</aside>
{/if}

{#if menu.target && menu.items.length > 0}
	<ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={menu.close} />
{/if}
