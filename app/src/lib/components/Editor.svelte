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
	import { writeFileBytes, fileExists } from '$lib/fs/bridge';
	import { fileTitle } from '$lib/stores/panes.svelte';
	import type { ViewMode } from '$lib/stores/panes.svelte';
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
	import type { SourceEditor } from '$lib/editor/source/codemirror';
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
		/** Which surface this tab shows: the rich editor or the raw Markdown. */
		viewMode?: ViewMode;
		/** Caret position to restore once, on first mount (workspace restore). */
		initialCursorPos?: number;
		onrename?: (oldPath: string, newPath: string) => void | Promise<void>;
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
		viewMode = 'rich',
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
	 * an edit fails validation. Seeded through `untrack` because a rename
	 * originates *here*: only a path change from outside (the sync effect below)
	 * may overwrite it, never the prop feeding back mid-edit.
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
	/** Last `filePath` prop value seen, to tell a real move from a re-render. */
	let syncedFilePath = untrack(() => filePath);
	/** True while a title rename started here is awaiting its callback. */
	let titleRenamePending = false;
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
	/** Mount point the CodeMirror source surface is attached to. */
	let sourceEl: HTMLDivElement | undefined = $state();
	/** The source surface, created on first use and kept for the tab's life. */
	let source: SourceEditor | null = null;
	/** Invalidates an in-flight source-surface setup after a fast mode switch. */
	let sourceToken = 0;
	/**
	 * The exact text the rich document's current content came from, when that is
	 * known (the file as loaded, an external reload, or a source-mode edit). While
	 * it stands, switching to source shows that text verbatim instead of a
	 * serializer round-trip; any edit in the rich editor clears it.
	 */
	let sourceSeed: string | null = untrack(() => initialContent);
	/** Workspace-restored caret offset, held until the source surface exists. */
	let pendingSourceCursor: number | null = null;
	const lowlight = createLowlight(common);

	const RENAME_DELAY = 150;
	const SAVE_DEBOUNCE_MS = 400;
	const SNAPSHOT_INTERVAL_MS = 5 * 60 * 1000; // 5 minutes between snapshots
	let lastSnapshotTime = 0;
	let lastSnapshotMd: string | null = null;

	// The tab's path changes underneath us when the sidebar renames or moves the
	// note; without this, saves keep writing to the path the file no longer has.
	$effect(() => {
		const path = filePath;
		if (path === syncedFilePath) return;
		syncedFilePath = path;
		// A rename started here owns both fields until its callback settles.
		if (titleRenamePending) return;
		currentPath = path;
		titleText = fileTitle(path);
	});

	// Reload content on external update
	let lastSeenVersion = untrack(() => externalContentVersion);
	// Guards every full-document replacement of the rich editor against an
	// out-of-order async resolution, whichever path started it.
	let richReplaceToken = 0;
	$effect(() => {
		const v = externalContentVersion;
		if (v !== lastSeenVersion) {
			lastSeenVersion = v;
			if (tiptap && initialContent != null) {
				const mode = viewMode;
				// Whatever local text is still queued for the debounced save is
				// superseded by the incoming content. Snapshot the document as it
				// stands so the edit survives in history, then drop the stale write
				// — flushing it would put the old text back on disk over the very
				// change being adopted.
				snapshot(mode === 'source' && source ? source.getText() : serializeDocument(tiptap));
				cancelPendingSave();
				editorStore.setDirty(false);
				sourceSeed = initialContent;

				// The source surface holds the file's bytes as they are; only the
				// rich editor needs the image paths resolved before parsing.
				if (mode === 'source' && source) {
					source.setText(initialContent);
					return;
				}

				// Guard against out-of-order async resolution (mirror bubblePositionToken).
				const token = ++richReplaceToken;
				transformImagePaths(
					takeFrontmatter(initialContent),
					vault.vaultPath,
					attachmentFolder,
					'resolve'
				).then((resolved) => {
					if (token !== richReplaceToken || !tiptap) return;
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
				if (Date.now() - lastSnapshotTime >= SNAPSHOT_INTERVAL_MS) snapshot(text);
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

	/**
	 * Keep a copy of `text` in the note's history. Skipped when the last snapshot
	 * already holds this content, so the periodic interval and the pre-overwrite
	 * snapshot don't duplicate each other.
	 */
	function snapshot(text: string) {
		if (!vault.vaultPath || text === lastSnapshotMd) return;
		lastSnapshotTime = Date.now();
		lastSnapshotMd = text;
		saveSnapshot(vault.vaultPath, currentPath, new TextEncoder().encode(text)).catch((err) => {
			console.warn('Snapshot save failed:', err);
			toast.error(m.toast_save_snapshot_failed());
		});
	}

	/** Drop the pending debounced save — its text has been superseded. */
	function cancelPendingSave() {
		if (saveTimer) {
			clearTimeout(saveTimer);
			saveTimer = null;
		}
		pendingSaveText = null;
	}

	/** Report the caret position to the parent so it can be persisted per tab. */
	function snapshotCursor() {
		if (viewMode === 'source' && source) {
			onsnapshotcursor?.(source.getCursorOffset());
			return;
		}
		if (!tiptap) return;
		onsnapshotcursor?.(tiptap.state.selection.from);
	}

	/**
	 * Restore a persisted caret position exactly once, on first mount. Clamped to
	 * the document so a stale position (file shrank on disk) can't throw.
	 */
	function restoreCursorOnce() {
		if (initialCursorPos == null) return;
		// The persisted position was recorded against whichever surface was up at
		// the time, so a source-mode tab hands it to CodeMirror instead.
		if (viewMode === 'source') {
			pendingSourceCursor = initialCursorPos;
			return;
		}
		if (!tiptap) return;
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

		if (raw === fileTitle(currentPath)) return;

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
			if (newPath !== currentPath) void renameTo(newPath);
		}, RENAME_DELAY);
	}

	/** Put the title input back to the name of the file this editor still holds. */
	function revertTitle() {
		titleText = fileTitle(currentPath);
	}

	/**
	 * Rename the file behind this editor, leaving `currentPath` untouched until
	 * the rename has gone through: while it is in flight a flush must still write
	 * to the old path, and the backend refuses to overwrite an existing file.
	 */
	async function renameTo(newPath: string) {
		// A second attempt while one is in flight would still be targeting the path
		// this editor has not moved to yet; the first one settles the title.
		if (titleRenamePending) return;
		titleRenamePending = true;
		try {
			// A case-only rename targets the same directory entry, so `fileExists()`
			// reports it as present — only a destination that differs beyond case can
			// be a collision.
			const differsBeyondCase = newPath.toLowerCase() !== currentPath.toLowerCase();
			if (differsBeyondCase && (await fileExists(newPath))) {
				toast.error(m.toast_path_exists({ name: newPath.split('/').pop() ?? '' }));
				revertTitle();
				return;
			}

			await onrename?.(currentPath, newPath);
			currentPath = newPath;
			titleText = fileTitle(newPath);
		} catch (err) {
			console.error('Rename failed:', err);
			revertTitle();
			toast.error(m.toast_rename_failed({ error: String(err) }));
		} finally {
			titleRenamePending = false;
		}
	}

	function handleTitleBlur() {
		if (validateName(titleText.trim())) revertTitle();
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
		// If this drop originated from our own native drag, skip it
		if (drag.nativeDragActive) return;
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
				// The rich document no longer matches any text the source surface
				// could hold verbatim, so a later switch has to serialize it.
				sourceSeed = null;
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

	/**
	 * The caret's plain-text offset in the rich document, mapped onto the source
	 * text. Block text is the anchor and document-proportional scroll the floor:
	 * markdown markers (`**`, `> `) drop out of the rich text, so an anchor that
	 * cannot be found is expected rather than exceptional.
	 */
	function richCursorToSourceOffset(e: Editor, sourceText: string): number {
		const { doc, selection } = e.state;
		const from = selection.from;
		const proportional = Math.round((from / Math.max(doc.content.size, 1)) * sourceText.length);
		const resolved = doc.resolve(from);
		if (resolved.depth < 1) return proportional;
		const anchor = doc.textBetween(resolved.start(1), from, ' ').slice(-24).trim();
		if (!anchor) return proportional;
		const at = sourceText.indexOf(anchor);
		return at === -1 ? proportional : at + anchor.length;
	}

	/** The reverse of {@link richCursorToSourceOffset}. */
	function sourceOffsetToRichPos(e: Editor, sourceText: string, offset: number): number {
		const doc = e.state.doc;
		const size = doc.content.size;
		const clamp = (pos: number) => Math.min(Math.max(pos, 0), Math.max(size - 1, 0));
		const proportional = Math.round((offset / Math.max(sourceText.length, 1)) * size);
		const anchor = sourceText.slice(Math.max(0, offset - 24), offset).trim();
		if (!anchor) return clamp(proportional);
		const plain = doc.textBetween(0, size, '\n');
		const at = plain.indexOf(anchor);
		if (at === -1) return clamp(proportional);
		return clamp(Math.round(((at + anchor.length) / Math.max(plain.length, 1)) * size));
	}

	/** Status-bar line/column for a plain-text offset. */
	function reportSourceCursor(text: string, offset: number) {
		const lineStart = text.lastIndexOf('\n', Math.max(offset - 1, 0));
		const line = text.slice(0, offset).split('\n').length;
		editorStore.setCursor(line, offset - lineStart);
	}

	/** A source-surface edit: the raw text *is* the file, so it saves as-is. */
	function handleSourceChange(text: string) {
		if (!alive) return;
		editorStore.setDirty(true);
		scheduleSave(text);
	}

	/** Replace the rich document with `text`, re-parsing it through the load path. */
	function applyTextToRich(e: Editor, text: string, caretPos: number) {
		const token = ++richReplaceToken;
		return transformImagePaths(
			takeFrontmatter(text),
			vault.vaultPath,
			attachmentFolder,
			'resolve'
		).then((resolved) => {
			if (token !== richReplaceToken || !alive) return;
			e.commands.setContent(resolved, { emitUpdate: false });
			const size = e.state.doc.content.size;
			e.commands.setTextSelection(Math.min(Math.max(caretPos, 0), Math.max(size - 1, 0)));
			e.commands.scrollIntoView();
		});
	}

	async function enterSourceMode(e: Editor) {
		const token = ++sourceToken;
		// The debounce window would otherwise swallow the last rich keystroke.
		flushPendingSave();
		showFindReplace = false;

		const text = sourceSeed ?? serializeDocument(e);
		let offset = richCursorToSourceOffset(e, text);
		if (pendingSourceCursor != null) {
			offset = pendingSourceCursor;
			pendingSourceCursor = null;
		}

		if (!source) {
			if (!sourceEl) return;
			const { createSourceEditor } = await import('$lib/editor/source/codemirror');
			if (token !== sourceToken || !alive || !sourceEl) return;
			source = await createSourceEditor({
				parent: sourceEl,
				doc: text,
				onChange: handleSourceChange,
				onCursor: (line, col) => editorStore.setCursor(line, col)
			});
			source.setCursorOffset(offset);
		} else if (source.getText() !== text) {
			source.setText(text);
			source.setCursorOffset(offset);
		} else {
			// Nothing was reseeded, so the surface's own caret is still the better one.
			offset = source.getCursorOffset();
		}

		// The mode flipped back while the surface was being built.
		if (token !== sourceToken || !alive) return;

		// A programmatic cursor move emits no selection event when the offset is
		// unchanged, so the readout is refreshed from the text either way.
		reportSourceCursor(text, offset);
		// A background tab (cached editor) must not pull focus off the active one.
		if (active) source.focus();
	}

	function exitSourceMode(e: Editor) {
		sourceToken++;
		// Same reason as entering: the source save must land before the rich
		// editor is rebuilt from it.
		flushPendingSave();
		if (!source) return;
		const text = source.getText();
		const offset = source.getCursorOffset();
		sourceSeed = text;
		void applyTextToRich(e, text, sourceOffsetToRichPos(e, text, offset));
	}

	// The surface follows the tab's mode: the rich editor stays alive (hidden)
	// while the source view is up, so neither switch pays a re-parse it can avoid.
	$effect(() => {
		const mode = viewMode;
		const e = tiptap;
		if (!e) return;
		untrack(() => {
			if (mode === 'source') void enterSourceMode(e);
			else exitSourceMode(e);
		});
	});

	// A hidden tab measures as empty; remeasure when it comes back to the front.
	$effect(() => {
		if (active && viewMode === 'source' && source) source.requestMeasure();
	});

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
				// CodeMirror's own search keymap already opened its panel on this
				// event; the TipTap panel would land on the hidden rich editor.
				if (viewMode === 'source') return;
				e.preventDefault();
				findReplaceMode = false;
				showFindReplace = true;
			} else if ((e.metaKey || e.ctrlKey) && e.key === 'h') {
				if (viewMode === 'source') return;
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
		sourceToken++;
		source?.destroy();
		source = null;
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
<div
	class="editor-container relative flex-1 bg-background"
	class:flex={viewMode === 'source'}
	class:flex-col={viewMode === 'source'}
	class:overflow-y-auto={viewMode === 'rich'}
	class:overflow-hidden={viewMode === 'source'}
>
	{#if showFindReplace && viewMode === 'rich'}
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
		class="title-input mx-auto max-w-187.5 cursor-text px-10 pt-12 font-sans text-3xl leading-[1.2] font-bold tracking-tight wrap-break-word text-foreground outline-none empty:before:pointer-events-none empty:before:text-subtle-foreground empty:before:content-[attr(data-placeholder)]"
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
	     this class name, so it stays. Hidden, not torn down, in source mode. -->
	<div
		class="editor-wrap overflow-hidden bg-background"
		class:hidden={viewMode === 'source'}
		bind:this={container}
	></div>
	<!-- Source mode. The outer box mirrors the rich surface's `max-width: 750px`
	     and `2.5rem` side padding so toggling does not shift the text column. -->
	<div class="min-h-0 flex-1" class:hidden={viewMode !== 'source'}>
		<div class="mx-auto h-full w-full max-w-187.5 px-10 pt-4" bind:this={sourceEl}></div>
	</div>
</div>

{#if lightboxSrc}
	<ImageLightbox src={lightboxSrc} alt={lightboxAlt} onclose={() => (lightboxSrc = null)} />
{/if}

{#if ctxMenu}
	<ContextMenu x={ctxMenu.x} y={ctxMenu.y} items={ctxMenu.items} onclose={() => (ctxMenu = null)} />
{/if}
