import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode, Tree } from '@lezer/common';
import type { EditorState, Extension, Range, Text } from '@codemirror/state';
import {
	Decoration,
	ViewPlugin,
	WidgetType,
	hoverTooltip,
	type DecorationSet,
	type EditorView,
	type ViewUpdate
} from '@codemirror/view';
import { line, refreshDecorations, touched, treeChanged } from './decorate';
import { near, revealMoved, revealPoints, touchedLines } from './reveal';
import { COMMENT, COMMENT_BLOCK } from './syntax';
import { renderCell } from './table-inline';

const DEFINITION = /^[ \t]{0,3}\[\^([^\]\s]+)\]:[ \t]*/;
const REFERENCE = /\[\^([^\]\s]+)\]/g;
const CODE_NODES: Record<string, true> = {
	FencedCode: true,
	CodeBlock: true,
	InlineCode: true,
	InlineMath: true,
	BlockMath: true,
	Frontmatter: true,
	[COMMENT]: true,
	[COMMENT_BLOCK]: true
};

interface Definition {
	from: number;
	to: number;
	body: number;
	text: string;
}

interface Reference {
	from: number;
	to: number;
	id: string;
}

export interface FootnoteIndex {
	definitions: Map<string, Definition>;
	references: Reference[];
	numbers: Map<string, number>;
	signature: string;
}

const indexes = new WeakMap<Tree, { doc: Text; index: FootnoteIndex }>();

function inCode(state: EditorState, pos: number): boolean {
	let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, 1);
	while (node) {
		if (CODE_NODES[node.name]) return true;
		node = node.parent;
	}
	return false;
}

// IDs are case-insensitive, and numbers follow the order of first reference, as in Obsidian's reading view.
function buildIndex(state: EditorState): FootnoteIndex {
	const lines: { from: number; text: string }[] = [];
	let pos = 0;
	for (const text of state.doc.iterLines()) {
		if (text.includes('[^')) lines.push({ from: pos, text });
		pos += text.length + 1;
	}
	const definitions = new Map<string, Definition>();
	for (const at of lines) {
		const match = DEFINITION.exec(at.text);
		if (!match || inCode(state, at.from)) continue;
		const id = match[1].toLowerCase();
		if (definitions.has(id)) continue;
		const from = at.from + at.text.indexOf('[^');
		const body = at.from + match[0].length;
		definitions.set(id, {
			from,
			to: from + match[1].length + 4,
			body,
			text: at.text.slice(match[0].length)
		});
	}
	const references: Reference[] = [];
	const numbers = new Map<string, number>();
	for (const at of definitions.size ? lines : []) {
		for (const match of at.text.matchAll(REFERENCE)) {
			const from = at.from + (match.index ?? 0);
			const id = match[1].toLowerCase();
			if (definitions.get(id)?.from === from || !definitions.has(id) || inCode(state, from))
				continue;
			references.push({ from, to: from + match[0].length, id });
			if (!numbers.has(id)) numbers.set(id, numbers.size + 1);
		}
	}
	const signature = [...numbers].map(([id, n]) => `${id}=${n}`).join(',');
	return { definitions, references, numbers, signature };
}

export function footnoteIndex(state: EditorState): FootnoteIndex {
	const tree = syntaxTree(state);
	const cached = indexes.get(tree);
	if (cached?.doc === state.doc) return cached.index;
	const index = buildIndex(state);
	indexes.set(tree, { doc: state.doc, index });
	return index;
}

class FootnoteWidget extends WidgetType {
	constructor(
		readonly label: string,
		readonly target: number | null,
		readonly definition: boolean
	) {
		super();
	}

	eq(other: FootnoteWidget): boolean {
		return (
			other.label === this.label &&
			other.target === this.target &&
			other.definition === this.definition
		);
	}

	ignoreEvent(): boolean {
		return false;
	}

	toDOM(view: EditorView): HTMLElement {
		const wrap = document.createElement(this.definition ? 'span' : 'sup');
		wrap.className = this.definition ? 'cm-lp-footnote-number' : 'cm-lp-footnote-ref';
		const link = document.createElement('button');
		link.type = 'button';
		link.className = 'cm-lp-footnote-link';
		link.textContent = this.definition ? `${this.label}.` : this.label;
		link.addEventListener('mousedown', (event) => {
			event.preventDefault();
			event.stopPropagation();
		});
		link.addEventListener('click', (event) => {
			event.preventDefault();
			event.stopPropagation();
			if (this.target === null) return;
			view.dispatch({ selection: { anchor: this.target }, scrollIntoView: true });
			view.focus();
		});
		wrap.appendChild(link);
		return wrap;
	}
}

function buildFootnotes(state: EditorState): DecorationSet {
	const touchedSet = touchedLines(state);
	const points = revealPoints(state);
	const index = footnoteIndex(state);
	const ranges: Range<Decoration>[] = [];

	for (const [id, definition] of index.definitions) {
		const at = state.doc.lineAt(definition.from);
		ranges.push(line(at.from, 'cm-lp-footnote-def'));
		if (touched(state, touchedSet, at.from, at.to)) continue;
		const first = index.references.find((reference) => reference.id === id)?.from ?? null;
		const label = String(index.numbers.get(id) ?? id);
		ranges.push(
			Decoration.replace({ widget: new FootnoteWidget(label, first, true) }).range(
				definition.from,
				definition.to
			)
		);
	}

	for (const reference of index.references) {
		if (near(points, reference.from, reference.to)) continue;
		const target = index.definitions.get(reference.id)?.body ?? null;
		const label = String(index.numbers.get(reference.id));
		ranges.push(
			Decoration.replace({ widget: new FootnoteWidget(label, target, false) }).range(
				reference.from,
				reference.to
			)
		);
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
			!update.transactions.some(revealMoved) &&
			!treeChanged(update) &&
			!update.transactions.some((tr) => tr.effects.some((e) => e.is(refreshDecorations)))
		) {
			return;
		}
		this.decorations = buildFootnotes(update.state);
	}
}

const footnotePreview = hoverTooltip((view, pos) => {
	const index = footnoteIndex(view.state);
	const reference = index.references.find((entry) => pos >= entry.from && pos <= entry.to);
	const definition = reference && index.definitions.get(reference.id);
	if (!reference || !definition) return null;
	return {
		pos: reference.from,
		end: reference.to,
		above: true,
		create: () => {
			const dom = document.createElement('div');
			dom.className = 'cm-lp-footnote-preview';
			dom.append(renderCell(definition.text));
			return { dom };
		}
	};
});

export const liveFootnotes: Extension = [
	ViewPlugin.fromClass(LiveFootnotes, { decorations: (plugin) => plugin.decorations }),
	footnotePreview
];
