import type { ChangeSpec, EditorState, Text } from '@codemirror/state';
import { minimalDiff } from '$lib/utils/text-diff';
import { rowTexts, tableAt, type Align, type TableModel } from './table-model';

const WIDE =
	/[\u1100-\u115F\u2E80-\uA4CF\uAC00-\uD7A3\uF900-\uFAFF\uFE30-\uFE4F\uFF00-\uFF60\uFFE0-\uFFE6\u{1F300}-\u{1FAFF}\u{20000}-\u{3FFFD}]/u;

export interface CellPlace {
	row: number;
	col: number;
}

export interface TableEdit {
	changes: ChangeSpec[];
	cellStart: (place: CellPlace) => number;
}

function widthOf(text: string): number {
	let width = 0;
	for (const char of text) width += WIDE.test(char) ? 2 : 1;
	return width;
}

function padded(text: string, width: number, align: Align): { text: string; lead: number } {
	const gap = Math.max(0, width - widthOf(text));
	const lead = align === 'right' ? gap : align === 'center' ? Math.floor(gap / 2) : 0;
	return { text: ' '.repeat(lead) + text + ' '.repeat(gap - lead), lead };
}

function delimiter(width: number, align: Align): string {
	if (align === 'left') return `:${'-'.repeat(width - 1)}`;
	if (align === 'right') return `${'-'.repeat(width - 1)}:`;
	if (align === 'center') return `:${'-'.repeat(width - 2)}:`;
	return '-'.repeat(width);
}

function lineOf(row: number): number {
	return row === 0 ? 0 : row + 1;
}

export function formatTable(rows: string[][], align: Align[]) {
	const widths = align.map((_, col) =>
		Math.max(3, ...rows.map((cells) => widthOf(cells[col] ?? '')))
	);
	const starts: number[][] = [];
	const body = rows.map((cells, row) => {
		let at = 2;
		starts[row] = [];
		const parts = widths.map((width, col) => {
			const cell = padded(cells[col] ?? '', width, align[col] ?? 'none');
			starts[row][col] = at + cell.lead;
			at += cell.text.length + 3;
			return cell.text;
		});
		return `| ${parts.join(' | ')} |`;
	});
	const rule = `| ${widths.map((width, col) => delimiter(width, align[col] ?? 'none')).join(' | ')} |`;
	return { lines: [body[0], rule, ...body.slice(1)], starts };
}

export function tableEdit(
	doc: Text,
	table: TableModel,
	rows: string[][],
	align: Align[]
): TableEdit {
	const { lines, starts } = formatTable(rows, align);
	const old = doc.sliceString(table.from, table.to).split('\n');
	const indents = old.map((line) => /^[ \t]*/.exec(line)?.[0] ?? '');
	const next = lines.map((line, index) => (indents[index] ?? indents[0]) + line);
	const lineStart = (index: number) =>
		table.from + next.slice(0, index).reduce((sum, line) => sum + line.length + 1, 0);
	const cellStart = ({ row, col }: CellPlace) =>
		lineStart(lineOf(row)) + (indents[lineOf(row)] ?? indents[0]).length + starts[row][col];
	const changes: ChangeSpec[] = [];
	if (old.length !== next.length) {
		const diff = minimalDiff(old.join('\n'), next.join('\n'));
		changes.push({ from: table.from + diff.from, to: table.from + diff.to, insert: diff.insert });
		return { changes, cellStart };
	}
	let pos = table.from;
	old.forEach((line, index) => {
		const diff = minimalDiff(line, next[index]);
		if (diff.from !== diff.to || diff.insert)
			changes.push({ from: pos + diff.from, to: pos + diff.to, insert: diff.insert });
		pos += line.length + 1;
	});
	return { changes, cellStart };
}

export function retypeCell(
	state: EditorState,
	at: number,
	place: CellPlace,
	text: string
): { changes: ChangeSpec[]; from: number } | null {
	const table = tableAt(state, at);
	if (!table?.rows[place.row] || place.col >= table.cols) return null;
	const rows = rowTexts(table).map((cells) => cells.slice());
	rows[place.row][place.col] = text;
	const edit = tableEdit(state.doc, table, rows, table.align);
	return { changes: edit.changes, from: edit.cellStart(place) };
}
