import { editor } from '$lib/stores/editor.svelte';
import type { EditorView } from '@codemirror/view';

let pendingText = $state<string | null>(null);

function scrollToText(view: EditorView, searchText: string): void {
	const text = view.state.doc.toString();
	const at = text.indexOf(searchText);
	if (at < 0) return;
	view.dispatch({ selection: { anchor: at, head: at + searchText.length } });
	const scrollContainer = view.dom.closest('.editor-container');
	if (!scrollContainer) return;
	// A frame later: setting the selection reveals raw Markdown on the matched lines, which changes the line heights the centring depends on.
	requestAnimationFrame(() => {
		try {
			const coords = view.coordsAtPos(at);
			if (!coords) return;
			const rect = scrollContainer.getBoundingClientRect();
			const relativeTop = coords.top - rect.top + scrollContainer.scrollTop;
			scrollContainer.scrollTo({
				top: Math.max(0, relativeTop - rect.height / 2),
				behavior: 'smooth'
			});
		} catch {}
	});
}

export function scrollEditorToText(searchText: string): void {
	const view = editor.view;
	if (!view) {
		pendingText = searchText;
		return;
	}
	scrollToText(view, searchText);
}

export function initPendingScroll(): void {
	$effect(() => {
		const view = editor.view;
		const text = pendingText;
		if (view && text) {
			pendingText = null;
			scrollToText(view, text);
		}
	});
}
