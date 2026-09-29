import { editor } from '$lib/stores/editor.svelte';
import type { EditorView } from '@codemirror/view';

type ScrollTarget = { text: string } | { subpath: string };

const HEADING = /^#{1,6}[ \t]+(.*?)[ \t#]*$/;

let pending = $state<ScrollTarget | null>(null);

function squash(text: string): string {
	return text.trim().replace(/\s+/g, ' ').toLowerCase();
}

// A `#^id` subpath names a block by the id at the end of its line; any other subpath is a heading.
function locate(view: EditorView, target: ScrollTarget): { from: number; to: number } | null {
	const doc = view.state.doc;
	if ('text' in target) {
		const at = doc.toString().indexOf(target.text);
		return at < 0 ? null : { from: at, to: at + target.text.length };
	}
	const wanted = squash(target.subpath);
	for (let number = 1; number <= doc.lines; number++) {
		const line = doc.line(number);
		if (wanted.startsWith('^')) {
			if (line.text.trimEnd().toLowerCase().endsWith(` ${wanted}`)) return line;
			continue;
		}
		const heading = HEADING.exec(line.text);
		if (!heading || squash(heading[1]) !== wanted) continue;
		const from = line.from + heading[0].indexOf(heading[1]);
		return { from, to: from + heading[1].length };
	}
	return null;
}

function scrollTo(view: EditorView, target: ScrollTarget): void {
	const found = locate(view, target);
	if (!found) return;
	view.dispatch({ selection: { anchor: found.from, head: found.to } });
	const scrollContainer = view.dom.closest('.editor-container');
	if (!scrollContainer) return;
	// A frame later: setting the selection reveals raw Markdown on the matched lines, which changes the line heights the centring depends on.
	requestAnimationFrame(() => {
		try {
			const coords = view.coordsAtPos(found.from);
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

function scrollEditor(target: ScrollTarget): void {
	const view = editor.view;
	if (!view) {
		pending = target;
		return;
	}
	scrollTo(view, target);
}

export function scrollEditorToText(searchText: string): void {
	scrollEditor({ text: searchText });
}

export function scrollEditorToSubpath(subpath: string): void {
	scrollEditor({ subpath });
}

export function initPendingScroll(): void {
	$effect(() => {
		const view = editor.view;
		const target = pending;
		if (view && target) {
			pending = null;
			scrollTo(view, target);
		}
	});
}
