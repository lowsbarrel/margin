import { StateEffect, StateField, type Range } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';
import * as m from '$lib/paraglide/messages.js';
import { staticPreview } from './context';

const SVG = 'http://www.w3.org/2000/svg';

export const markSource = StateEffect.define<{ from: number; to: number } | null>();

export const sourceLines = StateField.define<DecorationSet>({
	create: () => Decoration.none,
	update(value, tr) {
		for (const effect of tr.effects) {
			if (!effect.is(markSource)) continue;
			if (!effect.value) return Decoration.none;
			const ranges: Range<Decoration>[] = [];
			for (let at = effect.value.from; at <= effect.value.to; ) {
				const line = tr.state.doc.lineAt(at);
				ranges.push(Decoration.line({ class: 'cm-lp-drag-source' }).range(line.from));
				at = line.to + 1;
			}
			return Decoration.set(ranges);
		}
		return value.map(tr.changes);
	},
	provide: (field) => EditorView.decorations.from(field)
});

export function dragEnabled(view: EditorView): boolean {
	const { state } = view;
	return !state.facet(staticPreview) && state.facet(EditorView.editable) && !state.readOnly;
}

export function scrollParent(element: HTMLElement): HTMLElement | null {
	for (let at = element.parentElement; at; at = at.parentElement) {
		const overflow = getComputedStyle(at).overflowY;
		if ((overflow === 'auto' || overflow === 'scroll') && at.scrollHeight > at.clientHeight)
			return at;
	}
	return null;
}

export function gripElement(): HTMLElement {
	const grip = document.createElement('div');
	grip.className = 'cm-lp-grip';
	grip.title = m.editor_block_handle();
	grip.setAttribute('aria-hidden', 'true');
	const svg = document.createElementNS(SVG, 'svg');
	svg.setAttribute('width', '10');
	svg.setAttribute('height', '14');
	svg.setAttribute('viewBox', '0 0 10 14');
	svg.setAttribute('fill', 'currentColor');
	for (const cx of [2.5, 7.5]) {
		for (const cy of [2.5, 7, 11.5]) {
			const dot = document.createElementNS(SVG, 'circle');
			dot.setAttribute('cx', String(cx));
			dot.setAttribute('cy', String(cy));
			dot.setAttribute('r', '1.3');
			svg.appendChild(dot);
		}
	}
	grip.appendChild(svg);
	return grip;
}
