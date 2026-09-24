import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';
import type { EditorState, Extension, Range } from '@codemirror/state';
import {
	Decoration,
	ViewPlugin,
	WidgetType,
	type DecorationSet,
	type EditorView,
	type ViewUpdate
} from '@codemirror/view';
import { hide, line, mark, refreshDecorations, touched, touchedLines } from './decorate';

const DEFINITION = /^[ \t]{0,3}\[\^([^\]\s]+)\]:[ \t]*/;
const REFERENCE = /\[\^([^\]\s]+)\]/g;
const CODE_NODES: Record<string, true> = {
	FencedCode: true,
	CodeBlock: true,
	InlineCode: true,
	InlineMath: true,
	BlockMath: true,
	Frontmatter: true
};

function inCode(state: EditorState, pos: number): boolean {
	let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, 1);
	while (node) {
		if (CODE_NODES[node.name]) return true;
		node = node.parent;
	}
	return false;
}

class FootnoteRefWidget extends WidgetType {
	readonly id: string;
	readonly target: number;

	constructor(id: string, target: number) {
		super();
		this.id = id;
		this.target = target;
	}

	eq(other: FootnoteRefWidget): boolean {
		return other.id === this.id && other.target === this.target;
	}

	ignoreEvent(): boolean {
		return false;
	}

	toDOM(view: EditorView): HTMLElement {
		const sup = document.createElement('sup');
		sup.className = 'cm-lp-footnote-ref';
		const link = document.createElement('button');
		link.type = 'button';
		link.className = 'cm-lp-footnote-link';
		link.textContent = this.id;
		link.addEventListener('mousedown', (event) => {
			event.preventDefault();
			event.stopPropagation();
		});
		link.addEventListener('click', (event) => {
			event.preventDefault();
			event.stopPropagation();
			view.dispatch({ selection: { anchor: this.target }, scrollIntoView: true });
			view.focus();
		});
		sup.appendChild(link);
		return sup;
	}
}

function buildFootnotes(state: EditorState): DecorationSet {
	const doc = state.doc;
	const touchedSet = touchedLines(state);
	const definitions = new Map<string, number>();
	const definitionStarts = new Map<number, number>();
	const ranges: Range<Decoration>[] = [];

	for (let number = 1; number <= doc.lines; number++) {
		const at = doc.line(number);
		const match = DEFINITION.exec(at.text);
		if (!match || inCode(state, at.from)) continue;
		const start = at.text.indexOf('[^');
		const end = start + match[1].length + 4;
		definitions.set(match[1], at.from + end);
		definitionStarts.set(number, start);
		ranges.push(line(at.from, 'cm-lp-footnote-def'));
		if (touched(state, touchedSet, at.from, at.to)) continue;
		ranges.push(hide(at.from + start, at.from + start + 2));
		ranges.push(hide(at.from + end - 2, at.from + end));
		ranges.push(mark(at.from + start + 2, at.from + end - 2, 'cm-lp-footnote-label'));
	}

	for (let number = 1; number <= doc.lines; number++) {
		const at = doc.line(number);
		const marker = definitionStarts.get(number);
		for (const match of at.text.matchAll(REFERENCE)) {
			if (match.index === undefined) continue;
			if (marker !== undefined && match.index === marker) continue;
			const from = at.from + match.index;
			const target = definitions.get(match[1]);
			if (target === undefined || inCode(state, from)) continue;
			ranges.push(
				Decoration.replace({ widget: new FootnoteRefWidget(match[1], target) }).range(
					from,
					from + match[0].length
				)
			);
		}
	}

	return Decoration.set(ranges, true);
}

class LiveFootnotes {
	decorations: DecorationSet;

	constructor(view: EditorView) {
		this.decorations = buildFootnotes(view.state);
	}

	update(update: ViewUpdate) {
		if (
			!update.docChanged &&
			!update.selectionSet &&
			!update.viewportChanged &&
			!update.transactions.some((tr) => tr.effects.some((e) => e.is(refreshDecorations)))
		) {
			return;
		}
		this.decorations = buildFootnotes(update.state);
	}
}

export const liveFootnotes: Extension = ViewPlugin.fromClass(LiveFootnotes, {
	decorations: (plugin) => plugin.decorations
});
