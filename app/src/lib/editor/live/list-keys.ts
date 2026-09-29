import { indentLess, indentMore } from '@codemirror/commands';
import { EditorSelection, type ChangeSpec, type EditorState } from '@codemirror/state';
import type { EditorView, KeyBinding } from '@codemirror/view';
import type { SyntaxNode } from '@lezer/common';
import {
	childColumn,
	columns,
	itemAt,
	leadingSpace,
	listAt,
	listEdit,
	prefersTabs,
	relevel,
	relocateItem,
	tokenFor,
	treeUpTo,
	type ListBlock,
	type ListItem
} from './list-tree';

const CODE_BLOCK = /^(?:FencedCode|CodeBlock)$/;

interface Picked {
	block: ListBlock;
	items: ListItem[];
}

function contains(outer: ListItem, inner: ListItem): boolean {
	return outer !== inner && outer.from <= inner.from && inner.to <= outer.to;
}

function picked(state: EditorState): Picked | null {
	if (state.selection.ranges.length > 1) return null;
	const range = state.selection.main;
	const doc = state.doc;
	const first = doc.lineAt(range.from);
	const last = doc.lineAt(range.to);
	const block = listAt(state, first.from + leadingSpace(first.text).length);
	if (!block) return null;
	let items = block.items.filter((item) => item.from >= first.from && item.from <= last.to);
	if (!items.length) {
		const at = itemAt(block, range.head);
		items = at ? [at] : [];
	}
	items = items.filter((item) => !items.some((other) => contains(other, item)));
	return items.length ? { block, items } : null;
}

function inCodeBlock(state: EditorState): boolean {
	const pos = state.selection.main.head;
	for (
		let node: SyntaxNode | null = treeUpTo(state, pos).resolveInner(pos, -1);
		node;
		node = node.parent
	)
		if (CODE_BLOCK.test(node.name)) return true;
	return false;
}

function shift(
	view: EditorView,
	{ items }: Picked,
	cols: number,
	tabs: boolean,
	token: string | null
): boolean {
	const { state } = view;
	const delta = cols - columns(state, items[0].indent);
	const changes: ChangeSpec[] = [];
	const maps = items.map((item, index) => {
		const target = Math.max(0, columns(state, item.indent) + delta);
		const result = relevel(state, item, target, tabs, index === 0 ? token : null);
		changes.push({ from: item.from, to: item.to, insert: result.text });
		return { item, map: result.map };
	});
	const spec = listEdit(
		state,
		changes,
		(set) => [set.mapPos(items[0].from, -1)],
		(set) => {
			const moveTo = (pos: number) => {
				const hit = maps.find(({ item }) => item.from <= pos && pos <= item.to);
				return hit ? set.mapPos(hit.item.from, -1) + hit.map(pos - hit.item.from) : set.mapPos(pos);
			};
			return EditorSelection.create(
				state.selection.ranges.map((range) =>
					EditorSelection.range(moveTo(range.anchor), moveTo(range.head))
				),
				state.selection.mainIndex
			);
		}
	);
	if (spec) view.dispatch({ ...spec, userEvent: 'input.indent', scrollIntoView: true });
	return true;
}

export function indentItems(view: EditorView): boolean {
	const found = picked(view.state);
	if (!found) return false;
	const head = found.items[0];
	const siblings = head.parent?.children ?? found.block.roots;
	const prev = siblings[siblings.indexOf(head) - 1];
	if (!prev) return true;
	const state = view.state;
	const tabs = prefersTabs(state, prev.indent, head.indent);
	const last = prev.children.at(-1) ?? null;
	const cols = last ? columns(state, last.indent) : childColumn(state, prev, tabs);
	const token = found.items.length === 1 ? tokenFor(state, last, head) : null;
	return shift(view, found, cols, tabs, token);
}

export function outdentItems(view: EditorView): boolean {
	const found = picked(view.state);
	if (!found) return false;
	const head = found.items[0];
	const parent = head.parent;
	if (!parent) return true;
	const state = view.state;
	const tabs = prefersTabs(state, parent.indent, head.indent);
	const token = found.items.length === 1 ? tokenFor(state, parent, head) : null;
	return shift(view, found, columns(state, parent.indent), tabs, token);
}

function moveItem(view: EditorView, dir: -1 | 1): boolean {
	const found = picked(view.state);
	if (!found || found.items.length > 1) return false;
	const { state } = view;
	const item = found.items[0];
	const siblings = item.parent?.children ?? found.block.roots;
	const neighbor = siblings[siblings.indexOf(item) + dir];
	const tabs = prefersTabs(state, item.indent);
	let spec;
	if (neighbor) {
		const target =
			dir < 0
				? { pos: neighbor.from, lead: '', tail: '\n' }
				: { pos: neighbor.to, lead: '\n', tail: '' };
		spec = relocateItem(state, item, target, columns(state, item.indent), tabs, null);
	} else {
		const parent = item.parent;
		const uncles = parent?.parent?.children ?? found.block.roots;
		const uncle = parent ? uncles[uncles.indexOf(parent) + dir] : undefined;
		if (!uncle) return true;
		const edge = dir < 0 ? (uncle.children.at(-1) ?? null) : (uncle.children[0] ?? null);
		const cols = edge ? columns(state, edge.indent) : childColumn(state, uncle, tabs);
		const target =
			dir > 0 && edge
				? { pos: edge.from, lead: '', tail: '\n' }
				: { pos: uncle.to, lead: '\n', tail: '' };
		spec = relocateItem(state, item, target, cols, tabs, tokenFor(state, edge, item));
	}
	if (spec) view.dispatch({ ...spec, userEvent: 'move.line', scrollIntoView: true });
	return true;
}

function tabKey(view: EditorView): boolean {
	return (!inCodeBlock(view.state) && indentItems(view)) || indentMore(view);
}

function shiftTabKey(view: EditorView): boolean {
	return (!inCodeBlock(view.state) && outdentItems(view)) || indentLess(view);
}

export const listKeymap: readonly KeyBinding[] = [
	{ key: 'Tab', run: tabKey },
	{ key: 'Shift-Tab', run: shiftTabKey },
	{ key: 'Mod-]', run: tabKey },
	{ key: 'Mod-[', run: shiftTabKey },
	{ key: 'Alt-ArrowUp', run: (view) => moveItem(view, -1) },
	{ key: 'Alt-ArrowDown', run: (view) => moveItem(view, 1) }
];
