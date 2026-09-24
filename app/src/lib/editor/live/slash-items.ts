import type { ChangeSpec, EditorState } from '@codemirror/state';
import { getLocale } from '$lib/paraglide/runtime.js';
import * as m from '$lib/paraglide/messages.js';

export interface SlashEdit {
	changes: ChangeSpec[];
	selection: number;
}

export interface SlashItem {
	id: string;
	title: string;
	description: string;
	icon: string;
	terms: string[];
	edit(state: EditorState, from: number, to: number): SlashEdit;
}

const BLOCK_PREFIX =
	/^(?:#{1,6}[ \t]+|[-*+][ \t]+\[[ xX]\][ \t]+|[-*+][ \t]+|\d+[.)][ \t]+|>[ \t]?)+/;
const TABLE = '|   |   |   |\n| --- | --- | --- |\n|   |   |   |';

function markerEdit(state: EditorState, from: number, to: number, marker: string): SlashEdit {
	const line = state.doc.lineAt(from);
	const indent = /^[ \t]*/.exec(state.sliceDoc(line.from, from))?.[0] ?? '';
	const head = state.sliceDoc(line.from + indent.length, from);
	const tail = state.sliceDoc(to, line.to);
	const joined = head ? `${head.trimEnd()} ${tail.trimStart()}` : tail.trimStart();
	const body = joined.replace(BLOCK_PREFIX, '').trimEnd();
	return {
		changes: [{ from: line.from, to: line.to, insert: indent + marker + body }],
		selection: line.from + indent.length + marker.length
	};
}

function blockEdit(
	state: EditorState,
	from: number,
	to: number,
	block: string,
	caret: number
): SlashEdit {
	const line = state.doc.lineAt(from);
	const head = state.sliceDoc(line.from, from).replace(BLOCK_PREFIX, '').trim();
	const tail = state.sliceDoc(to, line.to).trim();
	const prefix = head ? `${head}\n` : '';
	const suffix = tail ? `\n${tail}` : '';
	return {
		changes: [{ from: line.from, to: line.to, insert: prefix + block + suffix }],
		selection: line.from + prefix.length + caret
	};
}

function inlineEdit(
	state: EditorState,
	from: number,
	to: number,
	text: string,
	caret: number
): SlashEdit {
	return { changes: [{ from, to, insert: text }], selection: from + caret };
}

function today(): string {
	return new Date().toLocaleDateString(getLocale(), {
		year: 'numeric',
		month: 'long',
		day: 'numeric'
	});
}

// Built per call: Paraglide resolves the labels against the locale current at call time.
export function buildSlashItems(): SlashItem[] {
	return [
		{
			id: 'h1',
			title: m.slash_heading1(),
			description: m.slash_heading1_description(),
			icon: 'H1',
			terms: ['title', 'big', 'large', 'h1'],
			edit: (state, from, to) => markerEdit(state, from, to, '# ')
		},
		{
			id: 'h2',
			title: m.slash_heading2(),
			description: m.slash_heading2_description(),
			icon: 'H2',
			terms: ['subtitle', 'medium', 'h2'],
			edit: (state, from, to) => markerEdit(state, from, to, '## ')
		},
		{
			id: 'h3',
			title: m.slash_heading3(),
			description: m.slash_heading3_description(),
			icon: 'H3',
			terms: ['subtitle', 'small', 'h3'],
			edit: (state, from, to) => markerEdit(state, from, to, '### ')
		},
		{
			id: 'bullet',
			title: m.slash_bullet(),
			description: m.slash_bullet_description(),
			icon: '•',
			terms: ['unordered', 'point', 'list', 'ul'],
			edit: (state, from, to) => markerEdit(state, from, to, '- ')
		},
		{
			id: 'numbered',
			title: m.slash_numbered(),
			description: m.slash_numbered_description(),
			icon: '1.',
			terms: ['numbered', 'ordered', 'list', 'ol'],
			edit: (state, from, to) => markerEdit(state, from, to, '1. ')
		},
		{
			id: 'todo',
			title: m.slash_todo(),
			description: m.slash_todo_description(),
			icon: '☑',
			terms: ['todo', 'task', 'list', 'check', 'checkbox'],
			edit: (state, from, to) => markerEdit(state, from, to, '- [ ] ')
		},
		{
			id: 'quote',
			title: m.slash_quote(),
			description: m.slash_quote_description(),
			icon: '"',
			terms: ['blockquote', 'quotes'],
			edit: (state, from, to) => markerEdit(state, from, to, '> ')
		},
		{
			id: 'code',
			title: m.slash_code(),
			description: m.slash_code_description(),
			icon: '</>',
			terms: ['codeblock', 'code', 'snippet', 'fence'],
			edit: (state, from, to) => blockEdit(state, from, to, '```\n\n```', 4)
		},
		{
			id: 'table',
			title: m.slash_table(),
			description: m.slash_table_description(),
			icon: '▦',
			terms: ['table', 'rows', 'columns', 'grid'],
			edit: (state, from, to) => blockEdit(state, from, to, TABLE, 2)
		},
		{
			id: 'callout',
			title: m.slash_callout(),
			description: m.slash_callout_description(),
			icon: '💡',
			terms: ['callout', 'admonition', 'alert', 'info', 'warning', 'note'],
			edit: (state, from, to) => blockEdit(state, from, to, '> [!note]\n> ', 12)
		},
		{
			id: 'math',
			title: m.slash_math_block(),
			description: m.slash_math_block_description(),
			icon: '∑',
			terms: ['math', 'equation', 'formula', 'latex', 'katex', 'block'],
			edit: (state, from, to) => blockEdit(state, from, to, '$$\n\n$$', 3)
		},
		{
			id: 'mermaid',
			title: m.slash_mermaid(),
			description: m.slash_mermaid_description(),
			icon: '🧜',
			terms: ['mermaid', 'diagram', 'flowchart', 'graph', 'sequence', 'chart'],
			edit: (state, from, to) => blockEdit(state, from, to, '```mermaid\n\n```', 11)
		},
		{
			id: 'divider',
			title: m.slash_divider(),
			description: m.slash_divider_description(),
			icon: '—',
			terms: ['horizontal rule', 'hr', 'divider', 'separator'],
			edit: (state, from, to) => blockEdit(state, from, to, '---', 3)
		},
		{
			id: 'link',
			title: m.slash_link(),
			description: m.slash_link_description(),
			icon: '🔗',
			terms: ['link', 'url', 'href'],
			edit: (state, from, to) => inlineEdit(state, from, to, '[]()', 1)
		},
		{
			id: 'embed',
			title: m.slash_embed_note(),
			description: m.slash_embed_note_description(),
			icon: '📄',
			terms: ['embed', 'transclude', 'include', 'note', 'reference'],
			edit: (state, from, to) => inlineEdit(state, from, to, '![[]]', 3)
		},
		{
			id: 'date',
			title: m.slash_date(),
			description: m.slash_date_description(),
			icon: '📅',
			terms: ['date', 'today', 'time'],
			edit: (state, from, to) => inlineEdit(state, from, to, today(), today().length)
		}
	];
}

export function fuzzyMatch(
	needle: string,
	haystack: string
): { score: number; ranges: number[] } | null {
	const hay = haystack.toLowerCase();
	const ranges: number[] = [];
	let cursor = -1;
	let score = 0;
	for (const char of needle) {
		const found = hay.indexOf(char, cursor + 1);
		if (found < 0) return null;
		if (found === cursor + 1) score += 4;
		if (found === 0 || hay[found - 1] === ' ' || hay[found - 1] === '-') score += 2;
		score += 1;
		ranges.push(found, found + 1);
		cursor = found;
	}
	return { score: score - hay.length * 0.01, ranges };
}

function itemScore(item: SlashItem, needle: string): { score: number; ranges: number[] } | null {
	let best: { score: number; ranges: number[] } | null = fuzzyMatch(needle, item.title);
	for (const term of item.terms) {
		const match = fuzzyMatch(needle, term);
		if (!match) continue;
		if (!best || match.score > best.score) best = { score: match.score, ranges: match.ranges };
	}
	return best;
}

export function filterSlashItems(query: string): SlashItem[] {
	const needle = query.trim().toLowerCase();
	const items = buildSlashItems();
	if (!needle) return items;
	return items
		.map((item) => ({ item, match: itemScore(item, needle) }))
		.filter((entry) => entry.match !== null)
		.sort((a, b) => (b.match?.score ?? 0) - (a.match?.score ?? 0))
		.map((entry) => entry.item);
}
