import {
	EditorSelection,
	type EditorState,
	type Text,
	type TransactionSpec
} from '@codemirror/state';
import type { SyntaxNode } from '@lezer/common';
import {
	ancestorAt,
	childColumn,
	columns,
	itemAt,
	leadingSpace,
	lineSpan,
	listAt,
	listEdit,
	prefersTabs,
	relocateItem,
	tokenFor,
	treeUpTo,
	type ListBlock,
	type ListItem,
	type Target
} from './list-tree';

export interface Block {
	from: number;
	to: number;
	anchor: number;
	node: string;
	item: ListItem | null;
	list: ListBlock | null;
}

export interface Gap {
	pos: number;
	after: boolean;
	prev: ListItem | null;
	next: ListItem | null;
	min: number;
	max: number;
}

export function blockAt(state: EditorState, pos: number, items: boolean): Block | null {
	const line = state.doc.lineAt(pos);
	const space = leadingSpace(line.text);
	if (space.length === line.length) return null;
	const start = line.from + space.length;
	if (items) {
		const list = listAt(state, start);
		const item = list && itemAt(list, start);
		if (list && item)
			return { from: item.from, to: item.to, anchor: item.mark, node: 'ListItem', item, list };
	}
	let top: SyntaxNode | null = null;
	const inner = treeUpTo(state, start).resolveInner(start, 1);
	for (let node: SyntaxNode | null = inner; node && node.name !== 'Document'; node = node.parent)
		top = node;
	if (!top) return null;
	const span = lineSpan(state.doc, top.from, top.to);
	return { ...span, anchor: span.from, node: top.name, item: null, list: null };
}

export function draggableAt(state: EditorState, pos: number): Block | null {
	const block = blockAt(state, pos, true);
	return block && block.node !== 'Frontmatter' ? block : null;
}

export function blockNear(
	state: EditorState,
	pos: number,
	items: boolean
): { block: Block; after: boolean | null } | null {
	const doc = state.doc;
	const line = doc.lineAt(pos);
	if (line.text.trim()) {
		const block = blockAt(state, pos, items);
		return block && { block, after: null };
	}
	for (let number = line.number - 1; number >= 1; number--) {
		if (!doc.line(number).text.trim()) continue;
		const block = blockAt(state, doc.line(number).from, items);
		return block && { block, after: true };
	}
	for (let number = line.number + 1; number <= doc.lines; number++) {
		if (!doc.line(number).text.trim()) continue;
		const block = blockAt(state, doc.line(number).from, items);
		return block && { block, after: false };
	}
	return null;
}

export function listGaps(doc: Text, list: ListBlock, skip: ListItem | null): Gap[] {
	const kept = skip
		? list.items.filter((item) => item.from < skip.from || item.to > skip.to)
		: list.items;
	const gaps: Gap[] = kept.map((item, index) => {
		const prev = kept[index - 1] ?? null;
		return {
			pos: item.from,
			after: false,
			prev,
			next: item,
			min: item.level,
			max: prev ? prev.level + 1 : item.level
		};
	});
	const last = kept.at(-1);
	if (last) {
		const holdsSkip = skip && last.from < skip.from && skip.to <= last.to;
		const end = holdsSkip ? doc.lineAt(skip.from).from - 1 : last.to;
		gaps.push({ pos: end, after: true, prev: last, next: null, min: 1, max: last.level + 1 });
	}
	return gaps;
}

export function dropItem(
	state: EditorState,
	item: ListItem,
	gap: Gap,
	level: number
): TransactionSpec | null {
	const tabs = prefersTabs(state, item.indent, gap.prev?.indent ?? '', gap.next?.indent ?? '');
	let ref: ListItem | null = null;
	let cols = 0;
	if (gap.next && gap.next.level === level) {
		ref = gap.next;
		cols = columns(state, ref.indent);
	} else if (gap.prev && level <= gap.prev.level) {
		ref = ancestorAt(gap.prev, level);
		cols = columns(state, ref.indent);
	} else if (gap.prev) {
		cols = childColumn(state, gap.prev, tabs);
	}
	const target: Target = gap.after
		? { pos: gap.pos, lead: '\n', tail: '' }
		: { pos: gap.pos, lead: '', tail: '\n' };
	return relocateItem(state, item, target, cols, tabs, tokenFor(state, ref, item));
}

function beside(doc: Text, block: Block, after: boolean): Target {
	if (after) {
		const last = doc.lineAt(block.to);
		const next = last.number < doc.lines ? doc.line(last.number + 1) : null;
		return { pos: block.to, lead: '\n\n', tail: next?.text.trim() ? '\n' : '' };
	}
	const first = doc.lineAt(block.from);
	const prev = first.number > 1 ? doc.line(first.number - 1) : null;
	return { pos: block.from, lead: prev?.text.trim() ? '\n' : '', tail: '\n\n' };
}

function blockRemoval(doc: Text, from: number, to: number): [number, number] {
	const first = doc.lineAt(from);
	const last = doc.lineAt(to);
	for (let number = last.number + 1; number <= doc.lines; number++) {
		const line = doc.line(number);
		if (line.text.trim()) return [from, line.from];
	}
	for (let number = first.number - 1; number >= 1; number--) {
		const line = doc.line(number);
		if (line.text.trim()) return [line.to, to];
	}
	return [from, to];
}

export function dropBeside(
	state: EditorState,
	dragged: Block,
	target: Block,
	after: boolean
): TransactionSpec | null {
	if (target.from <= dragged.to && dragged.from <= target.to) return null;
	const doc = state.doc;
	const spot = beside(doc, target, after);
	if (dragged.item) {
		const tabs = prefersTabs(state, dragged.item.indent);
		return relocateItem(state, dragged.item, spot, 0, tabs, tokenFor(state, null, dragged.item));
	}
	const [cutFrom, cutTo] = blockRemoval(doc, dragged.from, dragged.to);
	if (spot.pos > cutFrom && spot.pos < cutTo) return null;
	const text = doc.sliceString(dragged.from, dragged.to);
	return listEdit(
		state,
		[
			{ from: cutFrom, to: cutTo },
			{ from: spot.pos, insert: spot.lead + text + spot.tail }
		],
		() => [],
		(set) => {
			const start = set.mapPos(spot.pos, -1) + spot.lead.length;
			const moveTo = (pos: number) =>
				pos >= dragged.from && pos <= dragged.to ? start + pos - dragged.from : set.mapPos(pos);
			return EditorSelection.create(
				state.selection.ranges.map((range) =>
					EditorSelection.range(moveTo(range.anchor), moveTo(range.head))
				),
				state.selection.mainIndex
			);
		}
	);
}
