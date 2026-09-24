import type { EditorView } from '@codemirror/view';

export const ESCAPE_COMPLETION = 0;
export const ESCAPE_FIND = 1;
export const ESCAPE_BUBBLE = 2;
export const ESCAPE_TABLE = 3;

interface Handler {
	order: number;
	run: () => boolean;
}

const handlers = new Map<EditorView, Handler[]>();
let listening = false;

function onKeydown(event: KeyboardEvent): void {
	if (event.key !== 'Escape') return;
	for (const [view, list] of handlers) {
		if (!view.hasFocus) continue;
		for (const handler of list) {
			if (!handler.run()) continue;
			event.preventDefault();
			return;
		}
	}
}

// The app's capture-phase Escape handler is global, so one dispatcher decides which feature answers.
export function onEscape(view: EditorView, order: number, run: () => boolean): () => void {
	if (!listening) {
		window.addEventListener('keydown', onKeydown, true);
		listening = true;
	}
	const list = handlers.get(view) ?? [];
	const handler = { order, run };
	list.push(handler);
	list.sort((a, b) => a.order - b.order);
	handlers.set(view, list);
	return () => {
		const current = handlers.get(view);
		if (!current) return;
		const at = current.indexOf(handler);
		if (at >= 0) current.splice(at, 1);
		if (!current.length) handlers.delete(view);
	};
}
