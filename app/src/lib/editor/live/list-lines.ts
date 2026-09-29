import { syntaxTree } from '@codemirror/language';
import type { EditorState, Range } from '@codemirror/state';
import {
	Decoration,
	ViewPlugin,
	WidgetType,
	type DecorationSet,
	type EditorView,
	type ViewUpdate
} from '@codemirror/view';
import type { SyntaxNode } from '@lezer/common';
import { hide, refreshDecorations, treeChanged } from './decorate';
import './list-lines.css';

const TEXT_BLOCKS: Record<string, true> = { Paragraph: true, Task: true };

class IndentWidget extends WidgetType {
	constructor(readonly level: number) {
		super();
	}

	eq(other: IndentWidget): boolean {
		return other.level === this.level;
	}

	toDOM(): HTMLElement {
		const indent = document.createElement('span');
		indent.className = 'cm-lp-list-indent';
		indent.style.setProperty('--lp-list-level', String(this.level));
		return indent;
	}
}

function levelOf(item: SyntaxNode): number {
	let level = 0;
	for (let at = item.parent; at; at = at.parent) if (at.name === 'ListItem') level++;
	return level;
}

function kindOf(item: SyntaxNode): string {
	const ordered = item.parent?.name === 'OrderedList';
	if (!item.getChild('Task')) return ordered ? 'number' : 'bullet';
	return ordered ? 'numbered-task' : 'task';
}

function lineStyle(className: string, level: number): Decoration {
	return Decoration.line({ class: className, attributes: { style: `--lp-list-level: ${level}` } });
}

function itemDecorations(state: EditorState, item: SyntaxNode, out: Range<Decoration>[]): void {
	const mark = item.getChild('ListMark');
	if (!mark) return;
	const doc = state.doc;
	const first = doc.lineAt(mark.from);
	const level = levelOf(item);
	const kind = `cm-lp-list-${kindOf(item)}`;
	out.push(lineStyle(`cm-lp-list-line ${kind}`, level).range(first.from));
	if (mark.from > first.from)
		out.push(Decoration.replace({ widget: new IndentWidget(level) }).range(first.from, mark.from));
	for (let child = item.firstChild; child; child = child.nextSibling) {
		if (!TEXT_BLOCKS[child.name]) continue;
		for (let at = doc.lineAt(child.from); at.from < child.to; at = doc.line(at.number + 1)) {
			if (at.number !== first.number) {
				out.push(lineStyle(`cm-lp-list-cont ${kind}`, level).range(at.from));
				const lead = /^[ \t]*/.exec(at.text)?.[0].length ?? 0;
				if (lead) out.push(hide(at.from, at.from + lead));
			}
			if (at.number === doc.lines) break;
		}
	}
}

function build(view: EditorView): DecorationSet {
	const out: Range<Decoration>[] = [];
	const seen = new Set<number>();
	const tree = syntaxTree(view.state);
	for (const { from, to } of view.visibleRanges)
		tree.iterate({
			from,
			to,
			enter(ref) {
				if (ref.name !== 'ListItem' || seen.has(ref.from)) return;
				seen.add(ref.from);
				itemDecorations(view.state, ref.node, out);
			}
		});
	return Decoration.set(out, true);
}

export const listLines = ViewPlugin.fromClass(
	class {
		decorations: DecorationSet;

		constructor(view: EditorView) {
			this.decorations = build(view);
		}

		update(update: ViewUpdate): void {
			const refresh = update.transactions.some((tr) =>
				tr.effects.some((effect) => effect.is(refreshDecorations))
			);
			if (update.docChanged || update.viewportChanged || treeChanged(update) || refresh)
				this.decorations = build(update.view);
		}
	},
	{ decorations: (plugin) => plugin.decorations }
);
