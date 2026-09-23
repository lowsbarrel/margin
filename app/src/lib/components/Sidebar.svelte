<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import ContextMenu, { type ContextMenuItem } from './ContextMenu.svelte';
	import FileTree from './FileTree.svelte';
	import {
		fileExists,
		revealInFileManager,
		copyFile,
		copyDirectory,
		type FsEntry,
		type TreeEntry
	} from '$lib/fs/bridge';
	import { files } from '$lib/stores/files.svelte';
	import { editor } from '$lib/stores/editor.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { clipboard } from '$lib/stores/clipboard.svelte';
	import { installExternalDropRouter } from '$lib/utils/external-drop';
	import { IconButton } from '$lib/ui';
	import { handleNewFolder, handleNewNote } from '$lib/utils/page-actions';
	// Static / non-interactive glyphs stay on lucide; the registry has no
	// animated equivalent for PanelLeftClose.
	import { PanelLeftClose, ArrowDownAZ, ArrowDownWideNarrow, ChevronsDownUp } from '@lucide/svelte';
	// Hover-animated counterparts for the controls the user actually points at.
	import { FilePlus, FolderPlus, PenLine } from '$lib/components/movingicons';
	import * as m from '$lib/paraglide/messages.js';
	import { normalizeFileName, normalizeDirName, createUniquePath } from '$lib/utils/sidebar-ops';
	import { buildMenuItems, type MenuTarget } from '$lib/utils/sidebar-menu';

	interface Props {
		onfileselect: (path: string, searchText?: string) => void;
		onrenameentry: (from: string, to: string, isDir: boolean) => Promise<void>;
		ondeleteentry: (path: string, isDir: boolean) => Promise<void>;
		panelOpen: boolean;
		ontoggle: () => void;
		panelWidth?: number;
	}

	let {
		onfileselect,
		onrenameentry,
		ondeleteentry,
		panelOpen,
		ontoggle,
		panelWidth = $bindable(280)
	}: Props = $props();
	let menuTarget = $state<MenuTarget | null>(null);
	let menuX = $state(0);
	let menuY = $state(0);
	let unlistenExternalDrop: (() => void) | null = null;
	let dragDropDisposed = false;

	const PANEL_MIN = 180;
	const PANEL_MAX = 480;
	let resizing = $state(false);

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

	// Context menu
	function closeContextMenu() {
		menuTarget = null;
	}

	function openRootContextMenu(event: MouseEvent) {
		if (!vault.vaultPath) return;
		if ((event.target as HTMLElement).closest('.tree-row')) return;
		event.preventDefault();
		// Stop it here: the freshly mounted menu listens on `document` for
		// contextmenu, and an event still travelling up the tree would close the
		// menu the same tick it opened. The row handler does the same.
		event.stopPropagation();
		// The target travels in the menu item's own closure, so opening the menu
		// must not touch the selection: an accidental right-click dismissed with
		// Escape used to destroy a deliberate multi-selection and retarget the
		// toolbar at the vault root.
		menuX = event.clientX;
		menuY = event.clientY;
		menuTarget = { kind: 'root', path: vault.vaultPath };
	}

	function openEntryContextMenu(entry: TreeEntry, event: MouseEvent) {
		event.preventDefault();
		menuX = event.clientX;
		menuY = event.clientY;
		menuTarget = { kind: 'entry', entry };
	}

	// Rename/delete/duplicate
	/**
	 * Rename a tree entry. Returns whether the row may leave edit mode: a refused
	 * rename keeps the inline input open so the name the user typed is not thrown
	 * away by the toast reporting the collision.
	 *
	 * A case-only rename (`note.md` → `Note.md`) targets the same directory entry,
	 * so `fileExists` reports it as present; only a destination that differs
	 * beyond case can be a collision, and Rust resolves the case-folded decision
	 * inside the command.
	 */
	async function handleInlineRename(entry: TreeEntry, newName: string): Promise<boolean> {
		const sanitized = entry.is_dir ? normalizeDirName(newName) : normalizeFileName(newName);
		if (sanitized === null) return false;
		if (sanitized === entry.name) return true;

		const parent = entry.path.slice(0, entry.path.lastIndexOf('/'));
		const newPath = `${parent}/${sanitized}`;
		const differsBeyondCase = newPath.toLowerCase() !== entry.path.toLowerCase();
		if (differsBeyondCase && (await fileExists(newPath))) {
			toast.error(m.toast_path_exists({ name: sanitized }));
			return false;
		}

		try {
			await onrenameentry(entry.path, newPath, entry.is_dir);
			return true;
		} catch (err) {
			toast.error(m.toast_rename_failed({ error: String(err) }));
			return false;
		}
	}

	/**
	 * Delete the whole selection, not just the row that was right-clicked.
	 *
	 * Copy and cut already treat a multi-selection as the unit of work; delete
	 * used to act on the single clicked entry, so highlighting three notes and
	 * choosing Delete removed one and left the other two behind.
	 *
	 * Deletes run sequentially: each one refreshes the tree when it finishes, and
	 * overlapping refreshes can resolve out of order and restore a snapshot that
	 * still lists an already-deleted note.
	 */
	async function handleDeleteRequest(entry: FsEntry) {
		const selection = files.getSelectedAsList();
		const targets =
			selection.length > 1 && files.isSelected(entry.path)
				? selection
				: [{ path: entry.path, isDir: entry.is_dir }];

		const confirmed = window.confirm(
			targets.length > 1
				? m.sidebar_delete_many({ count: String(targets.length) })
				: `${m.sidebar_delete()} "${entry.name}"?`
		);
		if (!confirmed) return;

		for (const target of targets) {
			try {
				await ondeleteentry(target.path, target.isDir);
			} catch (err) {
				toast.error(m.toast_delete_failed({ error: String(err) }));
			}
		}
	}

	async function handleOpenInFinder(path: string) {
		try {
			await revealInFileManager(path);
		} catch (err) {
			toast.error(m.toast_open_finder_failed({ error: String(err) }));
		}
	}

	async function handleDuplicate(entry: FsEntry) {
		if (!vault.vaultPath) return;
		const parent = entry.path.slice(0, entry.path.lastIndexOf('/'));
		const candidate = await createUniquePath(parent, entry.name, 'copy');

		try {
			if (entry.is_dir) await copyDirectory(entry.path, candidate);
			else await copyFile(entry.path, candidate);
			await ensureFolderExpanded(parent);
			await files.refresh(vault.vaultPath);
			// Without this the copy is created somewhere the user may not be looking
			// and nothing tells them which of two identical rows is the new one.
			files.setSelectedEntry(candidate, entry.is_dir);
			files.requestTreeReveal(candidate);
		} catch (err) {
			toast.error(m.toast_duplicate_failed({ error: String(err) }));
		}
	}

	function collapseAll() {
		files.collapseAll();
	}

	// Move entry (sidebar drag-drop)
	async function handleMoveEntry(fromPath: string, toDir: string, isDir: boolean) {
		if (!vault.vaultPath) return;
		const name = fromPath.split('/').pop() ?? '';
		let dest = `${toDir}/${name}`;
		if (await fileExists(dest)) {
			toast.error(m.toast_exists_in_folder({ name }));
			return;
		}
		try {
			await onrenameentry(fromPath, dest, isDir);
		} catch (err) {
			toast.error(m.toast_move_failed({ error: String(err) }));
		}
	}

	// Clipboard
	function handleCopy(entry?: { path: string; is_dir: boolean }) {
		if (entry && files.selectedEntries.size <= 1) {
			clipboard.copy([entry.path], [entry.is_dir]);
		} else {
			const sel = files.getSelectedAsList();
			if (sel.length === 0) return;
			clipboard.copy(
				sel.map((s) => s.path),
				sel.map((s) => s.isDir)
			);
		}
		toast.success(m.toast_copied());
	}

	function handleCut(entry?: { path: string; is_dir: boolean }) {
		if (entry && files.selectedEntries.size <= 1) {
			clipboard.cut([entry.path], [entry.is_dir]);
		} else {
			const sel = files.getSelectedAsList();
			if (sel.length === 0) return;
			clipboard.cut(
				sel.map((s) => s.path),
				sel.map((s) => s.isDir)
			);
		}
		toast.success(m.toast_cut_clipboard());
	}

	async function handlePaste(targetDir: string) {
		if (!vault.vaultPath || !clipboard.hasItems) return;
		const data = clipboard.consume();
		if (!data) return;

		for (let i = 0; i < data.paths.length; i++) {
			const srcPath = data.paths[i];
			const isDir = data.isDirs[i];
			const name = srcPath.split('/').pop() ?? '';
			const dest = await createUniquePath(targetDir, name);

			try {
				if (data.operation === 'copy') {
					if (isDir) await copyDirectory(srcPath, dest);
					else await copyFile(srcPath, dest);
				} else {
					await onrenameentry(srcPath, dest, isDir);
				}
			} catch (err) {
				toast.error(m.toast_paste_failed({ error: String(err) }));
			}
		}

		if (targetDir !== vault.vaultPath) await files.expandFolder(targetDir);
		await files.refresh(vault.vaultPath);
		editor.markLocalChange();
	}

	function getPasteTarget(): string {
		const sel = files.selectedEntry;
		if (sel?.isDir) return sel.path;
		if (files.selectedFolder) return files.selectedFolder;
		return vault.vaultPath ?? '';
	}

	function handleSidebarKeydown(e: KeyboardEvent) {
		if (!panelOpen) return;
		const el = document.activeElement;
		if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) return;
		if (el instanceof HTMLElement && el.isContentEditable) return;

		const mod = e.metaKey || e.ctrlKey;
		if (!mod) return;
		const sel = files.selectedEntry;

		if (e.key === 'c' && sel) {
			e.preventDefault();
			e.stopPropagation();
			handleCopy();
		} else if (e.key === 'x' && sel) {
			e.preventDefault();
			e.stopPropagation();
			handleCut();
		} else if (e.key === 'a') {
			e.preventDefault();
			e.stopPropagation();
			files.selectAll();
		} else if (e.key === 'v' && clipboard.hasItems) {
			e.preventDefault();
			e.stopPropagation();
			handlePaste(getPasteTarget());
		}
	}

	onMount(() => {
		window.addEventListener('keydown', handleSidebarKeydown);
		// One router owns the webview's drag-drop events; it hit-tests the pointer
		// and dispatches to the tree, the editor, or nothing.
		installExternalDropRouter().then((unlisten) => {
			// The component may have been destroyed before this resolved; if so,
			// unlisten immediately to avoid leaking the listener.
			if (dragDropDisposed) unlisten();
			else unlistenExternalDrop = unlisten;
		});
	});

	onDestroy(() => {
		window.removeEventListener('keydown', handleSidebarKeydown);
		dragDropDisposed = true;
		unlistenExternalDrop?.();
	});

	let menuItems = $derived.by((): ContextMenuItem[] => {
		if (!menuTarget) return [];
		return buildMenuItems(menuTarget, {
			onNewFile: (base) => handleNewNote(base),
			onNewCanvas: (base) => handleNewNote(base, 'Untitled.canvas'),
			onNewFolder: (base) => handleNewFolder(base),
			onPaste: (dir) => handlePaste(dir),
			onOpenInFinder: (path) => handleOpenInFinder(path),
			onCopy: (entry) => handleCopy(entry),
			onCut: (entry) => handleCut(entry),
			onDuplicate: (entry) => handleDuplicate(entry),
			onDelete: (entry) => handleDeleteRequest(entry)
		});
	});
</script>

{#if panelOpen}
	<aside
		class="relative flex flex-col overflow-hidden border-r border-border bg-background {resizing
			? 'select-none'
			: ''}"
		style="width:{panelWidth}px;min-width:{panelWidth}px"
	>
		<div class="panel-header">
			<span class="panel-title">{m.sidebar_explorer()}</span>
			<div class="panel-actions">
				<IconButton
					icon={files.sortOrder === 'name' ? ArrowDownAZ : ArrowDownWideNarrow}
					size="sm"
					onclick={() => files.toggleSortOrder()}
					title={files.sortOrder === 'name' ? m.sidebar_sort_by_date() : m.sidebar_sort_by_name()}
				/>
				<IconButton
					icon={ChevronsDownUp}
					size="sm"
					onclick={collapseAll}
					title={m.sidebar_collapse_all()}
				/>
				<IconButton
					icon={FilePlus}
					size="sm"
					onclick={() => handleNewNote()}
					title={m.sidebar_new_file()}
				/>
				<IconButton
					icon={PenLine}
					size="sm"
					onclick={() => handleNewNote(undefined, 'Untitled.canvas')}
					title={m.sidebar_new_canvas()}
				/>
				<IconButton
					icon={FolderPlus}
					size="sm"
					onclick={() => handleNewFolder()}
					title={m.sidebar_new_folder()}
				/>
				<IconButton
					icon={PanelLeftClose}
					size="sm"
					onclick={ontoggle}
					title={m.sidebar_close_panel()}
				/>
			</div>
		</div>

		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="panel-content" data-drop-kind="tree-root" oncontextmenu={openRootContextMenu}>
			<FileTree
				activeFile={files.activeFile}
				{onfileselect}
				oncontextmenuentry={openEntryContextMenu}
				onrename={handleInlineRename}
				onmoveentry={handleMoveEntry}
				ondeleterow={handleDeleteRequest}
			/>
		</div>

		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="absolute top-0 right-0 z-10 h-full w-1 cursor-col-resize transition-colors {resizing
				? 'bg-brand'
				: 'hover:bg-brand'}"
			onmousedown={onResizeStart}
		></div>
	</aside>
{/if}

{#if menuTarget && menuItems.length > 0}
	<ContextMenu x={menuX} y={menuY} items={menuItems} onclose={closeContextMenu} />
{/if}

<!-- No scoped CSS left in this component — the panel and resize handle are all
     utilities; `.panel-header` / `.panel-title` / `.panel-actions` /
     `.panel-content` are the global classes from `$lib/styles/components.css`. -->
