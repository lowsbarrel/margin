<script lang="ts">
	import { onMount } from 'svelte';
	import type { TreeEntry } from '$lib/fs/bridge';
	import { createDirectory } from '$lib/fs/bridge';
	import { editor } from '$lib/stores/editor.svelte';
	import { files, type TreeRevealTarget } from '$lib/stores/files.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { favourites } from '$lib/stores/favourites.svelte';
	import { drag } from '$lib/stores/drag.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import * as m from '$lib/paraglide/messages.js';
	import { validateName } from '$lib/utils/filename';
	import { resolveResource } from '@tauri-apps/api/path';
	import {
		isDescendantOrSelf,
		handleFolderDrop as doFolderDrop,
		handleRootDrop as doRootDrop,
		tryNativeDrag,
		startDragEntry
	} from '$lib/utils/file-tree-drag';
	import { ChevronRight, Folder, FolderOpen, FileText, Star } from '@lucide/svelte';

	interface Props {
		activeFile: string | null;
		onfileselect: (path: string) => void;
		oncontextmenuentry: (entry: TreeEntry, event: MouseEvent) => void;
		onrename: (entry: TreeEntry, newName: string) => void;
		onmoveentry: (fromPath: string, toDir: string, isDir: boolean) => Promise<void>;
		ondeleteentry: (path: string, isDir: boolean) => Promise<void>;
	}

	let {
		activeFile,
		onfileselect,
		oncontextmenuentry,
		onrename,
		onmoveentry,
		ondeleteentry
	}: Props = $props();

	let dropTargetFolder = $state<string | null>(null);

	// ── Row chrome ───────────────────────────────────────────────────────────
	// `.tree-row` carries no styling any more — it survives purely as the hook
	// the viewport's click handler uses to tell "clicked a row" from "clicked
	// the empty gutter". `h-8` is ROW_HEIGHT and must stay in lockstep with it,
	// or the virtual-scroll maths and the painted rows drift apart.
	//
	// `pr-3!` (and the weight/padding importants below): app.css's base
	// `button` rule is unlayered, so it outranks everything Tailwind emits into
	// `@layer utilities`. On a <button> those declarations only stick when
	// marked important.
	const ROW_BASE =
		'tree-row relative flex h-8 w-full items-center gap-2 overflow-hidden pr-3 text-sm tracking-normal [&_svg]:shrink-0';
	const ROW_BUTTON = `${ROW_BASE} cursor-pointer rounded-sm text-left transition-colors`;
	const ROW_QUIET = 'font-normal text-muted-foreground hover:bg-surface-1 hover:text-foreground';
	const ROW_SELECTED = 'font-normal bg-surface-2 text-foreground';

	function folderRowClass(selected: boolean, isDropTarget: boolean) {
		if (isDropTarget) {
			return `${ROW_BUTTON} font-normal bg-brand/24 text-foreground shadow-[inset_0_0_0_1px_var(--color-border-focus)]`;
		}
		return `${ROW_BUTTON} ${selected ? ROW_SELECTED : ROW_QUIET}`;
	}

	// The open file is the one row that earns brand colour: a 2px orange rail
	// plus medium weight, so it stays legible even when another row is selected.
	function fileRowClass(active: boolean, selected: boolean) {
		if (active) {
			return `${ROW_BUTTON} font-medium bg-accent text-foreground [&_svg]:text-accent-foreground before:absolute before:top-1.5 before:bottom-1.5 before:left-0 before:w-0.5 before:rounded-full before:bg-brand before:content-['']`;
		}
		return `${ROW_BUTTON} ${selected ? ROW_SELECTED : ROW_QUIET}`;
	}

	const INLINE_INPUT =
		'h-6 min-w-0 flex-1 rounded-sm border-ring bg-background px-2 py-0.5 text-sm text-foreground shadow-[0_0_0_3px_var(--color-brand-16)] outline-none';

	// Virtual scroll: only visible rows are in the DOM.
	const ROW_HEIGHT = 32;
	const OVERSCAN = 5;

	let viewport = $state<HTMLDivElement | undefined>();
	let scrollTop = $state(0);
	let viewportHeight = $state(0);

	let rows = $derived(files.flatTree);

	// New-folder row inserted into virtual scroll flow
	let newFolderInsertIdx = $derived.by(() => {
		const parent = files.pendingNewFolder;
		if (!parent) return -1;
		const idx = rows.findIndex((r) => r.path === parent);
		if (idx === -1) return 0;
		const depth = rows[idx].depth;
		let at = idx + 1;
		while (at < rows.length && rows[at].depth > depth) at++;
		return at;
	});

	let newFolderDepth = $derived.by(() => {
		const parent = files.pendingNewFolder;
		if (!parent || !vault.vaultPath) return 0;
		if (parent === vault.vaultPath) return 0;
		const idx = rows.findIndex((r) => r.path === parent);
		return idx >= 0 ? rows[idx].depth + 1 : 0;
	});

	let virtualCount = $derived(rows.length + (newFolderInsertIdx >= 0 ? 1 : 0));
	let totalHeight = $derived(virtualCount * ROW_HEIGHT);

	let visibleStart = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN));
	let visibleEnd = $derived(
		Math.min(virtualCount, Math.ceil((scrollTop + viewportHeight) / ROW_HEIGHT) + OVERSCAN)
	);
	let slabTop = $derived(visibleStart * ROW_HEIGHT);

	type VisibleItem = { kind: 'row'; row: (typeof rows)[0] } | { kind: 'new-folder' };

	let visibleItems = $derived.by(() => {
		const items: VisibleItem[] = [];
		const insertIdx = newFolderInsertIdx;
		for (let vi = visibleStart; vi < visibleEnd; vi++) {
			if (vi === insertIdx) {
				items.push({ kind: 'new-folder' });
			} else {
				const ri = insertIdx >= 0 && vi > insertIdx ? vi - 1 : vi;
				if (ri >= 0 && ri < rows.length) {
					items.push({ kind: 'row', row: rows[ri] });
				}
			}
		}
		return items;
	});

	let handledTreeRevealVersion = $state(0);

	function getVirtualIndex(target: TreeRevealTarget) {
		const insertIdx = newFolderInsertIdx;
		if (target.kind === 'pending-new-folder') {
			return files.pendingNewFolder === target.parentPath ? insertIdx : -1;
		}

		const rowIndex = rows.findIndex((row) => row.path === target.path);
		if (rowIndex < 0) return -1;
		return insertIdx >= 0 && rowIndex >= insertIdx ? rowIndex + 1 : rowIndex;
	}

	function scrollVirtualIndexIntoView(index: number) {
		if (!viewport) return false;
		const visibleHeight = viewport.clientHeight || viewportHeight;
		if (visibleHeight <= 0) return false;

		const rowTop = index * ROW_HEIGHT;
		const rowBottom = rowTop + ROW_HEIGHT;
		const currentTop = viewport.scrollTop;
		const currentBottom = currentTop + visibleHeight;
		let nextTop: number;

		if (rowTop < currentTop) {
			nextTop = rowTop;
		} else if (rowBottom > currentBottom) {
			nextTop = rowBottom - visibleHeight;
		} else {
			return true;
		}

		const maxTop = Math.max(0, totalHeight - visibleHeight);
		const boundedTop = Math.max(0, Math.min(maxTop, nextTop));
		viewport.scrollTop = boundedTop;
		scrollTop = boundedTop;
		return true;
	}

	$effect(() => {
		const target = files.treeRevealTarget;
		const version = files.treeRevealVersion;
		if (!target || !viewport || version === handledTreeRevealVersion) return;

		const targetIndex = getVirtualIndex(target);
		if (targetIndex < 0) return;
		if (scrollVirtualIndexIntoView(targetIndex)) {
			handledTreeRevealVersion = version;
		}
	});

	// Drag support
	let suppressNextClick = false;
	const nativeDragState = { started: false };

	let dragIconPath = '';
	onMount(async () => {
		try {
			dragIconPath = await resolveResource('icons/32x32.png');
		} catch {
			dragIconPath = '';
		}
	});

	$effect(() => {
		if (drag.active) {
			function onMove(e: MouseEvent) {
				tryNativeDrag(e.clientX, e.clientY, dragIconPath, ondeleteentry, nativeDragState);
			}
			window.addEventListener('mousemove', onMove);
			return () => {
				window.removeEventListener('mousemove', onMove);
			};
		}
	});

	function startDrag(e: MouseEvent, entry: TreeEntry) {
		startDragEntry(e, entry, () => {
			suppressNextClick = true;
		});
	}

	function handleFolderDrop(e: MouseEvent, folderPath: string) {
		doFolderDrop(e, folderPath, onmoveentry, (v) => {
			dropTargetFolder = v;
		});
	}

	function handleRootDrop(e: MouseEvent) {
		if (!vault.vaultPath) return;
		doRootDrop(e, vault.vaultPath, onmoveentry, (v) => {
			dropTargetFolder = v;
		});
	}

	// Inline editing
	function submitRename(entry: TreeEntry, input: HTMLInputElement) {
		const newName = input.value.trim();
		files.cancelRename();
		if (newName && newName !== entry.name) onrename(entry, newName);
	}

	async function confirmNewFolder(name: string) {
		const parent = files.pendingNewFolder;
		if (!name.trim() || !parent || !vault.vaultPath) {
			files.cancelNewFolder();
			return;
		}
		const trimmed = name.trim();
		const error = validateName(trimmed);
		if (error) {
			toast.error(error);
			files.cancelNewFolder();
			return;
		}
		const folderPath = `${parent}/${trimmed}`;
		await createDirectory(folderPath);
		files.cancelNewFolder();
		await files.expandFolder(folderPath);
		files.setSelectedFolder(folderPath);
		await files.refresh();
		files.requestTreeReveal(folderPath);
		editor.markLocalChange();
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
	class="relative h-full overflow-x-hidden overflow-y-auto"
	bind:this={viewport}
	bind:clientHeight={viewportHeight}
	onscroll={() => {
		if (viewport) scrollTop = viewport.scrollTop;
	}}
	onclick={(e) => {
		if (!(e.target as HTMLElement).closest('.tree-row') && vault.vaultPath) {
			files.clearSelection();
			files.setSelectedFolder(vault.vaultPath);
		}
	}}
	onmouseup={(e) => handleRootDrop(e)}
>
	<!-- Full-height spacer keeps the scrollbar proportionate -->
	<div class="relative w-full" style="height: {totalHeight}px;">
		<!-- Only the rows in [visibleStart, visibleEnd) are in the DOM -->
		<div class="absolute inset-x-0" style="top: {slabTop}px;">
			{#each visibleItems as item (item.kind === 'row' ? item.row.path : '__new_folder__')}
				{#if item.kind === 'new-folder'}
					<div
						class="{ROW_BASE} text-muted-foreground"
						style="padding-left: {newFolderDepth * 16 + 8}px;"
					>
						<Folder size={16} />
						<!-- svelte-ignore a11y_autofocus -->
						<input
							class={INLINE_INPUT}
							autofocus
							placeholder={m.folder_name_placeholder()}
							onblur={(e) => confirmNewFolder(e.currentTarget.value)}
							onkeydown={(e) => {
								if (e.key === 'Enter') {
									e.preventDefault();
									confirmNewFolder(e.currentTarget.value);
								}
								if (e.key === 'Escape') files.cancelNewFolder();
							}}
						/>
					</div>
				{:else}
					{@const row = item.row}
					{@const indent = row.depth * 16}
					{@const isExpanded = files.expandedFolders.has(row.path)}
					{@const isRenaming = files.renamingPath === row.path}

					{#if row.is_dir}
						{#if isRenaming}
							<div class={ROW_BASE} style="padding-left: {indent + 8}px;">
								{#if isExpanded}<FolderOpen size={16} />{:else}<Folder size={16} />{/if}
								<!-- svelte-ignore a11y_autofocus -->
								<input
									class={INLINE_INPUT}
									autofocus
									value={row.name}
									onblur={(e) => submitRename(row, e.currentTarget)}
									onkeydown={(e) => {
										if (e.key === 'Enter') {
											e.preventDefault();
											submitRename(row, e.currentTarget);
										}
										if (e.key === 'Escape') files.cancelRename();
									}}
								/>
							</div>
						{:else}
							<button
								class={folderRowClass(files.isSelected(row.path), dropTargetFolder === row.path)}
								style="padding-left: {indent + 4}px;"
								onmousedown={(e) => startDrag(e, row)}
								onclick={(e) => {
									if (suppressNextClick) {
										suppressNextClick = false;
										return;
									}
									if (e.metaKey || e.ctrlKey) {
										files.selectToggle(row.path, true);
									} else if (e.shiftKey) {
										files.selectRange(row.path);
									} else {
										files.selectSingle(row.path, true);
										files.setSelectedFolder(row.path);
										files.toggleFolder(row.path);
									}
								}}
								ondblclick={(e) => {
									e.preventDefault();
									e.stopPropagation();
									files.startRename(row.path);
								}}
								oncontextmenu={(e) => {
									e.preventDefault();
									e.stopPropagation();
									if (!files.isSelected(row.path)) {
										files.selectSingle(row.path, true);
									}
									oncontextmenuentry(row, e);
								}}
								onmouseenter={() => {
									if (
										drag.active &&
										drag.item?.kind === 'file' &&
										!isDescendantOrSelf(drag.item.path, row.path)
									) {
										dropTargetFolder = row.path;
									}
								}}
								onmouseleave={() => {
									if (dropTargetFolder === row.path) dropTargetFolder = null;
								}}
								onmouseup={(e) => handleFolderDrop(e, row.path)}
							>
								<span
									class="flex shrink-0 items-center text-subtle-foreground transition-transform {isExpanded
										? 'rotate-90'
										: ''}"><ChevronRight size={14} /></span
								>
								{#if isExpanded}<FolderOpen size={16} />{:else}<Folder size={16} />{/if}
								<span class="min-w-0 flex-1 truncate">{row.name}</span>
							</button>
						{/if}
					{:else if isRenaming}
						<div class={ROW_BASE} style="padding-left: {indent + 8}px;">
							<FileText size={16} />
							<!-- svelte-ignore a11y_autofocus -->
							<input
								class={INLINE_INPUT}
								autofocus
								value={row.name}
								onblur={(e) => submitRename(row, e.currentTarget)}
								onkeydown={(e) => {
									if (e.key === 'Enter') {
										e.preventDefault();
										submitRename(row, e.currentTarget);
									}
									if (e.key === 'Escape') files.cancelRename();
								}}
								onfocus={(e) => {
									const val = e.currentTarget.value;
									const dot = val.lastIndexOf('.');
									e.currentTarget.setSelectionRange(0, dot > 0 ? dot : val.length);
								}}
							/>
						</div>
					{:else}
						<button
							class={fileRowClass(activeFile === row.path, files.isSelected(row.path))}
							style="padding-left: {indent + 8}px;"
							onmousedown={(e) => startDrag(e, row)}
							onclick={(e) => {
								if (suppressNextClick) {
									suppressNextClick = false;
									return;
								}
								if (e.metaKey || e.ctrlKey) {
									files.selectToggle(row.path, false);
								} else if (e.shiftKey) {
									files.selectRange(row.path);
								} else {
									files.selectSingle(row.path, false);
									onfileselect(row.path);
								}
							}}
							ondblclick={(e) => {
								e.preventDefault();
								e.stopPropagation();
								files.startRename(row.path);
							}}
							oncontextmenu={(e) => {
								e.preventDefault();
								e.stopPropagation();
								if (!files.isSelected(row.path)) {
									files.selectSingle(row.path, false);
								}
								oncontextmenuentry(row, e);
							}}
						>
							<FileText size={16} />
							<span class="min-w-0 flex-1 truncate">{row.name.replace(/\.(md|canvas)$/, '')}</span>
							{#if favourites.isFavourite(row.path)}
								<Star size={12} class="ml-auto shrink-0 fill-current text-accent-foreground" />
							{/if}
						</button>
					{/if}
				{/if}
			{/each}
		</div>
	</div>
</div>
