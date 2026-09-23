import { Table } from '@tiptap/extension-table';
import type { Node as PMNode } from '@tiptap/pm/model';
import { TableMap } from '@tiptap/pm/tables';

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

const ALIGN_DELIMITERS: Record<string, string> = {
	left: ':---',
	center: ':---:',
	right: '---:'
};

const DELIMITER = '---';

function renderToText(state: MarkdownSerializerState, render: () => void): string {
	const start = state.out.length;
	render();
	const text = state.out.slice(start);
	state.out = state.out.slice(0, start);
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
		units.push(renderToText(state, () => state.renderInline(block)));
	} else if (block.childCount) {
		block.forEach((child, _offset, i) => appendUnits(state, child, block, i, units));
	} else {
		units.push(renderToText(state, () => state.render(block, parent, index)));
	}
}

function cellText(state: MarkdownSerializerState, cell: PMNode): string {
	const units: string[] = [];
	cell.forEach((block, _offset, index) => appendUnits(state, block, cell, index, units));
	return units.filter(Boolean).join('<br>').replace(/\|/g, '\\|');
}

function serializeTable(state: MarkdownSerializerState, node: PMNode): void {
	const inTable = state.inTable;
	state.inTable = true;
	state.write('');

	const map = TableMap.get(node);
	const alignments: (string | null)[] = Array.from({ length: map.width }, () => null);
	const rows: string[][] = [];
	for (let row = 0; row < map.height; row++) {
		const cells: string[] = [];
		for (let col = 0; col < map.width; col++) {
			// TableMap marks a slot no cell spans with 0, which is the first row's position, never a cell's.
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
	const [header, ...body] = rows;

	function writeRow(cells: string[]) {
		state.write(`| ${cells.join(' | ')} |`);
		state.ensureNewLine();
	}

	writeRow(header);
	writeRow(delimiterRow);
	body.forEach(writeRow);
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
