<script lang="ts">
	import { onMount, tick } from 'svelte';
	import type { TreeEntry } from '$lib/fs/bridge';
	import { createDirectory, fileExists } from '$lib/fs/bridge';
	import { editor } from '$lib/stores/editor.svelte';
	import { files, type TreeRevealTarget } from '$lib/stores/files.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { drag } from '$lib/stores/drag.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import * as m from '$lib/paraglide/messages.js';
	import { validateName } from '$lib/utils/filename';
	import { resolveResource } from '@tauri-apps/api/path';
	import { hitTestDropZone } from '$lib/utils/drop-zone';
	import { moveEntriesInto, startDragEntry, tryNativeDrag } from '$lib/utils/file-tree-drag';
	import { useInlineEdit } from '$lib/utils/inline-edit.svelte';
	import { ChevronRight, Folder, FolderOpen, FileText } from '@lucide/svelte';

	interface Props {
		activeFile: string | null;
		onfileselect: (path: string) => void;
		oncontextmenuentry: (entry: TreeEntry, event: MouseEvent) => void;
		/** Resolves true when the rename went through; false keeps the input open. */
		onrename: (entry: TreeEntry, newName: string) => Promise<boolean>;
		onmoveentry: (fromPath: string, toDir: string, isDir: boolean) => Promise<void>;
		ondeleterow: (entry: TreeEntry) => void;
	}

	let { activeFile, onfileselect, oncontextmenuentry, onrename, onmoveentry, ondeleterow }: Props =
		$props();

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

	function folderRowClass(selected: boolean, dropTarget: boolean, focused: boolean) {
		const base = dropTarget
			? `${ROW_BUTTON} font-normal bg-brand/24 text-foreground shadow-[inset_0_0_0_1px_var(--color-border-focus)]`
			: `${ROW_BUTTON} ${selected ? ROW_SELECTED : ROW_QUIET}`;
		return focused ? `${base} is-focused` : base;
	}

	// The open file is the one row that earns brand colour: a 2px orange rail
	// plus medium weight, so it stays legible even when another row is selected.
	function fileRowClass(active: boolean, selected: boolean, focused: boolean) {
		const base = active
			? `${ROW_BUTTON} font-medium bg-accent text-foreground [&_svg]:text-accent-foreground before:absolute before:top-1.5 before:bottom-1.5 before:left-0 before:w-0.5 before:rounded-full before:bg-brand before:content-['']`
			: `${ROW_BUTTON} ${selected ? ROW_SELECTED : ROW_QUIET}`;
		return focused ? `${base} is-focused` : base;
	}

	const INLINE_INPUT =
		'h-6 min-w-0 flex-1 rounded-sm border-ring bg-background px-2 py-0.5 text-sm text-foreground shadow-[0_0_0_3px_var(--color-brand-16)] outline-none';

	// Virtual scroll: only visible rows are in the DOM.
	const ROW_HEIGHT = 32;
	const OVERSCAN = 5;
	const DOUBLE_CLICK_MS = 400;

	let viewport = $state<HTMLDivElement | undefined>();
	let scrollTop = $state(0);
	let viewportHeight = $state(0);
	let dropTargetFolder = $state<string | null>(null);
	let focusedPath = $state<string | null>(null);
	// The ring marks the row the keyboard is about to act on, so it shows only
	// after a keyboard interaction — the same contract as `:focus-visible`, made
	// explicit because the rows are focused programmatically by `setFocus`.
	let keyboardFocus = $state(false);

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

	type VisibleItem =
		| { kind: 'row'; row: (typeof rows)[0]; virtualIndex: number }
		| { kind: 'new-folder'; virtualIndex: number };

	let visibleItems = $derived.by((): VisibleItem[] => {
		const items: VisibleItem[] = [];
		const insertIdx = newFolderInsertIdx;
		for (let vi = visibleStart; vi < visibleEnd; vi++) {
			if (vi === insertIdx) {
				items.push({ kind: 'new-folder', virtualIndex: vi });
			} else {
				const ri = insertIdx >= 0 && vi > insertIdx ? vi - 1 : vi;
				if (ri >= 0 && ri < rows.length) {
					items.push({ kind: 'row', row: rows[ri], virtualIndex: vi });
				}
			}
		}
		return items;
	});

	function inVisibleRange(index: number): boolean {
		return index >= visibleStart && index < visibleEnd;
	}

	/**
	 * A row being inline-edited is pinned into the DOM even once it scrolls out
	 * of the rendered window. Unmounting a focused input reports it as a blur,
	 * which would commit (or cancel) a half-typed name the user never submitted.
	 */
	let pinnedItems = $derived.by((): VisibleItem[] => {
		const items: VisibleItem[] = [];
		const insertIdx = newFolderInsertIdx;
		if (files.pendingNewFolder && insertIdx >= 0 && !inVisibleRange(insertIdx)) {
			items.push({ kind: 'new-folder', virtualIndex: insertIdx });
		}
		const renaming = files.renamingPath;
		if (renaming) {
			const row = rows.find((r) => r.path === renaming);
			const index = row ? getVirtualIndex({ kind: 'entry', path: renaming }) : -1;
			if (row && index >= 0 && !inVisibleRange(index)) {
				items.push({ kind: 'row', row, virtualIndex: index });
			}
		}
		return items;
	});

	let renderedItems = $derived(
		[...visibleItems, ...pinnedItems].sort((a, b) => a.virtualIndex - b.virtualIndex)
	);

	// The row that owns the tree's tab stop. Roving tabindex: exactly one row is
	// reachable by Tab, and arrow keys move that stop, so a virtualized list of
	// thousands never lands in the tab order.
	let focusTargetPath = $derived.by(() => {
		if (focusedPath && rows.some((r) => r.path === focusedPath)) return focusedPath;
		return rows[0]?.path ?? null;
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

	// ── Drag ─────────────────────────────────────────────────────────────────

	// Suppresses the click that follows a completed pointer drag.
	let suppressNextClick = false;
	const nativeDragState = { started: false };

	// A multi-click fires one click per press. The folder toggle runs on the
	// first; without this the later clicks of the triple-click that renames would
	// collapse and re-expand the folder under the user.
	let lastClickPath = '';
	let lastClickAt = 0;

	function isSecondClick(path: string): boolean {
		const now = performance.now();
		const repeated = path === lastClickPath && now - lastClickAt < DOUBLE_CLICK_MS;
		lastClickPath = path;
		lastClickAt = now;
		return repeated;
	}

	let dragIconPath = '';
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
			tryNativeDrag(e.clientX, e.clientY, dragIconPath, nativeDragState);
			const zone = hitTestDropZone(e.clientX, e.clientY);
			dropTargetFolder = zone?.kind === 'folder' ? zone.path : null;
		}
		window.addEventListener('mousemove', onMove);
		return () => {
			window.removeEventListener('mousemove', onMove);
			dropTargetFolder = null;
		};
	});

	function isDropTarget(path: string): boolean {
		return dropTargetFolder === path || drag.externalDropTarget === path;
	}

	function startDrag(e: MouseEvent, entry: TreeEntry) {
		keyboardFocus = false;
		startDragEntry(e, entry, () => {
			suppressNextClick = true;
		});
	}

	function parentDir(path: string): string {
		return path.slice(0, path.lastIndexOf('/'));
	}

	/**
	 * One drop resolver for in-app drags: hit-test the point the pointer was
	 * released at, rather than remembering which row the pointer last crossed.
	 * The target is therefore exactly what the user saw under the cursor — a
	 * folder row is that folder, a file row is the folder holding it, and the
	 * tree's own background is the vault root.
	 */
	async function handleTreeDrop(e: MouseEvent) {
		if (!drag.active || drag.item?.kind !== 'file') return;
		const zone = hitTestDropZone(e.clientX, e.clientY);
		const folderPath =
			zone?.kind === 'folder' || zone?.kind === 'parent'
				? zone.path
				: zone?.kind === 'tree-root'
					? vault.vaultPath
					: null;
		if (!folderPath) return;
		e.stopPropagation();
		await moveEntriesInto(folderPath, onmoveentry, (value) => {
			dropTargetFolder = value;
		});
	}

	// ── Keyboard navigation ──────────────────────────────────────────────────

	async function setFocus(path: string) {
		focusedPath = path;
		const index = getVirtualIndex({ kind: 'entry', path });
		if (index >= 0) scrollVirtualIndexIntoView(index);
		await tick();
		viewport?.querySelector<HTMLElement>(`[data-path="${CSS.escape(path)}"]`)?.focus();
	}

	function activateRow(row: TreeEntry) {
		files.selectSingle(row.path, row.is_dir);
		if (row.is_dir) {
			files.setSelectedFolder(row.path);
			void files.toggleFolder(row.path);
		} else {
			onfileselect(row.path);
		}
	}

	function handleTreeKeydown(e: KeyboardEvent) {
		if (e.target instanceof HTMLInputElement) return;
		keyboardFocus = true;
		const path = focusTargetPath;
		if (!path) return;
		const index = rows.findIndex((r) => r.path === path);
		if (index < 0) return;
		const row = rows[index];

		switch (e.key) {
			case 'ArrowDown':
				e.preventDefault();
				if (index + 1 < rows.length) void setFocus(rows[index + 1].path);
				break;
			case 'ArrowUp':
				e.preventDefault();
				if (index > 0) void setFocus(rows[index - 1].path);
				break;
			case 'ArrowRight':
				e.preventDefault();
				if (!row.is_dir) break;
				if (!files.expandedFolders.has(row.path)) void files.expandFolder(row.path);
				else if (rows[index + 1]?.depth > row.depth) void setFocus(rows[index + 1].path);
				break;
			case 'ArrowLeft': {
				e.preventDefault();
				if (row.is_dir && files.expandedFolders.has(row.path)) {
					void files.collapseFolder(row.path);
					break;
				}
				const parent = parentDir(row.path);
				if (rows.some((r) => r.path === parent)) void setFocus(parent);
				break;
			}
			case 'Enter':
				e.preventDefault();
				activateRow(row);
				break;
			case 'F2':
				e.preventDefault();
				files.startRename(row.path);
				break;
			case 'Delete':
			case 'Backspace':
				e.preventDefault();
				ondeleterow(row);
				break;
		}
	}

	// ── Inline editing ───────────────────────────────────────────────────────

	let renameSubmitting = false;

	/**
	 * Commit the rename input.
	 *
	 * Enter and blur both land here, so it has to be idempotent: the first commit
	 * clears `renamingPath`, which unmounts the input and fires a blur that would
	 * otherwise rename a second time — reporting the name the rename just created
	 * as "already exists".
	 */
	async function submitRename(value: string) {
		const path = files.renamingPath;
		if (!path || renameSubmitting) return;
		const entry = rows.find((r) => r.path === path);
		if (!entry || value === entry.name) {
			files.cancelRename();
			return;
		}
		renameSubmitting = true;
		try {
			// A refused rename keeps the input open so the typed name is not lost.
			if (await onrename(entry, value)) files.cancelRename();
		} finally {
			renameSubmitting = false;
		}
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
		try {
			// `create_dir_all` succeeds silently on a folder that already exists, so
			// the pre-flight check is the only thing that reports the collision.
			if (await fileExists(folderPath)) {
				toast.error(m.toast_path_exists({ name: trimmed }));
				return;
			}
			await createDirectory(folderPath);
		} catch (err) {
			toast.error(m.toast_create_folder_failed({ error: String(err) }));
			return;
		} finally {
			// Any outcome clears the pending row: a stuck inline input would swallow
			// every later attempt to create a folder in this parent.
			files.cancelNewFolder();
		}
		await files.expandFolder(folderPath);
		files.setSelectedFolder(folderPath);
		files.requestTreeReveal(folderPath);
		editor.markLocalChange();
	}

	function selectStem(input: HTMLInputElement) {
		const dot = input.value.lastIndexOf('.');
		input.setSelectionRange(0, dot > 0 ? dot : input.value.length);
	}

	const newFolderEdit = useInlineEdit({
		onSubmit: (value) => void confirmNewFolder(value),
		onCancel: () => files.cancelNewFolder()
	});
	const renameEdit = useInlineEdit({
		onSubmit: (value) => void submitRename(value),
		onCancel: () => files.cancelRename()
	});
</script>

<!-- The tree itself is not a tab stop: the roving tabindex on the rows is. -->
<div
	class="relative h-full overflow-x-hidden overflow-y-auto"
	role="tree"
	aria-label={m.sidebar_explorer()}
	tabindex={-1}
	data-drop-kind="tree-root"
	bind:this={viewport}
	bind:clientHeight={viewportHeight}
	onkeydown={handleTreeKeydown}
	onfocusout={(e) => {
		const next = e.relatedTarget as Node | null;
		if (!next || !viewport?.contains(next)) keyboardFocus = false;
	}}
	onmouseup={handleTreeDrop}
	onscroll={() => {
		if (viewport) scrollTop = viewport.scrollTop;
	}}
	onclick={(e) => {
		if (!(e.target as HTMLElement).closest('.tree-row') && vault.vaultPath) {
			files.clearSelection();
			files.setSelectedFolder(vault.vaultPath);
		}
	}}
>
	<!-- Full-height spacer keeps the scrollbar proportionate -->
	<div class="relative w-full" style="height: {totalHeight}px;">
		{#each renderedItems as item (item.kind === 'row' ? item.row.path : '__new_folder__')}
			<div class="absolute inset-x-0 h-8" style="top: {item.virtualIndex * ROW_HEIGHT}px;">
				{@render rowItem(item)}
			</div>
		{/each}
	</div>
</div>

{#snippet rowItem(item: VisibleItem)}
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
				onfocus={(e) => e.currentTarget.select()}
				onkeydown={newFolderEdit.handleKeydown}
				onblur={newFolderEdit.handleBlur}
			/>
		</div>
	{:else}
		{@const row = item.row}
		{@const indent = row.depth * 16}
		{@const isExpanded = files.expandedFolders.has(row.path)}
		{@const isRenaming = files.renamingPath === row.path}
		{@const isFocused = keyboardFocus && focusTargetPath === row.path}
		{#if row.is_dir}
			{#if isRenaming}
				<div class={ROW_BASE} style="padding-left: {indent + 8}px;">
					{#if isExpanded}<FolderOpen size={16} />{:else}<Folder size={16} />{/if}
					<!-- svelte-ignore a11y_autofocus -->
					<input
						class={INLINE_INPUT}
						autofocus
						value={row.name}
						onfocus={(e) => selectStem(e.currentTarget)}
						onkeydown={renameEdit.handleKeydown}
						onblur={renameEdit.handleBlur}
					/>
				</div>
			{:else}
				<button
					class={folderRowClass(files.isSelected(row.path), isDropTarget(row.path), isFocused)}
					style="padding-left: {indent + 4}px;"
					role="treeitem"
					aria-selected={files.isSelected(row.path)}
					aria-expanded={isExpanded}
					aria-level={row.depth + 1}
					tabindex={isFocused ? 0 : -1}
					data-path={row.path}
					data-drop-kind="folder"
					data-drop-path={row.path}
					onmousedown={(e) => startDrag(e, row)}
					onclick={(e) => {
						focusedPath = row.path;
						if (suppressNextClick) {
							suppressNextClick = false;
							return;
						}
						if (e.metaKey || e.ctrlKey) {
							files.selectToggle(row.path, true);
						} else if (e.shiftKey) {
							files.selectRange(row.path);
						} else if (e.detail === 3) {
							files.startRename(row.path);
						} else {
							files.selectSingle(row.path, true);
							files.setSelectedFolder(row.path);
							if (!isSecondClick(row.path)) void files.toggleFolder(row.path);
						}
					}}
					oncontextmenu={(e) => {
						e.preventDefault();
						e.stopPropagation();
						if (!files.isSelected(row.path)) {
							files.selectSingle(row.path, true);
						}
						files.setSelectedFolder(row.path);
						focusedPath = row.path;
						oncontextmenuentry(row, e);
					}}
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
					onfocus={(e) => selectStem(e.currentTarget)}
					onkeydown={renameEdit.handleKeydown}
					onblur={renameEdit.handleBlur}
				/>
			</div>
		{:else}
			<button
				class={fileRowClass(activeFile === row.path, files.isSelected(row.path), isFocused)}
				style="padding-left: {indent + 8}px;"
				role="treeitem"
				aria-selected={files.isSelected(row.path)}
				aria-level={row.depth + 1}
				tabindex={isFocused ? 0 : -1}
				data-path={row.path}
				data-drop-kind="file"
				data-drop-path={row.path}
				data-drop-parent={parentDir(row.path)}
				onmousedown={(e) => startDrag(e, row)}
				onclick={(e) => {
					focusedPath = row.path;
					if (suppressNextClick) {
						suppressNextClick = false;
						return;
					}
					if (e.metaKey || e.ctrlKey) {
						files.selectToggle(row.path, false);
					} else if (e.shiftKey) {
						files.selectRange(row.path);
					} else if (e.detail === 3) {
						files.startRename(row.path);
					} else {
						files.selectSingle(row.path, false);
						onfileselect(row.path);
					}
				}}
				oncontextmenu={(e) => {
					e.preventDefault();
					e.stopPropagation();
					if (!files.isSelected(row.path)) {
						files.selectSingle(row.path, false);
					}
					focusedPath = row.path;
					oncontextmenuentry(row, e);
				}}
			>
				<FileText size={16} />
				<span class="min-w-0 flex-1 truncate">{row.name.replace(/\.(md|canvas)$/, '')}</span>
			</button>
		{/if}
	{/if}
{/snippet}

<style>
	/* Roving tabindex moves a single tab stop through the tree; the ring has to be
	   visible on the row itself, not only under `:focus-visible`, so the keyboard
	   user can see where Enter/F2/Delete will act. */
	.tree-row.is-focused {
		outline: 2px solid var(--color-border-focus);
		outline-offset: -2px;
	}
</style>
