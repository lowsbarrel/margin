<script lang="ts">
	import { onMount, onDestroy, untrack } from 'svelte';
	import type { Editor } from '@tiptap/core';
	import { common, createLowlight } from 'lowlight';
	import { resolveAttachmentFolder, type AttachmentTarget } from '$lib/editor/attachments';
	import { editor as editorStore } from '$lib/stores/editor.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import type { ViewMode } from '$lib/stores/panes.svelte';
	import { drag } from '$lib/stores/drag.svelte';
	import { handleEditorPaste } from '$lib/editor/handlers/paste';
	import { setCursorAtCoords } from '$lib/editor/handlers/drag-drop';
	import * as m from '$lib/paraglide/messages.js';
	import ContextMenu from './ContextMenu.svelte';
	import BubbleToolbar from './BubbleToolbar.svelte';
	import FindReplace from './FindReplace.svelte';
	import ImageLightbox from './ImageLightbox.svelte';
	import { NoteDocument } from './editor/note-document';
	import { createEditorInstance } from './editor/editor-instance';
	import { SaveController } from './editor/save-controller';
	import { SourceSurface } from './editor/source-surface';
	import { BubbleMenu } from './editor/bubble-menu.svelte';
	import { EditorInteractions } from './editor/editor-interactions.svelte';
	import { TitleEditor } from './editor/title-editor.svelte';
	import { adoptExternalContent } from './editor/adopt-external-content';
	import { acceptPendingInsert, registerEditorDropTarget } from './editor/drop-target';
	import { reportRichCursor, restoreRichCursor } from './editor/cursor-position';
	import '$lib/editor/editor-styles.css';
	import '$lib/editor/search-replace';

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
	let sourceEl: HTMLDivElement | undefined = $state();
	let tiptap = $state<Editor | null>(null);
	let showFindReplace = $state(false);
	let findReplaceMode = $state(false);
	let currentPath = $state(untrack(() => filePath));
	let syncedFilePath = untrack(() => filePath);
	let lastSeenVersion = untrack(() => externalContentVersion);
	let wasActive = untrack(() => active);
	let alive = true;
	let blurTimer: ReturnType<typeof setTimeout> | undefined;
	let bubbleRaf = 0;
	let findHotkeyListener: EventListener | null = null;

	const folder = $derived(resolveAttachmentFolder(attachmentFolder));
	const lowlight = createLowlight(common);

	const note = new NoteDocument({ folder: () => folder, vaultPath: () => vault.vaultPath });

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
			focusEditor: () => void tiptap?.commands.focus('start'),
			onrename: () => onrename
		},
		untrack(() => initialTitle)
	);

	const source = new SourceSurface(
		{
			serializeRich: (editor) => note.serialize(editor),
			toEditorContent: (markdown) => note.toEditor(markdown),
			flushSave: () => save.flush(),
			scheduleSave: handleSourceChange,
			isAlive: () => alive,
			isActiveTab: () => active,
			onEnter: () => (showFindReplace = false)
		},
		untrack(() => initialContent)
	);

	const bubble = new BubbleMenu();
	const flushSave = save.flush.bind(save);

	const interactions = new EditorInteractions({
		container: () => container,
		rich: () => tiptap,
		vaultPath: () => vault.vaultPath,
		onWikiLink: () => onwikilink
	});
	const contextMenuListener = interactions.editorContextMenu as EventListener;

	function attachmentTarget(): AttachmentTarget | null {
		if (!tiptap || !vault.vaultPath) return null;
		return { editor: tiptap, vaultPath: vault.vaultPath, attachmentFolder: folder };
	}

	function handleSourceChange(text: string) {
		if (!alive) return;
		editorStore.setDirty(true);
		save.schedule(text);
	}

	function handleDocumentChange(markdown: string) {
		source.seed = null;
		editorStore.setDirty(true);
		save.schedule(markdown);
	}

	function reportCursorSnapshot() {
		if (viewMode === 'source' && source.editor) {
			onsnapshotcursor?.(source.editor.getCursorOffset());
			return;
		}
		if (tiptap) onsnapshotcursor?.(tiptap.state.selection.from);
	}

	function restoreCursorOnce() {
		if (initialCursorPos == null) return;
		if (viewMode === 'source') {
			source.pendingCursor = initialCursorPos;
			return;
		}
		if (tiptap) restoreRichCursor(tiptap, initialCursorPos);
	}

	function handleSelectionUpdate() {
		if (bubbleRaf) return;
		bubbleRaf = requestAnimationFrame(() => {
			bubbleRaf = 0;
			if (tiptap) reportRichCursor(tiptap);
			void bubble.update(tiptap);
		});
	}

	function handleEditorBlur() {
		save.flush();
		blurTimer = setTimeout(() => {
			blurTimer = undefined;
			if (!bubble.element?.contains(document.activeElement)) bubble.hide();
		}, 100);
	}

	function handleEditorFocus(editor: Editor) {
		void bubble.update(editor);
		if (active) editorStore.setTiptap(editor);
	}

	function handleInternalDragMouseMove(event: MouseEvent) {
		if (tiptap) setCursorAtCoords(tiptap, event.clientX, event.clientY);
	}

	function handlePaste(event: ClipboardEvent) {
		const target = attachmentTarget();
		if (target) handleEditorPaste(event, target);
	}

	function handleFindHotkey(event: KeyboardEvent) {
		if (viewMode === 'source') return;
		if (!event.metaKey && !event.ctrlKey) return;
		if (event.key === 'f') {
			event.preventDefault();
			findReplaceMode = false;
			showFindReplace = true;
		} else if (event.key === 'h') {
			event.preventDefault();
			findReplaceMode = true;
			showFindReplace = true;
		}
	}

	$effect(() => {
		if (active && tiptap) editorStore.setTiptap(tiptap);
		else editorStore.releaseTiptap(tiptap);
	});

	$effect(() => {
		const path = filePath;
		if (path === syncedFilePath) return;
		syncedFilePath = path;
		if (title.pending) return;
		currentPath = path;
		title.syncToPath(path);
	});

	$effect(() => {
		const version = externalContentVersion;
		if (version === lastSeenVersion) return;
		lastSeenVersion = version;
		if (!tiptap || initialContent == null) return;
		adoptExternalContent({
			content: initialContent,
			mode: viewMode,
			rich: tiptap,
			note,
			save,
			source
		});
	});

	$effect(() => {
		if (drag.active && drag.item?.kind === 'file' && container) {
			container.addEventListener('mousemove', handleInternalDragMouseMove);
			return () => container.removeEventListener('mousemove', handleInternalDragMouseMove);
		}
	});

	$effect(() => {
		if (!drag.pendingInsert) return;
		acceptPendingInsert(container, attachmentTarget());
	});

	$effect(() => {
		if (!active) return;
		return registerEditorDropTarget(
			() => tiptap,
			() => attachmentTarget()
		);
	});

	$effect(() => {
		const mode = viewMode;
		const editor = tiptap;
		if (!editor) return;
		untrack(() => {
			if (mode === 'source') void source.enter(editor, sourceEl);
			else source.exit(editor);
		});
	});

	$effect(() => {
		if (active && viewMode === 'source' && source.editor) source.editor.requestMeasure();
	});

	$effect(() => {
		if (!active) return;
		const handler = (event: MouseEvent) => interactions.linkClick(event);
		document.addEventListener('click', handler as EventListener, true);
		return () => document.removeEventListener('click', handler as EventListener, true);
	});

	$effect(() => {
		const isActive = active;
		if (wasActive && !isActive) untrack(() => reportCursorSnapshot());
		wasActive = isActive;
	});

	onMount(() => {
		tiptap = createEditorInstance({
			element: container,
			content: note.toEditor(initialContent),
			lowlight,
			attachmentFolder: folder,
			serialize: (editor) => note.serialize(editor),
			onDocumentChange: handleDocumentChange,
			onSelectionChange: handleSelectionUpdate,
			onBlur: handleEditorBlur,
			onFocus: handleEditorFocus
		});
		restoreCursorOnce();
		container.addEventListener('paste', handlePaste as EventListener, true);
		container.addEventListener('contextmenu', contextMenuListener, true);
		findHotkeyListener = handleFindHotkey as EventListener;
		container.addEventListener('keydown', findHotkeyListener);
		window.addEventListener('margin:flush', flushSave);
		window.addEventListener('margin:flush', reportCursorSnapshot);
	});

	onDestroy(() => {
		save.flush();
		alive = false;
		title.dispose();
		clearTimeout(blurTimer);
		if (bubbleRaf) cancelAnimationFrame(bubbleRaf);
		window.removeEventListener('margin:flush', flushSave);
		window.removeEventListener('margin:flush', reportCursorSnapshot);
		container?.removeEventListener('paste', handlePaste as EventListener, true);
		container?.removeEventListener('contextmenu', contextMenuListener, true);
		if (findHotkeyListener) container?.removeEventListener('keydown', findHotkeyListener);
		if (showFindReplace && tiptap) tiptap.commands.clearSearch();
		source.destroy();
		editorStore.releaseTiptap(tiptap);
		tiptap?.destroy();
		tiptap = null;
	});
</script>

<div class="bubble-wrapper" class:visible={bubble.visible} bind:this={bubble.element}>
	<BubbleToolbar editor={tiptap} />
</div>

<div
	class="editor-container relative flex-1 bg-background"
	class:flex={viewMode === 'source'}
	class:flex-col={viewMode === 'source'}
	class:overflow-y-auto={viewMode === 'rich'}
	class:overflow-hidden={viewMode === 'source'}
	data-drop-kind="editor"
>
	{#if showFindReplace && viewMode === 'rich'}
		<FindReplace
			editor={tiptap}
			showReplace={findReplaceMode}
			onclose={() => (showFindReplace = false)}
		/>
	{/if}
	<div
		class="title-input mx-auto max-w-187.5 cursor-text px-10 pt-12 font-sans text-3xl leading-[1.2] font-bold tracking-tight wrap-break-word text-foreground outline-none empty:before:pointer-events-none empty:before:text-subtle-foreground empty:before:content-[attr(data-placeholder)]"
		contenteditable="true"
		bind:textContent={title.text}
		oninput={(e) => title.input(e.currentTarget.textContent?.trim() ?? '')}
		onkeydown={(e) => title.keydown(e)}
		onblur={() => title.blur()}
		data-placeholder={m.editor_untitled()}
		role="textbox"
		tabindex={0}
	></div>
	<div
		class="editor-wrap overflow-hidden bg-background"
		class:hidden={viewMode === 'source'}
		bind:this={container}
	></div>
	<div class="min-h-0 flex-1" class:hidden={viewMode !== 'source'}>
		<div class="mx-auto h-full w-full max-w-187.5 px-10 pt-4" bind:this={sourceEl}></div>
	</div>
</div>

{#if interactions.lightbox}
	<ImageLightbox
		images={interactions.lightbox.images}
		index={interactions.lightbox.index}
		onclose={() => (interactions.lightbox = null)}
		onnavigate={(index) => interactions.navigateLightbox(index)}
	/>
{/if}

{#if interactions.contextMenu}
	<ContextMenu
		x={interactions.contextMenu.x}
		y={interactions.contextMenu.y}
		items={interactions.contextMenu.items}
		onclose={() => (interactions.contextMenu = null)}
	/>
{/if}
