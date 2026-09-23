/**
 * The raw-Markdown surface. The CodeMirror packages are pulled in here on first
 * use so their chunk stays out of the boot bundle; only types are imported
 * statically.
 *
 * Every colour goes through a `--color-*` token, so the surface follows
 * `data-theme` with no JS listening for a theme change.
 */

import type { HighlightStyle } from '@codemirror/language';
import type { EditorView } from '@codemirror/view';
import type { tags } from '@lezer/highlight';

export interface SourceEditor {
	getText(): string;
	setText(text: string): void;
	getCursorOffset(): number;
	setCursorOffset(offset: number): void;
	focus(): void;
	requestMeasure(): void;
	openSearch(): void;
	destroy(): void;
}

export interface SourceEditorOptions {
	parent: HTMLElement;
	doc: string;
	onChange: (text: string) => void;
	/** 1-based line and column of the caret, for the status bar. */
	onCursor: (line: number, col: number) => void;
}

function buildTheme(view: typeof EditorView) {
	return view.theme({
		'&': {
			height: '100%',
			backgroundColor: 'var(--color-bg-primary)',
			color: 'var(--color-text-primary)',
			fontSize: '15px'
		},
		'&.cm-focused': { outline: 'none' },
		'.cm-scroller': {
			fontFamily: 'var(--font-mono)',
			lineHeight: '1.7',
			overflow: 'auto'
		},
		// Bottom padding keeps the last line clear of the viewport edge when the
		// caret is at the end of the document.
		'.cm-content': { padding: '4px 0 40vh', caretColor: 'var(--color-text-primary)' },
		'.cm-activeLine': { backgroundColor: 'var(--color-brand-8)' },
		'.cm-cursor, .cm-dropCursor': {
			borderLeft: '2px solid var(--color-text-primary)'
		},
		'.cm-selectionBackground, &.cm-focused .cm-selectionBackground': {
			backgroundColor: 'var(--color-brand-24)'
		},
		'.cm-searchMatch': {
			backgroundColor: 'var(--color-brand-24)',
			outline: '1px solid var(--color-border-strong)'
		},
		'.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: 'var(--color-brand-32)' },
		'.cm-panels': {
			backgroundColor: 'var(--color-bg-secondary)',
			color: 'var(--color-text-primary)',
			fontFamily: 'var(--font-sans)',
			fontSize: '12px'
		},
		'.cm-panels.cm-panels-top': { borderBottom: '1px solid var(--color-border-secondary)' },
		'.cm-panel.cm-search': { padding: '6px 8px' },
		'.cm-panel.cm-search input, .cm-panel.cm-search button, .cm-panel.cm-search label': {
			marginRight: '6px',
			fontFamily: 'inherit',
			fontSize: 'inherit'
		},
		'.cm-textfield': {
			padding: '2px 6px',
			border: '1px solid var(--color-border-secondary)',
			borderRadius: 'var(--radius-xs)',
			backgroundColor: 'var(--color-bg-primary)',
			color: 'var(--color-text-primary)'
		},
		'.cm-button': {
			padding: '2px 8px',
			border: '1px solid var(--color-border-secondary)',
			borderRadius: 'var(--radius-xs)',
			backgroundImage: 'none',
			backgroundColor: 'var(--color-bg-tertiary)',
			color: 'var(--color-text-primary)'
		},
		'&.cm-focused .cm-matchingBracket': { backgroundColor: 'var(--color-brand-24)' }
	});
}

function buildHighlightStyle(hl: typeof HighlightStyle, hlTags: typeof tags) {
	return hl.define([
		{ tag: hlTags.heading, fontWeight: '600', color: 'var(--color-text-primary)' },
		{ tag: hlTags.strong, fontWeight: '600' },
		{ tag: hlTags.emphasis, fontStyle: 'italic' },
		{ tag: hlTags.strikethrough, textDecoration: 'line-through' },
		{ tag: hlTags.link, color: 'var(--color-text-brand)' },
		{ tag: hlTags.url, color: 'var(--color-syntax-function)' },
		{ tag: hlTags.monospace, color: 'var(--color-text-code)' },
		{ tag: hlTags.quote, color: 'var(--color-syntax-comment)' },
		{ tag: hlTags.list, color: 'var(--color-syntax-keyword)' },
		// Markdown's structural punctuation — `#`, `**`, `>`, fence ticks. Kept
		// quiet so the prose, not the syntax, carries the page.
		{ tag: hlTags.meta, color: 'var(--color-text-tertiary)' },
		{ tag: hlTags.processingInstruction, color: 'var(--color-text-tertiary)' },
		{ tag: hlTags.comment, color: 'var(--color-syntax-comment)', fontStyle: 'italic' },
		{ tag: hlTags.string, color: 'var(--color-syntax-string)' },
		{ tag: hlTags.number, color: 'var(--color-syntax-number)' },
		{ tag: hlTags.keyword, color: 'var(--color-syntax-keyword)' },
		{ tag: hlTags.typeName, color: 'var(--color-syntax-attr)' }
	]);
}

export async function createSourceEditor({
	parent,
	doc,
	onChange,
	onCursor
}: SourceEditorOptions): Promise<SourceEditor> {
	const [
		{ EditorState },
		viewModule,
		{ defaultKeymap, history, historyKeymap },
		{ HighlightStyle, syntaxHighlighting },
		{ markdown },
		{ searchKeymap, openSearchPanel },
		{ tags }
	] = await Promise.all([
		import('@codemirror/state'),
		import('@codemirror/view'),
		import('@codemirror/commands'),
		import('@codemirror/language'),
		import('@codemirror/lang-markdown'),
		import('@codemirror/search'),
		import('@lezer/highlight')
	]);
	const {
		EditorView,
		keymap,
		drawSelection,
		dropCursor,
		highlightActiveLine,
		highlightSpecialChars,
		rectangularSelection,
		crosshairCursor
	} = viewModule;

	// Seeding and cursor restoration are programmatic, so their transactions must
	// not read back as user edits (which would schedule a save) or move the status
	// bar's caret readout.
	let programmatic = false;

	const view = new EditorView({
		parent,
		state: EditorState.create({
			doc,
			extensions: [
				EditorView.lineWrapping,
				EditorState.tabSize.of(4),
				history(),
				drawSelection(),
				dropCursor(),
				highlightSpecialChars(),
				highlightActiveLine(),
				rectangularSelection(),
				crosshairCursor(),
				syntaxHighlighting(buildHighlightStyle(HighlightStyle, tags)),
				buildTheme(EditorView),
				markdown(),
				keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap]),
				EditorView.updateListener.of((update) => {
					if (programmatic) return;
					if (update.docChanged) onChange(update.state.doc.toString());
					if (update.selectionSet || update.docChanged) {
						const head = update.state.selection.main.head;
						const line = update.state.doc.lineAt(head);
						onCursor(line.number, head - line.from + 1);
					}
				})
			]
		})
	});

	function setCursor(offset: number) {
		const target = Math.max(0, Math.min(offset, view.state.doc.length));
		if (target === view.state.selection.main.head) return;
		view.dispatch({ selection: { anchor: target } });
	}

	return {
		getText: () => view.state.doc.toString(),
		setText(text) {
			if (text === view.state.doc.toString()) return;
			programmatic = true;
			view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
			programmatic = false;
		},
		getCursorOffset: () => view.state.selection.main.head,
		setCursorOffset(offset) {
			programmatic = true;
			setCursor(offset);
			programmatic = false;
		},
		focus: () => view.focus(),
		requestMeasure: () => view.requestMeasure(),
		openSearch: () => {
			view.focus();
			openSearchPanel(view);
		},
		destroy: () => view.destroy()
	};
}
