import type { Node as PMNode } from '@tiptap/pm/model';

export interface SearchMatch {
	from: number;
	to: number;
}

export interface FlattenedDoc {
	text: string;
	lowerText: string;
	pos: number[];
	gaps: number[];
}

let cachedDoc: PMNode | null = null;
let cached: FlattenedDoc | null = null;

export function flattenDoc(doc: PMNode): FlattenedDoc {
	if (doc === cachedDoc && cached) return cached;

	const chars: string[] = [];
	const pos: number[] = [];
	const gaps: number[] = [];
	doc.descendants((node: PMNode, at: number) => {
		if (!node.isText) return;
		const text = node.text!;
		for (let i = 0; i < text.length; i++) {
			const charPos = at + i;
			if (pos.length > 0 && charPos !== pos[pos.length - 1] + 1) gaps.push(chars.length);
			chars.push(text[i]);
			pos.push(charPos);
		}
	});

	const text = chars.join('');
	cached = { text, lowerText: text.toLowerCase(), pos, gaps };
	cachedDoc = doc;
	return cached;
}

export function findMatchesSync(
	doc: PMNode,
	searchTerm: string,
	caseSensitive: boolean
): SearchMatch[] {
	if (!searchTerm) return [];

	const flat = flattenDoc(doc);
	const matches: SearchMatch[] = [];
	const term = caseSensitive ? searchTerm : searchTerm.toLowerCase();
	const fullText = caseSensitive ? flat.text : flat.lowerText;
	const { pos } = flat;

	let start = 0;
	while (start <= fullText.length - term.length) {
		const idx = fullText.indexOf(term, start);
		if (idx === -1) break;

		let crossBlock = false;
		for (let k = idx + 1; k < idx + term.length; k++) {
			if (pos[k] !== pos[k - 1] + 1) {
				crossBlock = true;
				break;
			}
		}

		if (!crossBlock) {
			matches.push({ from: pos[idx], to: pos[idx + term.length - 1] + 1 });
		}

		start = idx + 1;
	}

	return matches;
}
