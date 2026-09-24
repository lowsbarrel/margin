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

// A rendered widget is not editable text, so a drag that starts on it has to extend the selection itself.
export function dragSelection(view: EditorView, anchor: number): void {
	const move = (event: MouseEvent): void => {
		const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
		if (pos == null) return;
		view.dispatch({ selection: { anchor, head: pos }, scrollIntoView: true });
	};
	const stop = (): void => {
		window.removeEventListener('mousemove', move);
		window.removeEventListener('mouseup', stop);
	};
	window.addEventListener('mousemove', move);
	window.addEventListener('mouseup', stop);
}
