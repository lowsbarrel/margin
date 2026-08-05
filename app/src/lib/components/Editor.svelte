<script lang="ts">
	import { onMount, onDestroy, untrack } from 'svelte';
	import { Editor } from '@tiptap/core';
	import { common, createLowlight } from 'lowlight';
	import { createEditorExtensions } from '$lib/editor/extensions';
	import { splitFrontmatter, joinFrontmatter } from '$lib/editor/frontmatter';
	import { unresolveImagePaths } from '$lib/editor/image-paths';
	import { transformImagePaths } from '$lib/editor/text-transform-bridge';
	import { editor as editorStore } from '$lib/stores/editor.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { writeFileBytes } from '$lib/fs/bridge';
	import { saveSnapshot } from '$lib/history/bridge';
	import { getCurrentWebview } from '@tauri-apps/api/webview';
	import { drag } from '$lib/stores/drag.svelte';
	import ContextMenu from './ContextMenu.svelte';
	import type { ContextMenuItem } from './ContextMenu.svelte';
	import BubbleToolbar from './BubbleToolbar.svelte';
	import FindReplace from './FindReplace.svelte';
	import ImageLightbox from './ImageLightbox.svelte';
	import { validateName } from '$lib/utils/filename';
	import * as m from '$lib/paraglide/messages.js';
	import '$lib/editor/editor-styles.css';
	import { handleEditorPaste } from '$lib/editor/handlers/paste';
	import { handleEditorClick, buildEditorContextMenu } from '$lib/editor/handlers/clicks';
	import {
		setCursorAtCoords,
		insertFileAtCursor,
		handleTauriFileDrop
	} from '$lib/editor/handlers/drag-drop';
	import { positionBubbleMenu } from '$lib/editor/handlers/bubble-menu';
	// SearchReplace augments TipTap's Storage interface with both `searchReplace`
	// and the tiptap-markdown `markdown` storage, so `editor.storage.markdown` is typed.
	import '$lib/editor/search-replace';

	/** Read serialized Markdown from the editor's tiptap-markdown storage. */
	function getEditorMarkdown(e: Editor): string {
		return e.storage.markdown?.getMarkdown?.() ?? '';
	}

	/**
	 * The note's YAML frontmatter, held verbatim while the body is edited —
	 * TipTap would otherwise rewrite the block into a setext heading. Not `$state`:
	 * nothing renders it, and it is only ever read inside editor callbacks.
	 */
	let frontmatter: string | null = null;

	/**
	 * The full document as it should hit disk: what the editor holds, with the
	 * frontmatter put back. Everything downstream — save, history, sync, the
	 * index — sees the complete note.
	 */
	function serializeDocument(e: Editor): string {
		return joinFrontmatter(frontmatter, unresolveImagePaths(getEditorMarkdown(e), vault.vaultPath));
	}

	/** Take the frontmatter aside and hand back only the body for the editor. */
	function takeFrontmatter(markdown: string): string {
		const split = splitFrontmatter(markdown);
		frontmatter = split.frontmatter;
		return split.body;
	}

	interface Props {
		filePath: string;
		initialContent: string;
		externalContentVersion?: number;
		title: string;
		active?: boolean;
		/** Caret position to restore once, on first mount (workspace restore). */
		initialCursorPos?: number;
		onrename?: (oldPath: string, newPath: string) => void;
		onwikilink?: (title: string) => void;
		onsave?: (content: string) => void;
		/** Report the current caret position so it can be persisted per tab. */
		onsnapshotcursor?: (pos: number) => void;
		attachmentFolder?: string | null;
	}

	let {
		filePath,
		initialContent,
		externalContentVersion = 0,
		title: initialTitle,
		active = true,
		initialCursorPos,
		onrename,
		onwikilink,
		onsave,
		onsnapshotcursor,
		attachmentFolder = null
	}: Props = $props();

	let container: HTMLDivElement;
	let bubbleMenuEl: HTMLDivElement;
	/**
	 * The note title lives in a `contenteditable` div, so its text is state the
	 * component owns rather than something to poke into the DOM by hand:
	 * `bind:textContent` seeds it on mount and writes the reverted name back when
	 * an edit fails validation. Seeded through `untrack` because the `title` prop
	 * follows the file path — a rename originates *here*, and letting the prop
	 * feed back in would fight the caret while typing.
	 */
	let titleText = $state(untrack(() => initialTitle));
	let tiptap = $state<Editor | null>(null);

	$effect(() => {
		if (active && tiptap) {
			editorStore.setTiptap(tiptap);
		}
	});
	let bubbleVisible = $state(false);
	let bubblePositionToken = 0;
	let bubbleUpdateRaf = 0;

	let renameTimer: ReturnType<typeof setTimeout> | null = null;
	let blurTimer: ReturnType<typeof setTimeout> | null = null;
	let saveTimer: ReturnType<typeof setTimeout> | null = null;
	let pendingSaveText: string | null = null;
	let alive = true;
	let currentPath = $state(untrack(() => filePath));
	let unlistenDragDrop: (() => void) | null = null;
	let handleFindHotkeyRef: EventListener | null = null;
	let lightboxSrc = $state<string | null>(null);
	let lightboxAlt = $state('');
	let ctxMenu = $state<{
		x: number;
		y: number;
		items: ContextMenuItem[];
	} | null>(null);
	let showFindReplace = $state(false);
	let findReplaceMode = $state(false);
	const lowlight = createLowlight(common);

	const RENAME_DELAY = 150;
	const SAVE_DEBOUNCE_MS = 400;
	const SNAPSHOT_INTERVAL_MS = 5 * 60 * 1000; // 5 minutes between snapshots
	let lastSnapshotTime = 0;
	let lastSnapshotMd: string | null = null;

	// Reload content on external update
	let lastSeenVersion = untrack(() => externalContentVersion);
	let externalUpdateToken = 0;
	$effect(() => {
		const v = externalContentVersion;
		if (v !== lastSeenVersion) {
			lastSeenVersion = v;
			if (tiptap && initialContent != null) {
				// Flush any in-flight local edit before swapping content so it isn't lost.
				flushPendingSave();
				// Guard against out-of-order async resolution (mirror bubblePositionToken).
				const token = ++externalUpdateToken;
				transformImagePaths(
					takeFrontmatter(initialContent),
					vault.vaultPath,
					attachmentFolder,
					'resolve'
				).then((resolved) => {
					if (token !== externalUpdateToken || !tiptap) return;
					// Preserve selection across the full-document replacement.
					const prev = tiptap.state.selection;
					const prevFrom = prev.from;
					const prevTo = prev.to;
					tiptap.commands.setContent(resolved, { emitUpdate: false });
					const size = tiptap.state.doc.content.size;
					const from = Math.min(prevFrom, size);
					const to = Math.min(prevTo, size);
					tiptap.commands.setTextSelection({ from, to });
				});
			}
		}
	});

	function saveNow(text: string) {
		if (!alive) return;
		onsave?.(text);
		const encoder = new TextEncoder();
		const encoded = encoder.encode(text);
		writeFileBytes(currentPath, encoded)
			.then(() => {
				editorStore.setDirty(false);
				const now = Date.now();
				if (
					vault.vaultPath &&
					now - lastSnapshotTime >= SNAPSHOT_INTERVAL_MS &&
					text !== lastSnapshotMd
				) {
					lastSnapshotTime = now;
					lastSnapshotMd = text;
					saveSnapshot(vault.vaultPath, currentPath, encoded).catch((err) => {
						console.warn('Snapshot save failed:', err);
						toast.error(m.toast_save_snapshot_failed());
					});
				}
			})
			.catch((err) => {
				console.error('Save failed:', err);
				toast.error(m.toast_save_file_failed());
			});
	}

	/** Schedule a debounced save; coalesces bursts of typing into one write. */
	function scheduleSave(text: string) {
		pendingSaveText = text;
		if (saveTimer) clearTimeout(saveTimer);
		saveTimer = setTimeout(() => {
			saveTimer = null;
			const text = pendingSaveText;
			pendingSaveText = null;
			if (text != null) saveNow(text);
		}, SAVE_DEBOUNCE_MS);
	}

	/** Flush any pending debounced save immediately (enqueues to the write queue). */
	function flushPendingSave() {
		if (saveTimer) {
			clearTimeout(saveTimer);
			saveTimer = null;
		}
		const text = pendingSaveText;
		pendingSaveText = null;
		if (text != null) saveNow(text);
	}

	/** Report the caret position to the parent so it can be persisted per tab. */
	function snapshotCursor() {
		if (!tiptap) return;
		onsnapshotcursor?.(tiptap.state.selection.from);
	}

	/**
	 * Restore a persisted caret position exactly once, on first mount. Clamped to
	 * the document so a stale position (file shrank on disk) can't throw.
	 */
	function restoreCursorOnce() {
		if (initialCursorPos == null || !tiptap) return;
		try {
			const size = tiptap.state.doc.content.size;
			const pos = Math.min(Math.max(initialCursorPos, 0), Math.max(size - 1, 0));
			tiptap.commands.setTextSelection(pos);
			tiptap.commands.scrollIntoView();
		} catch {
			/* stale position — ignore */
		}
	}

	// Capture the caret when this editor goes active → inactive (tab switch) so
	// the per-tab snapshot stays fresh. Within a session the cached instance keeps
	// its own caret; this only matters for persistence across a restart.
	let wasActive = untrack(() => active);
	$effect(() => {
		const isActive = active;
		if (wasActive && !isActive) untrack(() => snapshotCursor());
		wasActive = isActive;
	});

	function handleTitleInput(raw: string) {
		if (!alive) return;
		if (!raw) return;

		const currentName = currentPath.split('/').pop() ?? '';
		const currentTitle = currentName.endsWith('.md') ? currentName.slice(0, -3) : currentName;
		if (raw === currentTitle) return;

		if (renameTimer) clearTimeout(renameTimer);
		renameTimer = setTimeout(() => {
			if (!alive) return;
			const error = validateName(raw);
			if (error) {
				toast.error(error);
				return;
			}
			const dir = currentPath.substring(0, currentPath.lastIndexOf('/'));
			const newPath = `${dir}/${raw}.md`;
			if (newPath !== currentPath) {
				const oldPath = currentPath;
				currentPath = newPath;
				onrename?.(oldPath, newPath);
			}
		}, RENAME_DELAY);
	}

	function handleTitleBlur() {
		const error = validateName(titleText.trim());
		if (error) {
			const name = currentPath.split('/').pop() ?? '';
			titleText = name.endsWith('.md') ? name.slice(0, -3) : name;
		}
	}

	function handleTitleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			tiptap?.commands.focus('start');
		}
	}

	function hideBubbleMenu() {
		bubbleVisible = false;
		if (!bubbleMenuEl) return;
		bubbleMenuEl.style.left = '-9999px';
		bubbleMenuEl.style.top = '-9999px';
	}

	/**
	 * Compute the status-bar Ln/Col cheaply from the resolved cursor position,
	 * reading only the current top-level block instead of slicing the whole doc.
	 */
	function updateCursorStatus() {
		if (!tiptap) return;
		const { doc, selection } = tiptap.state;
		const from = selection.from;
		const resolvedPos = doc.resolve(from);
		// Line = top-level block index (+1); for top-level node selections (depth 0)
		// there is no inner block to inspect.
		let line = resolvedPos.index(0) + 1;
		let col = 1;
		if (resolvedPos.depth >= 1) {
			const blockStart = resolvedPos.start(1);
			col = from - blockStart + 1;
			// Account for hard breaks / newlines within the current textblock.
			if (from > blockStart) {
				const blockText = doc.textBetween(blockStart, from, '\n');
				const nl = blockText.lastIndexOf('\n');
				if (nl !== -1) {
					line += blockText.split('\n').length - 1;
					col = blockText.length - nl;
				}
			}
		}
		editorStore.setCursor(line, col);
	}

	function updateBubbleMenu() {
		if (!tiptap || !bubbleMenuEl) return;
		const token = ++bubblePositionToken;
		positionBubbleMenu(tiptap, bubbleMenuEl).then((result) => {
			if (token !== bubblePositionToken) return;
			if (result) {
				bubbleMenuEl.style.left = `${result.x}px`;
				bubbleMenuEl.style.top = `${result.y}px`;
				bubbleVisible = true;
			} else {
				hideBubbleMenu();
			}
		});
	}

	function handleInternalDragMouseMove(e: MouseEvent) {
		if (tiptap) setCursorAtCoords(tiptap, e.clientX, e.clientY);
	}

	$effect(() => {
		if (drag.active && drag.item?.kind === 'file' && container) {
			container.addEventListener('mousemove', handleInternalDragMouseMove);
			return () => container.removeEventListener('mousemove', handleInternalDragMouseMove);
		}
	});

	// Handle pending insert from drop handler in +page.svelte
	$effect(() => {
		const pending = drag.pendingInsert;
		if (!pending || !container || !tiptap) return;
		const rect = container.getBoundingClientRect();
		if (
			pending.x >= rect.left &&
			pending.x <= rect.right &&
			pending.y >= rect.top &&
			pending.y <= rect.bottom
		) {
			drag.clearPendingInsert();
			setCursorAtCoords(tiptap, pending.x, pending.y);
			insertFileAtCursor(pending.path, tiptap, vault.vaultPath!, attachmentFolder);
		}
	});

	function handleLinkClick(event: MouseEvent) {
		if (!container) return;
		handleEditorClick(event, container, {
			vaultPath: vault.vaultPath,
			onLightbox: (src, alt) => {
				lightboxSrc = src;
				lightboxAlt = alt;
			},
			onWikiLink: onwikilink
		});
	}

	function handleTauriDragOver(pos: { x: number; y: number }) {
		if (tiptap) setCursorAtCoords(tiptap, pos.x, pos.y);
	}

	async function handleTauriDrop(paths: string[], position?: { x: number; y: number }) {
		// If this drop originated from our own native drag, flag it and skip
		if (drag.nativeDragActive) {
			drag.markDroppedBackInApp();
			return;
		}
		if (!vault.vaultPath || !tiptap) return;
		await handleTauriFileDrop(
			paths,
			position,
			tiptap,
			container,
			vault.vaultPath,
			attachmentFolder
		);
	}

	function handlePaste(event: ClipboardEvent) {
		if (!attachmentFolder || !vault.vaultPath || !tiptap) return;
		handleEditorPaste(event, tiptap, vault.vaultPath, attachmentFolder);
	}

	function handleEditorContextMenu(event: MouseEvent) {
		if (!container) return;
		const result = buildEditorContextMenu(event, container, tiptap, {
			vaultPath: vault.vaultPath,
			onLightbox: (src, alt) => {
				lightboxSrc = src;
				lightboxAlt = alt;
			}
		});
		if (result) ctxMenu = result;
	}

	function createEditor(content: string) {
		if (tiptap) {
			tiptap.destroy();
			tiptap = null;
		}

		let lastSavedMd: string | null = null;

		const inst = new Editor({
			element: container,
			extensions: createEditorExtensions({ lowlight, attachmentFolder }),
			content: content,
			editorProps: {
				attributes: {
					class: 'md-editor',
					spellcheck: 'false'
				}
			},
			onCreate: ({ editor: e }) => {
				lastSavedMd = serializeDocument(e);
			},
			onUpdate: ({ editor: e }) => {
				const text = serializeDocument(e);
				if (text === lastSavedMd) return;
				lastSavedMd = text;
				// Immediate UI feedback; debounce the actual write to coalesce typing.
				editorStore.setDirty(true);
				scheduleSave(text);
			},
			onSelectionUpdate: () => {
				// Throttle the (potentially O(n)) cursor read + bubble update to one
				// per frame to avoid work on every keystroke.
				if (!bubbleUpdateRaf) {
					bubbleUpdateRaf = requestAnimationFrame(() => {
						bubbleUpdateRaf = 0;
						updateCursorStatus();
						updateBubbleMenu();
					});
				}
			},
			onBlur: () => {
				// Flush the pending debounced save so leaving the editor never loses an edit.
				flushPendingSave();
				blurTimer = setTimeout(() => {
					blurTimer = null;
					if (!bubbleMenuEl?.contains(document.activeElement)) {
						hideBubbleMenu();
					}
				}, 100);
			},
			onFocus: () => {
				updateBubbleMenu();
				editorStore.setTiptap(inst);
			}
		});

		tiptap = inst;
		editorStore.setTiptap(inst);
	}

	onMount(() => {
		transformImagePaths(
			takeFrontmatter(initialContent),
			vault.vaultPath,
			attachmentFolder,
			'resolve'
		).then((resolved) => {
			createEditor(resolved);
			restoreCursorOnce();
		});
		container.addEventListener('paste', handlePaste as EventListener, true);
		container.addEventListener('contextmenu', handleEditorContextMenu as EventListener, true);
		// Find & Replace keyboard shortcuts
		function handleFindHotkey(e: KeyboardEvent) {
			if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
				e.preventDefault();
				findReplaceMode = false;
				showFindReplace = true;
			} else if ((e.metaKey || e.ctrlKey) && e.key === 'h') {
				e.preventDefault();
				findReplaceMode = true;
				showFindReplace = true;
			}
		}
		handleFindHotkeyRef = handleFindHotkey as EventListener;
		container.addEventListener('keydown', handleFindHotkeyRef);

		// Flush the pending save when the app requests it (e.g. before window close).
		window.addEventListener('margin:flush', flushPendingSave);
		// Also capture the caret on flush so the active tab's position survives quit.
		window.addEventListener('margin:flush', snapshotCursor);
	});

	// Global document click + webview drag-drop are gated on `active` so hidden
	// (cached) editors don't register O(tabs) redundant global handlers.
	$effect(() => {
		if (!active) return;

		document.addEventListener('click', handleLinkClick as EventListener, true);

		let unlisten: (() => void) | null = null;
		let disposed = false;
		getCurrentWebview()
			.onDragDropEvent((event) => {
				if (event.payload.type === 'drop') {
					handleTauriDrop(event.payload.paths, event.payload.position);
				} else if (event.payload.type === 'over') {
					handleTauriDragOver(event.payload.position);
				}
			})
			.then((fn) => {
				if (disposed) {
					fn();
				} else {
					unlisten = fn;
					unlistenDragDrop = fn;
				}
			});

		return () => {
			disposed = true;
			document.removeEventListener('click', handleLinkClick as EventListener, true);
			unlisten?.();
			if (unlistenDragDrop === unlisten) unlistenDragDrop = null;
		};
	});

	onDestroy(() => {
		// Final safety net: persist any pending edit before tearing down.
		flushPendingSave();
		alive = false;
		if (renameTimer) clearTimeout(renameTimer);
		if (blurTimer) clearTimeout(blurTimer);
		if (saveTimer) clearTimeout(saveTimer);
		if (bubbleUpdateRaf) cancelAnimationFrame(bubbleUpdateRaf);
		window.removeEventListener('margin:flush', flushPendingSave);
		window.removeEventListener('margin:flush', snapshotCursor);
		container?.removeEventListener('paste', handlePaste as EventListener, true);
		container?.removeEventListener('contextmenu', handleEditorContextMenu as EventListener, true);
		if (handleFindHotkeyRef) {
			container?.removeEventListener('keydown', handleFindHotkeyRef);
		}
		unlistenDragDrop?.();
		if (showFindReplace && tiptap) {
			tiptap.commands.clearSearch();
		}
		if (editorStore.tiptap === tiptap) {
			editorStore.setTiptap(null);
		}
		tiptap?.destroy();
		tiptap = null;
	});
</script>

<div class="bubble-wrapper" class:visible={bubbleVisible} bind:this={bubbleMenuEl}>
	<BubbleToolbar editor={tiptap} />
</div>

<!-- `editor-container` carries no styling any more, but the name must stay: it is
     the scroll-parent hook that content-drag, search-replace, pdf-export and the
     page-level "scroll to match" helper all find via `closest()`. -->
<div class="editor-container relative flex-1 overflow-y-auto bg-background">
	{#if showFindReplace}
		<FindReplace
			editor={tiptap}
			showReplace={findReplaceMode}
			onclose={() => {
				showFindReplace = false;
			}}
		/>
	{/if}
	<!-- `title-input` is kept as a selector hook for pdf-export, which reads the
	     note title out of the DOM. The placeholder is the `:empty::before` pair. -->
	<div
		class="title-input mx-auto max-w-[750px] cursor-text px-10 pt-12 font-sans text-3xl leading-[1.2] font-bold tracking-tight wrap-break-word text-foreground outline-none empty:before:pointer-events-none empty:before:text-subtle-foreground empty:before:content-[attr(data-placeholder)]"
		contenteditable="true"
		bind:textContent={titleText}
		oninput={(e) => handleTitleInput(e.currentTarget.textContent?.trim() ?? '')}
		onkeydown={handleTitleKeydown}
		onblur={handleTitleBlur}
		data-placeholder={m.editor_untitled()}
		role="textbox"
		tabindex={0}
	></div>
	<!-- `editor-wrap` is the root of the ProseMirror-generated document; the whole
	     of `editor-styles.css` (plus the list-marker rules in app.css) hangs off
	     this class name, so it stays. -->
	<div class="editor-wrap overflow-hidden bg-background" bind:this={container}></div>
</div>

{#if lightboxSrc}
	<ImageLightbox src={lightboxSrc} alt={lightboxAlt} onclose={() => (lightboxSrc = null)} />
{/if}

{#if ctxMenu}
	<ContextMenu x={ctxMenu.x} y={ctxMenu.y} items={ctxMenu.items} onclose={() => (ctxMenu = null)} />
{/if}
