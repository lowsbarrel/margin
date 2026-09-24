<script lang="ts">
	import { onMount, onDestroy, untrack } from 'svelte';
	import { editor as editorStore } from '$lib/stores/editor.svelte';
	import { noteFocus } from '$lib/stores/note-focus';
	import { vault } from '$lib/stores/vault.svelte';
	import type { ViewMode } from '$lib/stores/panes.svelte';
	import { drag } from '$lib/stores/drag.svelte';
	import { resolveAttachmentFolder } from '$lib/editor/attachments';
	import { isModalOpen } from '$lib/utils/modal';
	import { findVaultFile, vaultFileExists, watchVaultIndex } from '$lib/editor/live/vault-index';
	import { ESCAPE_FIND, onEscape } from '$lib/editor/live/escape';
	import type { LiveEditorHandle } from '$lib/editor/live/editor-factory';
	import type { ContextMenuItem } from './ContextMenu.svelte';
	import type { LightboxImage } from './ImageLightbox.svelte';
	import * as m from '$lib/paraglide/messages.js';
	import ContextMenu from './ContextMenu.svelte';
	import FindReplace from './FindReplace.svelte';
	import ImageLightbox from './ImageLightbox.svelte';
	import { SaveController } from './editor/save-controller';
	import { TitleEditor } from './editor/title-editor.svelte';
	import { acceptPendingInsert, registerEditorDropTarget } from './editor/drop-target';
	import '$lib/editor/live/live-preview.css';
	import '$lib/editor/live/code-block.css';

	interface Props {
		filePath: string;
		initialContent: string;
		externalContentVersion?: number;
		title: string;
		active?: boolean;
		viewMode?: ViewMode;
		initialCursorPos?: number;
		onrename?: (oldPath: string, newPath: string) => void | Promise<void>;
		onwikilink?: (title: string) => void;
		onsave?: (content: string) => void;
		onsnapshotcursor?: (pos: number) => void;
		attachmentFolder?: string | null;
	}

	let {
		filePath,
		initialContent,
		externalContentVersion = 0,
		title: initialTitle,
		active = true,
		viewMode = 'rich',
		initialCursorPos,
		onrename,
		onwikilink,
		onsave,
		onsnapshotcursor,
		attachmentFolder = null
	}: Props = $props();

	let container: HTMLDivElement;
	let titleEl: HTMLDivElement;
	let live = $state<LiveEditorHandle | null>(null);
	let showFindReplace = $state(false);
	let findReplaceMode = $state(false);
	let lightbox = $state<{ images: LightboxImage[]; index: number } | null>(null);
	let contextMenu = $state<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);
	let currentPath = $state(untrack(() => filePath));
	let appliedMode = untrack(() => viewMode);
	let syncedFilePath = untrack(() => filePath);
	let lastSeenVersion = untrack(() => externalContentVersion);
	let wasActive = untrack(() => active);
	let alive = true;
	let stopIndex: (() => void) | null = null;
	let releaseEscape: (() => void) | null = null;

	const folder = $derived(resolveAttachmentFolder(attachmentFolder));

	const save = new SaveController(
		{
			path: () => currentPath,
			vaultPath: () => vault.vaultPath,
			isAlive: () => alive,
			onsave: () => onsave
		},
		untrack(() => initialContent ?? null)
	);

	const title = new TitleEditor(
		{
			path: () => currentPath,
			setPath: (path) => (currentPath = path),
			isAlive: () => alive,
			focusEditor: () => live?.focus(),
			openSlashMenu: () => live?.openSlashMenu(),
			element: () => titleEl,
			onrename: () => onrename
		},
		untrack(() => initialTitle)
	);

	const flushSave = save.flush.bind(save);

	function openLightbox(src: string, alt: string): void {
		const images = container
			? Array.from(container.querySelectorAll('img')).map((img) => ({
					src: img.src,
					alt: img.alt
				}))
			: [];
		const clicked = images.findIndex((image) => image.src === src);
		lightbox = clicked >= 0 ? { images, index: clicked } : { images: [{ src, alt }], index: 0 };
	}

	function openContextMenu(x: number, y: number, items: ContextMenuItem[]): void {
		contextMenu = { x, y, items };
	}

	function navigateLightbox(index: number): void {
		if (lightbox) lightbox = { ...lightbox, index };
	}

	function handleDocumentChange(text: string): void {
		if (!alive) return;
		editorStore.setDirty(true);
		save.schedule(text);
	}

	function closeFind(): void {
		showFindReplace = false;
		live?.focus();
	}

	function handleFindHotkey(event: KeyboardEvent): void {
		if (!active || isModalOpen()) return;
		if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) return;
		const key = event.key.toLowerCase();
		if (key !== 'f' && key !== 'h') return;
		event.preventDefault();
		findReplaceMode = key === 'h';
		showFindReplace = true;
	}

	function reportCursorSnapshot(): void {
		const offset = live?.selectionOffset();
		if (offset != null) onsnapshotcursor?.(offset);
	}

	$effect(() => {
		if (!active || !noteFocus.take(currentPath)) return;
		title.focusTitle();
	});

	$effect(() => {
		const view = live?.view ?? null;
		if (active && view) editorStore.setView(view);
		else editorStore.releaseView(view);
	});

	$effect(() => {
		if (!live || viewMode === appliedMode) return;
		appliedMode = viewMode;
		live.setMode(viewMode === 'source');
	});

	$effect(() => {
		const path = filePath;
		if (path === syncedFilePath) return;
		syncedFilePath = path;
		if (title.pending) return;
		currentPath = path;
		title.syncToPath(path);
		live?.refresh();
	});

	$effect(() => {
		const version = externalContentVersion;
		if (version === lastSeenVersion) return;
		lastSeenVersion = version;
		const editor = live;
		if (!editor || initialContent == null) return;
		save.snapshot(editor.text());
		save.cancel();
		editorStore.setDirty(false);
		save.lastSavedText = initialContent;
		editor.adopt(initialContent);
	});

	$effect(() => {
		if (!drag.active || drag.item?.kind !== 'file' || !container || !live) return;
		const handler = (event: MouseEvent) => live?.placeCursor(event.clientX, event.clientY);
		container.addEventListener('mousemove', handler);
		return () => container.removeEventListener('mousemove', handler);
	});

	$effect(() => {
		if (!drag.pendingInsert) return;
		acceptPendingInsert(container, live?.view ?? null);
	});

	$effect(() => {
		if (!active) return;
		return registerEditorDropTarget(() => live?.view ?? null);
	});

	$effect(() => {
		const isActive = active;
		if (wasActive && !isActive) untrack(() => reportCursorSnapshot());
		wasActive = isActive;
	});

	onMount(() => {
		void (async () => {
			const { createLiveEditor } = await import('$lib/editor/live/editor-factory');
			const editor = await createLiveEditor({
				parent: container,
				doc: initialContent,
				source: appliedMode === 'source',
				vaultPath: () => vault.vaultPath,
				notePath: () => currentPath,
				attachmentFolder: () => folder,
				exists: vaultFileExists,
				findByName: findVaultFile,
				openLightbox,
				openWikiLink: (linkTitle) => onwikilink?.(linkTitle),
				openContextMenu,
				onDocChange: handleDocumentChange,
				onCursor: (line, col) => editorStore.setCursor(line, col),
				onSelection: () => {}
			});
			if (!alive) {
				editor.destroy();
				return;
			}
			live = editor;
			editorStore.setView(editor.view);
			if (initialCursorPos != null) editor.setSelectionOffset(initialCursorPos);
			stopIndex = watchVaultIndex(vault.vaultPath, () => live?.refresh());
			releaseEscape = onEscape(editor.view, ESCAPE_FIND, () => {
				if (!showFindReplace) return false;
				closeFind();
				return true;
			});
		})();

		window.addEventListener('margin:flush', flushSave);
		window.addEventListener('margin:flush', reportCursorSnapshot);
		window.addEventListener('keydown', handleFindHotkey, true);
	});

	onDestroy(() => {
		save.flush();
		alive = false;
		title.dispose();
		stopIndex?.();
		releaseEscape?.();
		window.removeEventListener('margin:flush', flushSave);
		window.removeEventListener('margin:flush', reportCursorSnapshot);
		window.removeEventListener('keydown', handleFindHotkey, true);
		editorStore.releaseView(live?.view ?? null);
		live?.destroy();
		live = null;
	});
</script>

<div class="editor-container relative flex-1 overflow-y-auto bg-background" data-drop-kind="editor">
	{#if showFindReplace && live}
		<FindReplace
			view={live.view}
			showReplace={findReplaceMode}
			ontogglereplace={() => (findReplaceMode = !findReplaceMode)}
			onclose={closeFind}
		/>
	{/if}
	<div
		class="title-input mx-auto max-w-187.5 cursor-text px-10 pt-12 font-sans text-3xl leading-[1.2] font-bold tracking-tight wrap-break-word text-foreground outline-none empty:before:pointer-events-none empty:before:text-subtle-foreground empty:before:content-[attr(data-placeholder)]"
		contenteditable="true"
		bind:this={titleEl}
		bind:textContent={title.text}
		oninput={(e) => title.input(e.currentTarget.textContent?.trim() ?? '')}
		onkeydown={(e) => title.keydown(e)}
		onblur={() => title.blur()}
		data-placeholder={m.editor_untitled()}
		role="textbox"
		tabindex="0"
	></div>
	<div class="editor-wrap overflow-hidden bg-background" bind:this={container}></div>
</div>

{#if lightbox}
	<ImageLightbox
		images={lightbox.images}
		index={lightbox.index}
		onclose={() => (lightbox = null)}
		onnavigate={navigateLightbox}
	/>
{/if}

{#if contextMenu}
	<ContextMenu
		x={contextMenu.x}
		y={contextMenu.y}
		items={contextMenu.items}
		onclose={() => (contextMenu = null)}
	/>
{/if}
