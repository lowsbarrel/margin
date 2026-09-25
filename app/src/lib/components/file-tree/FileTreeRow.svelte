<script lang="ts">
	import { ChevronRight, FileText, Folder, FolderOpen } from '@lucide/svelte';
	import type { TreeEntry } from '$lib/fs/bridge';
	import * as m from '$lib/paraglide/messages.js';
	import { files } from '$lib/stores/files.svelte';
	import { focusOnMount, useInlineEdit } from '$lib/utils/inline-edit.svelte';
	import { parentDir } from '$lib/utils/path';
	import { INLINE_INPUT, ROW_BASE, fileRowClass, folderRowClass } from './row-classes';
	import { selectStem } from './tree-rename';
	import type { VisibleItem } from './tree-window.svelte';

	type InlineEdit = ReturnType<typeof useInlineEdit>;

	interface Props {
		item: VisibleItem;
		activeFile: string | null;
		focused: boolean;
		dropTarget: boolean;
		newFolderDepth: number;
		newFolderEdit: InlineEdit;
		renameEdit: InlineEdit;
		onrowclick: (row: TreeEntry, event: MouseEvent) => void;
		onrowcontextmenu: (row: TreeEntry, event: MouseEvent) => void;
		onrowdragstart: (row: TreeEntry, event: MouseEvent) => void;
	}

	let {
		item,
		activeFile,
		focused,
		dropTarget,
		newFolderDepth,
		newFolderEdit,
		renameEdit,
		onrowclick,
		onrowcontextmenu,
		onrowdragstart
	}: Props = $props();
</script>

{#if item.kind === 'new-folder'}
	<div class="{ROW_BASE} text-muted-foreground" style="padding-left: {newFolderDepth * 16 + 8}px;">
		<Folder size={16} />
		<input
			class={INLINE_INPUT}
			use:focusOnMount
			placeholder={m.folder_name_placeholder()}
			onfocus={(e) => e.currentTarget.select()}
			onkeydown={newFolderEdit.handleKeydown}
			onblur={newFolderEdit.handleBlur}
		/>
	</div>
{:else}
	{@const row = item.row}
	{@const indent = row.depth * 16}
	{@const expanded = files.expandedFolders.has(row.path)}
	{@const renaming = files.renamingPath === row.path}
	{#if row.is_dir}
		{#if renaming}
			<div class={ROW_BASE} style="padding-left: {indent + 8}px;">
				{#if expanded}<FolderOpen size={16} />{:else}<Folder size={16} />{/if}
				<input
					class={INLINE_INPUT}
					use:focusOnMount
					value={row.name}
					onfocus={(e) => selectStem(e.currentTarget)}
					onkeydown={renameEdit.handleKeydown}
					onblur={renameEdit.handleBlur}
				/>
			</div>
		{:else}
			<button
				class={folderRowClass(files.isSelected(row.path), dropTarget, focused)}
				style="padding-left: {indent + 4}px;"
				role="treeitem"
				aria-selected={files.isSelected(row.path)}
				aria-expanded={expanded}
				aria-level={row.depth + 1}
				tabindex={focused ? 0 : -1}
				data-path={row.path}
				data-drop-kind="folder"
				data-drop-path={row.path}
				onmousedown={(e) => onrowdragstart(row, e)}
				onclick={(e) => onrowclick(row, e)}
				oncontextmenu={(e) => onrowcontextmenu(row, e)}
			>
				<span
					class="flex shrink-0 items-center text-subtle-foreground transition-transform {expanded
						? 'rotate-90'
						: ''}"><ChevronRight size={14} /></span
				>
				{#if expanded}<FolderOpen size={16} />{:else}<Folder size={16} />{/if}
				<span class="min-w-0 flex-1 truncate">{row.name}</span>
			</button>
		{/if}
	{:else if renaming}
		<div class={ROW_BASE} style="padding-left: {indent + 8}px;">
			<FileText size={16} />
			<input
				class={INLINE_INPUT}
				use:focusOnMount
				value={row.name}
				onfocus={(e) => selectStem(e.currentTarget)}
				onkeydown={renameEdit.handleKeydown}
				onblur={renameEdit.handleBlur}
			/>
		</div>
	{:else}
		<button
			class={fileRowClass(activeFile === row.path, files.isSelected(row.path), focused)}
			style="padding-left: {indent + 8}px;"
			role="treeitem"
			aria-selected={files.isSelected(row.path)}
			aria-level={row.depth + 1}
			tabindex={focused ? 0 : -1}
			data-path={row.path}
			data-drop-kind="file"
			data-drop-path={row.path}
			data-drop-parent={parentDir(row.path)}
			onmousedown={(e) => onrowdragstart(row, e)}
			onclick={(e) => onrowclick(row, e)}
			oncontextmenu={(e) => onrowcontextmenu(row, e)}
		>
			<FileText size={16} />
			<span class="min-w-0 flex-1 truncate">{row.name.replace(/\.(md|canvas)$/, '')}</span>
		</button>
	{/if}
{/if}

<style>
	.tree-row.is-focused {
		outline: 2px solid var(--color-border-focus);
		outline-offset: -2px;
	}
</style>
