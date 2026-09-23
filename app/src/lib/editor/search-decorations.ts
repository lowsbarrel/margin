import type { Node as PMNode } from '@tiptap/pm/model';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { SearchMatch } from '$lib/editor/search-text';

export function buildBaseDecos(doc: PMNode, matches: SearchMatch[]): DecorationSet {
	if (matches.length === 0) return DecorationSet.empty;
	return DecorationSet.create(
		doc,
		matches.map((match) => Decoration.inline(match.from, match.to, { class: 'search-match' }))
	);
}

export function buildCurrentDeco(
	doc: PMNode,
	matches: SearchMatch[],
	currentIndex: number
): DecorationSet {
	if (matches.length === 0 || currentIndex >= matches.length) return DecorationSet.empty;
	const match = matches[currentIndex];
	return DecorationSet.create(doc, [
		Decoration.inline(match.from, match.to, { class: 'search-match-current' })
	]);
}
