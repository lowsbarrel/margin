import type { EditorView } from '@codemirror/view';

export interface TextRange {
	from: number;
	to: number;
}

export function captureSelection(view: EditorView): TextRange {
	const { from, to } = view.state.selection.main;
	return { from, to };
}

export function insertText(view: EditorView, range: TextRange, text: string): TextRange {
	if (!view.dom.isConnected) return range;
	const at = range.from + text.length;
	view.dispatch({
		changes: { from: range.from, to: range.to, insert: text },
		selection: { anchor: at },
		scrollIntoView: true,
		userEvent: 'input'
	});
	return { from: at, to: at };
}
