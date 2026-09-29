import type { EditorState } from '@codemirror/state';
import { frontmatterEnd } from './decorate';

export type PropertyKind =
	| 'text'
	| 'list'
	| 'number'
	| 'checkbox'
	| 'date'
	| 'datetime'
	| 'tags'
	| 'aliases'
	| 'raw';

export type PropertyValue = string | number | boolean | string[] | null;

export interface Property {
	key: string;
	value: PropertyValue;
	raw: string | null;
	from: number;
	to: number;
}

export interface Frontmatter {
	end: number;
	close: number;
	properties: Property[];
}

const KEY_LINE =
	/^("(?:[^"\\]|\\.)*"|'(?:[^']|'')*'|[^\s#\-?:,[\]{}&*!|>'"%@`][^]*?|[-?:][^\s][^]*?):(?:[ \t]+(.*?))?[ \t]*$/;
const ITEM = /^[ \t]*-(?:[ \t]+(.*?))?[ \t]*$/;
const BLOCK_SCALAR = /^([|>])([-+]?)\d?[-+]?$/;
const DATE = /^\d{4}-[01]\d-[0-3]\d$/;
const DATETIME = /^\d{4}-[01]\d-[0-3]\dT[0-2]\d:[0-5]\d(?::[0-5]\d)?$/;
const PLAIN_UNSAFE =
	/^[\n\t ,[\]{}#&*!|>'"%@`]|^[?-]$|^[?-][ \t]|[\n:][ \t]|[ \t]\n|[\n\t ]#|[\n\t :]$/;
const NULL = /^(?:~|[Nn]ull|NULL)?$/;
const BOOL = /^(?:[Tt]rue|TRUE|[Ff]alse|FALSE)$/;
const NUMBER =
	/^(?:[-+]?[0-9]+|0o[0-7]+|0x[0-9a-fA-F]+|[-+]?\.(?:inf|Inf|INF)|\.nan|\.NaN|\.NAN|[-+]?(?:\.[0-9]+|[0-9]+(?:\.[0-9]*)?)(?:[eE][-+]?[0-9]+)?)$/;
const FIXED_KINDS: Record<string, PropertyKind> = {
	tags: 'tags',
	aliases: 'aliases',
	cssclasses: 'list'
};

function plainScalar(text: string): PropertyValue {
	if (NULL.test(text)) return null;
	if (BOOL.test(text)) return /^t/i.test(text);
	if (NUMBER.test(text)) {
		if (/inf/i.test(text)) return text.startsWith('-') ? -Infinity : Infinity;
		if (/nan/i.test(text)) return NaN;
		return /^0o/.test(text) ? parseInt(text.slice(2), 8) : Number(text.replace(/^\+/, ''));
	}
	return text;
}

function scalar(source: string): { value: PropertyValue } | null {
	const text = source.trim();
	if (text.startsWith('"')) {
		try {
			return { value: JSON.parse(text) as string };
		} catch {
			return null;
		}
	}
	if (text.startsWith("'")) {
		return /^'(?:[^']|'')*'$/.test(text) ? { value: text.slice(1, -1).replace(/''/g, "'") } : null;
	}
	const plain = text.replace(/[ \t]+#.*$/, '');
	if (/^[&*!{[|>@`%]/.test(plain)) return null;
	return { value: plainScalar(plain) };
}

function flowItems(text: string): string[] | null {
	if (!/^\[.*\]$/.test(text)) return null;
	const items: string[] = [];
	let current = '';
	let quote = '';
	for (const char of text.slice(1, -1)) {
		if (quote) {
			current += char;
			if (char === quote) quote = '';
		} else if (char === '"' || char === "'") {
			quote = char;
			current += char;
		} else if ('[]{}'.includes(char)) return null;
		else if (char === ',') {
			items.push(current);
			current = '';
		} else current += char;
	}
	if (quote) return null;
	items.push(current);
	const values: string[] = [];
	for (const item of items) {
		if (!item.trim()) continue;
		const parsed = scalar(item);
		if (!parsed || parsed.value === null) return null;
		values.push(String(parsed.value));
	}
	return values;
}

function blockScalar(header: string, lines: string[]): string | null {
	const match = BLOCK_SCALAR.exec(header);
	if (!match) return null;
	const indent = Math.min(...lines.filter((l) => l.trim()).map((l) => /^ */.exec(l)![0].length));
	let text = lines.map((line) => line.slice(Math.min(indent, line.length))).join('\n');
	if (match[1] === '>') text = text.replace(/([^\n])\n(?=[^\n])/g, '$1 ').replace(/\n(\n+)/g, '$1');
	if (match[2] !== '+') text = text.replace(/\n+$/, '');
	return match[2] || !text ? text : `${text}\n`;
}

function listValue(rest: string[]): string[] | undefined {
	const values: string[] = [];
	for (const line of rest) {
		const source = ITEM.exec(line)?.[1];
		if (source === undefined) {
			if (!ITEM.test(line)) return undefined;
			continue;
		}
		const parsed = scalar(source);
		if (!parsed || /^(?:[-?](?:\s|$)|[[{])/.test(source)) return undefined;
		if (!/^['"]/.test(source) && /:(?:\s|$)/.test(source)) return undefined;
		if (parsed.value !== null) values.push(String(parsed.value));
	}
	return values;
}

function valueOf(inline: string, rest: string[]): PropertyValue | undefined {
	if (!inline) return rest.length ? listValue(rest) : null;
	if (inline.startsWith('[')) return rest.length ? undefined : (flowItems(inline) ?? undefined);
	if (BLOCK_SCALAR.test(inline)) return blockScalar(inline, rest) ?? undefined;
	if (rest.length) {
		if (/^['"[{]/.test(inline)) return undefined;
		return [inline, ...rest].map((line) => line.trim()).join(' ');
	}
	return scalar(inline)?.value;
}

export function parseFrontmatter(state: EditorState): Frontmatter | null {
	const end = frontmatterEnd(state);
	if (!end) return null;
	const doc = state.doc;
	const closeLine = doc.lineAt(end);
	const properties: Property[] = [];
	let pending: { key: string; inline: string; from: number; to: number; rest: string[] } | null =
		null;
	let blanks = 0;
	const settle = () => {
		if (!pending) return;
		if (/^[|>]\d?\+/.test(pending.inline)) pending.rest.push(...Array<string>(blanks).fill(''));
		const value = valueOf(pending.inline, pending.rest);
		const raw = value === undefined ? doc.sliceString(pending.from, pending.to) : null;
		properties.push({
			key: pending.key,
			value: value ?? null,
			raw,
			from: pending.from,
			to: pending.to
		});
		pending = null;
	};
	for (let number = 2; number < closeLine.number; number++) {
		const line = doc.line(number);
		const text = line.text;
		if (pending && !text.trim()) {
			blanks++;
			continue;
		}
		if (pending && (/^[ \t]/.test(text) || /^-(?:[ \t]|$)/.test(text))) {
			pending.rest.push(...Array<string>(blanks).fill(''), text);
			pending.to = line.to;
			blanks = 0;
			continue;
		}
		settle();
		blanks = 0;
		if (!text.trim() || text.startsWith('#')) continue;
		const entry = KEY_LINE.exec(text);
		if (!entry) return null;
		const key = scalar(entry[1])?.value;
		const inline = entry[2]?.startsWith('#') ? '' : (entry[2] ?? '');
		pending = {
			key: key == null ? entry[1] : String(key),
			inline,
			from: line.from,
			to: line.to,
			rest: []
		};
	}
	settle();
	return { end, close: closeLine.from, properties };
}

export function kindOf(key: string, value: PropertyValue, raw: string | null): PropertyKind {
	if (raw !== null) return 'raw';
	const fixed = FIXED_KINDS[key.toLowerCase()];
	if (fixed) return fixed;
	if (value === null) return 'text';
	if (Array.isArray(value)) return 'list';
	if (typeof value === 'number') return 'number';
	if (typeof value === 'boolean') return 'checkbox';
	if (DATETIME.test(value)) return 'datetime';
	return DATE.test(value) ? 'date' : 'text';
}

export function renameSource(raw: string, key: string): string {
	const entry = KEY_LINE.exec(raw.split('\n')[0]);
	return entry ? `${serializeProperty(key, null)}${raw.slice(entry[1].length + 1)}` : raw;
}

export function isListKind(kind: PropertyKind): boolean {
	return kind === 'list' || kind === 'tags' || kind === 'aliases';
}

export function asList(value: PropertyValue): string[] {
	if (value === null || value === '') return [];
	return Array.isArray(value) ? value : [String(value)];
}

function quoted(text: string): string {
	return text.includes('"') && !text.includes("'")
		? `'${text.replace(/'/g, "''")}'`
		: JSON.stringify(text);
}

function yamlString(text: string, key: boolean): string {
	if (key && text.includes('\n')) return quoted(text);
	if (!text || PLAIN_UNSAFE.test(text) || NULL.test(text) || BOOL.test(text) || NUMBER.test(text))
		return quoted(text);
	return text;
}

function blockText(text: string, indent: string): string {
	const trailing = /\n*$/.exec(text)![0].length;
	const chomp = trailing === 0 ? '-' : trailing === 1 ? '' : '+';
	const body = text.slice(0, text.length - Math.min(trailing, 1));
	const lines = body.split('\n').map((line) => (line ? `${indent}  ${line}` : ''));
	return `|${/^[ \t]/.test(body) ? '2' : ''}${chomp}\n${lines.join('\n')}`;
}

function listItem(item: string): string {
	return `  - ${item.includes('\n') ? blockText(item, '  ') : yamlString(item, false)}`;
}

export function serializeProperty(key: string, value: PropertyValue): string {
	const name = yamlString(key, true);
	if (value === null || value === '' || (Array.isArray(value) && value.length === 0))
		return `${name}:`;
	if (Array.isArray(value)) return [`${name}:`, ...value.map(listItem)].join('\n');
	if (typeof value === 'string')
		return `${name}: ${value.includes('\n') ? blockText(value, '') : yamlString(value, false)}`;
	return `${name}: ${String(value)}`;
}
