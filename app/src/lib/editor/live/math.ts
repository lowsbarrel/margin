import type { SyntaxNode } from '@lezer/common';
import type { EditorState, Extension, Line, Range } from '@codemirror/state';
import { StateField } from '@codemirror/state';
import {
	Decoration,
	EditorView,
	ViewPlugin,
	WidgetType,
	type DecorationSet,
	type ViewUpdate
} from '@codemirror/view';
import * as m from '$lib/paraglide/messages.js';
import { nodesNamed } from './doc-index';
import { collapsedLines, hide, mark, refreshDecorations, touched, treeChanged } from './decorate';
import { near, revealLinesMoved, revealMoved, revealPoints, touchedLines } from './reveal';
import { paintMath } from './math-render';
import { BLOCK_MATH, BLOCK_MATH_MARK, INLINE_MATH } from './syntax';

interface MathBlock {
	from: number;
	to: number;
	line: Line;
	text: string;
}

function marksOf(node: SyntaxNode): SyntaxNode[] {
	const marks: SyntaxNode[] = [];
	for (let child = node.firstChild; child; child = child.nextSibling) {
		if (child.name === BLOCK_MATH_MARK) marks.push(child);
	}
	return marks;
}

function mathInner(state: EditorState, node: SyntaxNode): string {
	const marks = marksOf(node);
	if (marks.length < 2) return state.doc.sliceString(node.from, node.to).replace(/\$/g, '').trim();
	const doc = state.doc;
	const first = doc.lineAt(marks[0].from);
	const last = doc.lineAt(marks[1].from);
	if (first.number === last.number) return doc.sliceString(marks[0].to, marks[1].from);
	const from = Math.min(first.to + 1, doc.length);
	return doc.sliceString(from, Math.max(from, last.from - 1));
}

function blockMathOf(state: EditorState): MathBlock[] {
	const doc = state.doc;
	const out: MathBlock[] = [];
	for (const ref of nodesNamed(state, BLOCK_MATH)) {
		const at = doc.lineAt(ref.from);
		out.push({ from: at.from, to: ref.to, line: at, text: mathInner(state, ref) });
	}
	return out;
}

class MathInlineWidget extends WidgetType {
	readonly text: string;

	constructor(text: string) {
		super();
		this.text = text;
	}

	eq(other: MathInlineWidget): boolean {
		return other.text === this.text;
	}

	ignoreEvent(): boolean {
		return false;
	}

	toDOM(): HTMLElement {
		const span = document.createElement('span');
		span.className = 'cm-lp-math-inline';
		span.contentEditable = 'false';
		paintMath(span, this.text, false);
		return span;
	}
}

class MathBlockWidget extends WidgetType {
	readonly text: string;

	constructor(text: string) {
		super();
		this.text = text;
	}

	eq(other: MathBlockWidget): boolean {
		return other.text === this.text;
	}

	ignoreEvent(): boolean {
		return false;
	}

	toDOM(): HTMLElement {
		const block = document.createElement('div');
		block.className = 'cm-lp-math-block';
		block.contentEditable = 'false';
		const content = document.createElement('div');
		content.className = 'cm-lp-math-content';
		block.appendChild(content);
		if (!this.text.trim()) {
			const empty = document.createElement('span');
			empty.className = 'cm-lp-math-empty';
			empty.textContent = m.editor_empty_math();
			content.appendChild(empty);
			return block;
		}
		paintMath(content, this.text, true);
		return block;
	}
}

// Only a state field may replace a line break, and a fenced $$ block spans lines.
function blockMathDecorations(state: EditorState, touchedSet: ReadonlySet<number>): DecorationSet {
	const ranges: Range<Decoration>[] = [];
	for (const block of blockMathOf(state)) {
		if (touched(state, touchedSet, block.from, block.to)) {
			ranges.push(mark(block.from, block.to, 'cm-lp-dim'));
			continue;
		}
		ranges.push(hide(block.from, block.to));
		ranges.push(...collapsedLines(state, block.from, block.to));
		ranges.push(
			Decoration.widget({ block: true, widget: new MathBlockWidget(block.text), side: 1 }).range(
				block.to
			)
		);
	}
	return Decoration.set(ranges, true);
}

const mathBlocks = StateField.define<DecorationSet>({
	create: (state) => blockMathDecorations(state, touchedLines(state)),
	update(value, tr) {
		if (
			tr.docChanged ||
			treeChanged(tr) ||
			revealLinesMoved(tr) ||
			tr.effects.some((e) => e.is(refreshDecorations))
		)
			return blockMathDecorations(tr.state, touchedLines(tr.state));
		return value.map(tr.changes);
	},
	provide: (field) => EditorView.decorations.from(field)
});

function buildMath(state: EditorState): DecorationSet {
	const doc = state.doc;
	const points = revealPoints(state);
	const ranges: Range<Decoration>[] = [];

	for (const ref of nodesNamed(state, INLINE_MATH)) {
		if (near(points, ref.from, ref.to)) {
			ranges.push(mark(ref.from, ref.to, 'cm-lp-dim'));
			continue;
		}
		const text = doc.sliceString(ref.from + 1, Math.max(ref.from + 1, ref.to - 1));
		ranges.push(Decoration.replace({ widget: new MathInlineWidget(text) }).range(ref.from, ref.to));
	}

	return Decoration.set(ranges, true);
}

class LiveMath {
	decorations: DecorationSet;

	constructor(view: EditorView) {
		this.decorations = buildMath(view.state);
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
		this.decorations = buildMath(update.state);
	}
}

export const liveMath: readonly Extension[] = [
	mathBlocks,
	ViewPlugin.fromClass(LiveMath, { decorations: (plugin) => plugin.decorations })
];
