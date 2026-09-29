import type { EditorState, Range } from '@codemirror/state';
import {
	Decoration,
	ViewPlugin,
	WidgetType,
	type DecorationSet,
	type EditorView,
	type ViewUpdate
} from '@codemirror/view';
import { nodesNamed } from './doc-index';
import { hide, mark, refreshDecorations, treeChanged } from './decorate';
import { near, revealMoved, revealPoints } from './reveal';

export const LINE_BREAK = /^<br\s*\/?>/i;

// Bare formatting tags only: attributes never reach the DOM, so note HTML cannot inject markup or script.
export const INLINE_HTML: Record<string, string> = {
	kbd: 'cm-lp-kbd',
	u: 'cm-lp-underline',
	ins: 'cm-lp-underline',
	sub: 'cm-lp-sub',
	sup: 'cm-lp-sup',
	mark: 'cm-lp-highlight',
	b: 'cm-lp-strong',
	strong: 'cm-lp-strong',
	i: 'cm-lp-em',
	em: 'cm-lp-em',
	s: 'cm-lp-strike',
	del: 'cm-lp-strike',
	small: 'cm-lp-small'
};

const OPEN = /^<([a-z]+)>$/i;
const CLOSE = /^<\/([a-z]+)>$/i;

interface Tag {
	name: string;
	from: number;
	to: number;
	parent: number;
}

class BreakWidget extends WidgetType {
	eq(): boolean {
		return true;
	}

	toDOM(): HTMLElement {
		return document.createElement('br');
	}
}

function tagName(text: string): string | null {
	if (LINE_BREAK.test(text)) return 'br';
	const open = OPEN.exec(text);
	if (open) return open[1].toLowerCase();
	const close = CLOSE.exec(text);
	return close ? `/${close[1].toLowerCase()}` : null;
}

function tagsOf(state: EditorState): Tag[] {
	const tags: Tag[] = [];
	for (const ref of nodesNamed(state, 'HTMLTag')) {
		const name = tagName(state.sliceDoc(ref.from, ref.to));
		if (name) tags.push({ name, from: ref.from, to: ref.to, parent: ref.parent?.from ?? -1 });
	}
	return tags;
}

function openerOf(open: Tag[], close: Tag): number {
	for (let at = open.length - 1; at >= 0; at--)
		if (open[at].name === close.name.slice(1) && open[at].parent === close.parent) return at;
	return -1;
}

function inlineHtml(state: EditorState): DecorationSet {
	const points = revealPoints(state);
	const ranges: Range<Decoration>[] = [];
	const open: Tag[] = [];
	for (const tag of tagsOf(state)) {
		if (tag.name === 'br') {
			if (!near(points, tag.from, tag.to))
				ranges.push(Decoration.replace({ widget: new BreakWidget() }).range(tag.from, tag.to));
			continue;
		}
		if (!tag.name.startsWith('/')) {
			if (INLINE_HTML[tag.name]) open.push(tag);
			continue;
		}
		const at = openerOf(open, tag);
		if (at < 0) continue;
		const [start] = open.splice(at);
		if (near(points, start.from, tag.to)) continue;
		ranges.push(hide(start.from, start.to), hide(tag.from, tag.to));
		if (start.to < tag.from) ranges.push(mark(start.to, tag.from, INLINE_HTML[start.name]));
	}
	return Decoration.set(ranges, true);
}

export const liveInlineHtml = ViewPlugin.fromClass(
	class {
		decorations: DecorationSet;

		constructor(view: EditorView) {
			this.decorations = inlineHtml(view.state);
		}

		update(update: ViewUpdate): void {
			const refresh = update.transactions.some((tr) =>
				tr.effects.some((effect) => effect.is(refreshDecorations))
			);
			if (
				update.docChanged ||
				refresh ||
				treeChanged(update) ||
				update.transactions.some(revealMoved)
			)
				this.decorations = inlineHtml(update.state);
		}
	},
	{ decorations: (plugin) => plugin.decorations }
);
