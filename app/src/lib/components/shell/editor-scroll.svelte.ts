import { editor } from '$lib/stores/editor.svelte';
import type { Editor } from '@tiptap/core';

let pendingText = $state<string | null>(null);

export function scrollEditorToText(searchText: string): void {
	const tiptap = editor.tiptap;
	if (!tiptap?.state?.doc) {
		pendingText = searchText;
		return;
	}
	scrollToText(tiptap, searchText);
}

function scrollToText(tiptap: Editor, searchText: string): void {
	const doc = tiptap.state.doc;
	let targetPos = -1;
	doc.descendants((node, pos) => {
		if (targetPos >= 0) return false;
		if (node.isText && node.text?.includes(searchText)) {
			targetPos = pos + node.text.indexOf(searchText);
			return false;
		}
	});
	if (targetPos < 0) return;
	tiptap.commands.setTextSelection(targetPos);
	try {
		const view = tiptap.view;
		const coords = view.coordsAtPos(targetPos);
		const scrollContainer = view.dom.closest('.editor-container');
		if (scrollContainer && coords) {
			const rect = scrollContainer.getBoundingClientRect();
			const relativeTop = coords.top - rect.top + scrollContainer.scrollTop;
			scrollContainer.scrollTo({
				top: relativeTop - rect.height / 2,
				behavior: 'smooth'
			});
		}
	} catch {}
}

export function initPendingScroll(): void {
	$effect(() => {
		const tiptap = editor.tiptap;
		const text = pendingText;
		if (tiptap?.state?.doc && text) {
			pendingText = null;
			scrollToText(tiptap, text);
		}
	});
}
