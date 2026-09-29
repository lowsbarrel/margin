import type { ChangeSpec, Extension, Line, Range, Transaction } from '@codemirror/state';
import { EditorState, StateField } from '@codemirror/state';
import { Decoration, EditorView, ViewPlugin, keymap, type DecorationSet } from '@codemirror/view';
import { staticPreview } from './context';
import { refreshDecorations, treeChanged } from './decorate';
import { footnoteIndex } from './footnotes';
import { ESCAPE_TABLE, onEscape } from './escape';
import { afterMove, cellSync } from './table-cell';
import { onDelimiterRow, readTables, tableAt, type TableModel } from './table-model';
import {
	cellAt,
	enterCell,
	enterTable,
	leaveDelimiter,
	leaveTable,
	locate,
	nextCell,
	previousCell
} from './table-nav';
import { cellRange, sameRange, selectTableAbove, type CellRange } from './table-range';
import { TableWidget } from './table-widget';
import './tables.css';

interface Active {
	table: number;
	row: number;
	col: number;
}

interface Tables {
	active: Active | null;
	drafting: number | null;
	range: CellRange | null;
	epoch: number;
	decorations: DecorationSet;
}

function activeCell(state: EditorState): Active | null {
	if (state.facet(staticPreview) || state.readOnly || !state.facet(EditorView.editable))
		return null;
	if (state.selection.ranges.length > 1) return null;
	const { from, to } = state.selection.main;
	const start = cellAt(state, from);
	if (!start) return null;
	if (to !== from) {
		const end = cellAt(state, to);
		if (
			!end ||
			end.table.from !== start.table.from ||
			end.row !== start.row ||
			end.col !== start.col
		)
			return null;
	}
	return { table: start.table.from, row: start.row, col: start.col };
}

function sameActive(a: Active | null, b: Active | null): boolean {
	return a?.table === b?.table && a?.row === b?.row && a?.col === b?.col;
}

function draftedTable(state: EditorState): number | null {
	const { head } = state.selection.main;
	const table = tableAt(state, head);
	return table && onDelimiterRow(state, table, head) ? table.from : null;
}

function tableDecorations(state: EditorState, value: Omit<Tables, 'decorations'>): DecorationSet {
	const { active, drafting, range } = value;
	const epoch = `${value.epoch}:${footnoteIndex(state).signature}`;
	const ranges: Range<Decoration>[] = [];
	for (const table of readTables(state)) {
		if (table.from === drafting) continue;
		const cell = active?.table === table.from ? { row: active.row, col: active.col } : null;
		const cells = range?.table === table.from ? range : null;
		const source = state.doc.sliceString(table.from, table.to);
		ranges.push(
			Decoration.replace({
				block: true,
				widget: new TableWidget(table, source, cell, cells, epoch)
			}).range(table.from, table.to)
		);
	}
	return Decoration.set(ranges, true);
}

// Tables never fall back to raw Markdown in live preview, except while their delimiter row is typed.
const liveTables = StateField.define<Tables>({
	create(state) {
		const value = {
			active: activeCell(state),
			drafting: draftedTable(state),
			range: state.field(cellRange, false) ?? null,
			epoch: 0
		};
		return { ...value, decorations: tableDecorations(state, value) };
	},
	update(value, tr) {
		const refresh = tr.effects.some((effect) => effect.is(refreshDecorations));
		const rebuild = tr.docChanged || refresh || treeChanged(tr);
		const range = tr.state.field(cellRange, false) ?? null;
		const ranged = !sameRange(range, value.range);
		if (!rebuild && !tr.selection && !ranged) return value;
		const active = activeCell(tr.state);
		const drafting = draftedTable(tr.state);
		const same = sameActive(active, value.active) && drafting === value.drafting;
		if (!rebuild && !ranged && same) return value;
		const next = { active, drafting, range, epoch: value.epoch + (refresh ? 1 : 0) };
		return { ...next, decorations: tableDecorations(tr.state, next) };
	},
	provide: (field) => [
		EditorView.decorations.from(field, (value) => value.decorations),
		EditorView.editorAttributes.from(
			field,
			(value): Record<string, string> =>
				value.active || value.range ? { class: 'cm-lp-cell-editing' } : {}
		)
	]
});

// Escape leaves the table only when no higher-priority feature is answering it.
class TableEscape {
	readonly release: () => void;

	constructor(view: EditorView) {
		this.release = onEscape(view, ESCAPE_TABLE, () => (locate(view) ? leaveTable(view) : false));
	}

	destroy(): void {
		this.release();
	}
}

const tableEscape = ViewPlugin.fromClass(TableEscape);

const tableKeymap = keymap.of([
	{ key: 'Tab', run: (view) => afterMove(view, nextCell(view)) },
	{ key: 'Shift-Tab', run: (view) => afterMove(view, previousCell(view)) },
	{ key: 'Enter', run: (view) => afterMove(view, enterCell(view) || leaveDelimiter(view)) },
	{ key: 'ArrowDown', run: (view) => afterMove(view, enterTable(view, 1)) },
	{ key: 'ArrowUp', run: (view) => afterMove(view, enterTable(view, -1)) },
	{ key: 'Backspace', run: selectTableAbove }
]);

function gapBreak(tr: Transaction, gap: Line, table: TableModel | null, gapAbove: boolean) {
	if (!table) return null;
	const pos = gapAbove ? tr.changes.mapPos(gap.to, 1) : tr.changes.mapPos(gap.from, -1);
	const line = tr.newDoc.lineAt(pos);
	if (!line.text.trim() || line.text.trimStart().startsWith('|')) return null;
	return { from: gapAbove ? line.to : line.from, insert: '\n' };
}

const keepTableGaps = EditorState.transactionFilter.of((tr) => {
	if (!tr.docChanged || !tr.isUserEvent('input')) return tr;
	const state = tr.startState;
	const { doc } = state;
	const breaks: ChangeSpec[] = [];
	tr.changes.iterChangedRanges((fromA, toA) => {
		const gap = doc.lineAt(fromA);
		if (gap.text.trim() || doc.lineAt(toA).number !== gap.number) return;
		const next = gap.number < doc.lines ? doc.line(gap.number + 1) : null;
		const prev = gap.number > 1 ? doc.line(gap.number - 1) : null;
		const tableBelow = next && tableAt(state, next.from);
		const tableAbove = prev && tableAt(state, prev.from);
		const spec =
			gapBreak(tr, gap, tableBelow?.from === next?.from ? tableBelow : null, true) ??
			gapBreak(tr, gap, tableAbove?.to === prev?.to ? tableAbove : null, false);
		if (spec) breaks.push(spec);
	});
	return breaks.length ? [tr, { changes: breaks, sequential: true }] : tr;
});

export const liveTableEditing: Extension = [
	cellRange,
	liveTables,
	cellSync,
	tableEscape,
	tableKeymap,
	keepTableGaps
];
