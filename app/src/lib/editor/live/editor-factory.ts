import { indentUnit } from '@codemirror/language';
import { Compartment, EditorState, Transaction } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import type { ContextMenuItem } from '$lib/components/ContextMenu.svelte';
import { liveContext, type LiveContext } from './context';
import { createHighlighter, type Highlighter } from './code-highlight';
import { openSlashMenu } from './complete';
import { baseExtensions, previewExtensions } from './extensions';
import { refreshDecorations } from './decorate';
import { detectIndentUnit } from './list-tree';
import { focusEditor } from './table-cell';
import { sourceHighlighting } from './theme';
import { minimalDiff } from '$lib/utils/text-diff';

export interface LiveEditorHost {
	parent: HTMLElement;
	doc: string;
	source: boolean;
	vaultPath(): string | null;
	notePath(): string;
	attachmentFolder(): string;
	exists(relPath: string): boolean;
	findByName(name: string): string | null;
	openLightbox(src: string, alt: string): void;
	openWikiLink(title: string): void;
	openTag(tag: string): void;
	focusTitle(): void;
	openContextMenu(x: number, y: number, items: ContextMenuItem[]): void;
	onDocChange(text: () => string): void;
	onCursor(line: number, col: number): void;
	onSelection(): void;
}

export interface LiveEditorHandle {
	view: EditorView;
	code: Highlighter | null;
	setMode(source: boolean): void;
	text(): string;
	selectionOffset(): number;
	setSelectionOffset(offset: number): void;
	placeCursor(x: number, y: number): void;
	adopt(text: string): void;
	refresh(): void;
	focus(): void;
	openSlashMenu(): void;
	destroy(): void;
}

export async function createLiveEditor(host: LiveEditorHost): Promise<LiveEditorHandle> {
	const code = await createHighlighter();
	const context: LiveContext = {
		vaultPath: () => host.vaultPath(),
		notePath: () => host.notePath(),
		attachmentFolder: () => host.attachmentFolder(),
		exists: (relPath) => host.exists(relPath),
		findByName: (name) => host.findByName(name),
		openLightbox: (src, alt) => host.openLightbox(src, alt),
		openWikiLink: (title) => host.openWikiLink(title),
		openTag: (tag) => host.openTag(tag),
		focusTitle: () => host.focusTitle(),
		openContextMenu: (x, y, items) => host.openContextMenu(x, y, items),
		code
	};

	const preview = new Compartment();
	const highlighting = new Compartment();
	const view = new EditorView({
		parent: host.parent,
		state: EditorState.create({
			doc: host.doc,
			extensions: [
				liveContext.of(context),
				preview.of(host.source ? [] : previewExtensions),
				highlighting.of(host.source ? [sourceHighlighting] : []),
				indentUnit.of(detectIndentUnit(host.doc)),
				...baseExtensions,
				EditorView.updateListener.of((update) => {
					if (
						update.docChanged &&
						!update.transactions.some((tr) => tr.annotation(Transaction.addToHistory) === false)
					) {
						const state = update.state;
						host.onDocChange(() => state.doc.toString());
					}
					if (update.docChanged || update.selectionSet) {
						const head = update.state.selection.main.head;
						const line = update.state.doc.lineAt(head);
						host.onCursor(line.number, head - line.from + 1);
					}
					if (update.selectionSet) host.onSelection();
				})
			]
		})
	});

	const initial = view.state.selection.main.head;
	const initialLine = view.state.doc.lineAt(initial);
	host.onCursor(initialLine.number, initial - initialLine.from + 1);

	return {
		view,
		code,
		setMode(source: boolean) {
			view.dispatch({
				effects: [
					preview.reconfigure(source ? [] : previewExtensions),
					highlighting.reconfigure(source ? [sourceHighlighting] : []),
					refreshDecorations.of(null)
				]
			});
		},
		text: () => view.state.doc.toString(),
		selectionOffset: () => view.state.selection.main.head,
		setSelectionOffset(offset: number) {
			const target = Math.max(0, Math.min(offset, view.state.doc.length));
			if (target === view.state.selection.main.anchor) return;
			view.dispatch({ selection: { anchor: target }, scrollIntoView: true });
		},
		placeCursor(x: number, y: number) {
			const pos = view.posAtCoords({ x, y });
			if (pos == null) return;
			view.dispatch({ selection: { anchor: pos } });
		},
		adopt(text: string) {
			const current = view.state.doc.toString();
			if (current === text) return;
			view.dispatch({
				changes: minimalDiff(current, text),
				annotations: Transaction.addToHistory.of(false)
			});
		},
		refresh() {
			view.dispatch({ effects: refreshDecorations.of(null) });
		},
		focus: () => focusEditor(view),
		openSlashMenu: () => openSlashMenu(view),
		destroy: () => view.destroy()
	};
}
