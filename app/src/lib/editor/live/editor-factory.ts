import { Compartment, EditorState, Transaction } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import type { ContextMenuItem } from '$lib/components/ContextMenu.svelte';
import { liveContext, type LiveContext } from './context';
import { createHighlighter, type Highlighter } from './code-highlight';
import { baseExtensions, previewExtensions } from './extensions';
import { refreshDecorations } from './decorate';
import { sourceHighlighting } from './theme';

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
	openContextMenu(x: number, y: number, items: ContextMenuItem[]): void;
	onDocChange(text: string): void;
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
	destroy(): void;
}

export interface TextDifference {
	from: number;
	to: number;
	insert: string;
}

export function minimalDiff(oldText: string, newText: string): TextDifference {
	const shortest = Math.min(oldText.length, newText.length);
	let start = 0;
	while (start < shortest && oldText[start] === newText[start]) start++;
	let tail = 0;
	while (
		tail < shortest - start &&
		oldText[oldText.length - 1 - tail] === newText[newText.length - 1 - tail]
	) {
		tail++;
	}
	return {
		from: start,
		to: oldText.length - tail,
		insert: newText.slice(start, newText.length - tail)
	};
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
				...baseExtensions,
				EditorView.updateListener.of((update) => {
					if (
						update.docChanged &&
						!update.transactions.some((tr) => tr.annotation(Transaction.addToHistory) === false)
					) {
						host.onDocChange(update.state.doc.toString());
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
		focus: () => view.focus(),
		destroy: () => view.destroy()
	};
}
