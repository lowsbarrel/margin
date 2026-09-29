<script lang="ts">
	import { onMount } from 'svelte';
	import { resolveResource } from '@tauri-apps/api/path';
	import type { TreeEntry } from '$lib/fs/bridge';
	import * as m from '$lib/paraglide/messages.js';
	import { drag } from '$lib/stores/drag.svelte';
	import { files } from '$lib/stores/files.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import FileTreeRow from './file-tree/FileTreeRow.svelte';
	import { ROW_HEIGHT } from './file-tree/row-classes';
	import { TreeDrop } from './file-tree/tree-drop.svelte';
	import { TreeFocus } from './file-tree/tree-focus.svelte';
	import { createRenameFlow } from './file-tree/tree-rename';
	import { createTreeKeyboard } from './file-tree/tree-keyboard';
	import { TreeWindow } from './file-tree/tree-window.svelte';

	interface Props {
		activeFile: string | null;
		onfileselect: (path: string) => void;
		oncontextmenuentry: (entry: TreeEntry, event: MouseEvent) => void;
		onrename: (entry: TreeEntry, newName: string) => Promise<boolean>;
		onmoveentry: (fromPath: string, toDir: string, isDir: boolean) => Promise<void>;
		ondeleterow: (entry: TreeEntry) => void;
	}

	let { activeFile, onfileselect, oncontextmenuentry, onrename, onmoveentry, ondeleterow }: Props =
		$props();

	const treeWindow = new TreeWindow();
	const focus = new TreeFocus();
	const drop = new TreeDrop(focus);
	const { newFolderEdit, renameEdit } = createRenameFlow({
		onrename: (entry, name) => onrename(entry, name)
	});
	const keyboard = createTreeKeyboard({
		window: treeWindow,
		focus,
		onfileselect: (path) => onfileselect(path),
		ondeleterow: (entry) => ondeleterow(entry)
	});

	let focusedRow = $derived(focus.targetPath());
	let dragIconPath = $state('');

	onMount(async () => {
		try {
			dragIconPath = await resolveResource('icons/32x32.png');
		} catch {
			dragIconPath = '';
		}
	});

	$effect(() => {
		if (!drag.active) return;
		function onMove(e: MouseEvent) {
			drop.trackPointer(e.clientX, e.clientY, dragIconPath);
		}
		window.addEventListener('mousemove', onMove);
		return () => {
			window.removeEventListener('mousemove', onMove);
			drop.target = null;
		};
	});

	$effect(() => {
		treeWindow.reveal(files.treeRevealTarget, files.treeRevealVersion);
	});

	function handleRowClick(row: TreeEntry, event: MouseEvent) {
		focus.path = row.path;
		if (drop.takeSuppressedClick()) return;
		if (event.metaKey || event.ctrlKey) {
			files.selectToggle(row.path, row.is_dir);
		} else if (event.shiftKey) {
			files.selectRange(row.path);
		} else if (event.detail === 3) {
			files.startRename(row.path);
		} else if (row.is_dir) {
			files.selectSingle(row.path, true);
			files.setSelectedFolder(row.path);
			if (!drop.isRepeatClick(row.path)) void files.toggleFolder(row.path);
		} else {
			files.selectSingle(row.path, false);
			files.setSelectedFolder(null);
			onfileselect(row.path);
		}
	}

	function handleRowContextMenu(row: TreeEntry, event: MouseEvent) {
		event.preventDefault();
		event.stopPropagation();
		if (!files.isSelected(row.path)) files.selectSingle(row.path, row.is_dir);
		if (row.is_dir) files.setSelectedFolder(row.path);
		focus.path = row.path;
		oncontextmenuentry(row, event);
	}
</script>

<div
	class="relative h-full overflow-x-hidden overflow-y-auto"
	role="tree"
	aria-label={m.sidebar_explorer()}
	tabindex={-1}
	data-drop-kind="tree-root"
	bind:this={treeWindow.el}
	bind:clientHeight={treeWindow.clientHeight}
	onkeydown={keyboard.handleKeydown}
	onfocusout={(e) => {
		const next = e.relatedTarget as Node | null;
		if (!next || !treeWindow.el?.contains(next)) focus.keyboard = false;
	}}
	onmouseup={(e) => void drop.settle(e, onmoveentry)}
	onscroll={() => {
		if (treeWindow.el) treeWindow.scrollTop = treeWindow.el.scrollTop;
	}}
	onclick={(e) => {
		// Rows carry `.tree-row`; a click that misses them landed on the empty gutter.
		if (!(e.target as HTMLElement).closest('.tree-row') && vault.vaultPath) {
			files.clearSelection();
			files.setSelectedFolder(vault.vaultPath);
		}
	}}
>
	<div class="relative w-full" style="height: {treeWindow.totalHeight}px;">
		{#each treeWindow.renderedItems as item (item.kind === 'row' ? item.row.path : '__new_folder__')}
			<div class="absolute inset-x-0 h-8" style="top: {item.virtualIndex * ROW_HEIGHT}px;">
				<FileTreeRow
					{item}
					{activeFile}
					focused={focus.keyboard && item.kind === 'row' && item.row.path === focusedRow}
					dropTarget={item.kind === 'row' && drop.isTarget(item.row.path)}
					newFolderDepth={treeWindow.newFolderDepth}
					{newFolderEdit}
					{renameEdit}
					onrowclick={handleRowClick}
					onrowcontextmenu={handleRowContextMenu}
					onrowdragstart={(row, event) => drop.startPointerDrag(row, event)}
				/>
			</div>
		{/each}
	</div>
</div>
