// GFM table serializer: a table is always written as Markdown, never as an HTML
// block, and every cell collapses to a single line so a saved note keeps its shape.

import { Table } from '@tiptap/extension-table';
import type { Node as PMNode } from '@tiptap/pm/model';
import { TableMap } from '@tiptap/pm/tables';

/**
 * Minimal structural type for the tiptap-markdown serializer state. tiptap-markdown
 * ships no .d.ts for its state subclass, so we declare just the members used here
 * instead of falling back to `any`.
 */
interface MarkdownSerializerState {
	out: string;
	closed: PMNode | null;
	inTable: boolean;
	write(content: string): void;
	render(node: PMNode, parent: PMNode | null, index: number): void;
	renderInline(node: PMNode): void;
	ensureNewLine(): void;
	closeBlock(node: PMNode): void;
}

// Delimiters per column, keyed by the align attribute the table extension shares with HTML.
const ALIGN_DELIMITERS: Record<string, string> = {
	left: ':---',
	center: ':---:',
	right: '---:'
};

const DELIMITER = '---';

/**
 * Render one block into the output and take the text back out again, leaving the
 * state as it was found: a cell is assembled by its caller, not appended in place.
 */
function capture(state: MarkdownSerializerState, render: () => void): string {
	const start = state.out.length;
	render();
	const text = state.out.slice(start);
	state.out = state.out.slice(0, start);
	// A nested serializer may have closed a block; a row is one line, so no separator.
	state.closed = null;
	return text.replace(/^\n+|\n+$/g, '').replace(/\n+/g, '<br>');
}

function appendUnits(
	state: MarkdownSerializerState,
	block: PMNode,
	parent: PMNode | null,
	index: number,
	units: string[]
) {
	if (block.isTextblock) {
		units.push(capture(state, () => state.renderInline(block)));
	} else if (block.childCount) {
		block.forEach((child, _offset, i) => appendUnits(state, child, block, i, units));
	} else {
		units.push(capture(state, () => state.render(block, parent, index)));
	}
}

// Every block inside a cell degrades to its inline text, blocks joined with `<br>`;
// `|` is escaped because the cell delimiter is what prose has to avoid.
function cellText(state: MarkdownSerializerState, cell: PMNode): string {
	const units: string[] = [];
	cell.forEach((block, _offset, index) => appendUnits(state, block, cell, index, units));
	return units.filter(Boolean).join('<br>').replace(/\|/g, '\\|');
}

function serializeTable(state: MarkdownSerializerState, node: PMNode): void {
	const inTable = state.inTable;
	state.inTable = true;
	// Flush the pending block separator and line prefix before cells are rendered in isolation.
	state.write('');

	// Positions a colspan/rowspan covers, or a ragged row lacks, hold no cell of their
	// own: they become empty cells so every row keeps the column count. TableMap marks
	// a missing slot with 0, which is the first row's position, never a cell's.
	const map = TableMap.get(node);
	const alignments: (string | null)[] = Array.from({ length: map.width }, () => null);
	const rows: string[][] = [];
	for (let row = 0; row < map.height; row++) {
		const cells: string[] = [];
		for (let col = 0; col < map.width; col++) {
			const pos = map.map[row * map.width + col];
			const cell = pos ? node.nodeAt(pos) : null;
			const rect = cell && map.findCell(pos);
			if (!cell || !rect || rect.left !== col || rect.top !== row) {
				cells.push('');
				continue;
			}
			cells.push(cellText(state, cell));
			const align = cell.attrs.align as string | null;
			if (!alignments[col] && align && ALIGN_DELIMITERS[align]) alignments[col] = align;
		}
		rows.push(cells);
	}
	state.inTable = inTable;

	if (!map.width) {
		state.closeBlock(node);
		return;
	}

	const delimiterRow = alignments.map((align) => (align && ALIGN_DELIMITERS[align]) || DELIMITER);
	rows.forEach((cells, index) => {
		state.write(`| ${cells.join(' | ')} |`);
		state.ensureNewLine();
		// The first row is the header row of the file, whatever the cells were parsed as.
		if (!index) {
			state.write(`| ${delimiterRow.join(' | ')} |`);
			state.ensureNewLine();
		}
	});
	state.closeBlock(node);
}

export const TableMarkdown = Table.extend({
	addStorage() {
		return {
			markdown: {
				serialize: serializeTable
			}
		};
	}
});
