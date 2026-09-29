import {
	closeSearchPanel,
	findNext,
	getSearchQuery,
	openSearchPanel,
	replaceNext,
	search,
	SearchQuery,
	setSearchQuery
} from '@codemirror/search';
import { Compartment, StateEffect, type Extension, type Text } from '@codemirror/state';
import { EditorView, type Panel } from '@codemirror/view';

const MATCH_LIMIT = 5000;

export interface FindStats {
	total: number;
	index: number;
}

export interface FindQueryInput {
	search: string;
	replace: string;
	caseSensitive: boolean;
}

// The Svelte panel owns the UI, so CodeMirror's own panel stays mounted but hidden to keep the search state (and its match highlighting) alive.
function hiddenPanel(): Panel {
	const dom = document.createElement('div');
	dom.style.display = 'none';
	return { dom };
}

const findTheme = EditorView.theme({
	'.cm-searchMatch': { backgroundColor: 'var(--color-highlight-yellow)', borderRadius: '2px' },
	'.cm-searchMatch-selected': {
		backgroundColor: 'var(--color-brand-32)',
		boxShadow: 'inset 0 0 0 1px var(--color-text-brand)'
	}
});

export const liveFind: Extension = [search({ createPanel: hiddenPanel }), findTheme];

function dispatchQuery(view: EditorView, input: FindQueryInput): void {
	view.dispatch({
		effects: setSearchQuery.of(
			new SearchQuery({
				search: input.search,
				replace: input.replace,
				caseSensitive: input.caseSensitive,
				literal: true
			})
		)
	});
}

export function openFind(view: EditorView): void {
	openSearchPanel(view);
	dispatchQuery(view, { search: '', replace: '', caseSensitive: false });
}

export function closeFind(view: EditorView): void {
	closeSearchPanel(view);
}

export function setFindQuery(view: EditorView, input: FindQueryInput): void {
	const current = getSearchQuery(view.state);
	if (
		current.search === input.search &&
		current.replace === input.replace &&
		current.caseSensitive === input.caseSensitive
	)
		return;
	dispatchQuery(view, input);
}

interface MatchRange {
	from: number;
	to: number;
}

interface FindCache {
	doc: Text | null;
	query: SearchQuery | null;
	ranges: MatchRange[];
}

const caches = new WeakMap<EditorView, FindCache>();

function sameQuery(a: SearchQuery, b: SearchQuery | null): boolean {
	return (
		!!b &&
		a.search === b.search &&
		a.replace === b.replace &&
		a.caseSensitive === b.caseSensitive &&
		a.literal === b.literal
	);
}

function matchRanges(view: EditorView): MatchRange[] {
	const state = view.state;
	const query = getSearchQuery(state);
	let cache = caches.get(view);
	if (!cache) {
		cache = { doc: null, query: null, ranges: [] };
		caches.set(view, cache);
	}
	if (cache.doc === state.doc && sameQuery(query, cache.query)) return cache.ranges;

	const ranges: MatchRange[] = [];
	if (query.valid) {
		const cursor = query.getCursor(state);
		for (
			let match = cursor.next();
			!match.done && ranges.length < MATCH_LIMIT;
			match = cursor.next()
		)
			ranges.push(match.value);
	}
	cache.doc = state.doc;
	cache.query = query;
	cache.ranges = ranges;
	return ranges;
}

function firstAtOrAfter(ranges: MatchRange[], pos: number): number {
	let low = 0;
	let high = ranges.length;
	while (low < high) {
		const mid = (low + high) >> 1;
		if (ranges[mid].from < pos) low = mid + 1;
		else high = mid;
	}
	return low;
}

export function findState(view: EditorView): FindStats {
	const ranges = matchRanges(view);
	if (ranges.length === 0) return { total: 0, index: 0 };
	const selection = view.state.selection.main;
	const selectedAt = firstAtOrAfter(ranges, selection.from);
	const selected = ranges[selectedAt];
	const current =
		selected && selected.from === selection.from && selected.to === selection.to
			? selectedAt + 1
			: 0;
	const at = firstAtOrAfter(ranges, selection.head);
	const next = at < ranges.length ? at + 1 : 0;
	return { total: ranges.length, index: current || next || 1 };
}

export { findNext as findNextMatch, findPrevious as findPreviousMatch } from '@codemirror/search';
export { replaceAll as replaceEveryMatch } from '@codemirror/search';

export function replaceCurrentMatch(view: EditorView): void {
	const selection = view.state.selection.main;
	const onMatch = matchRanges(view).some(
		(range) => range.from === selection.from && range.to === selection.to
	);
	// CodeMirror's replaceNext only moves to the next match; a click on Replace has to replace the highlighted one.
	if (!onMatch) findNext(view);
	replaceNext(view);
}

export function watchFind(view: EditorView, onChange: () => void): () => void {
	const compartment = new Compartment();
	view.dispatch({
		effects: StateEffect.appendConfig.of(
			compartment.of(
				EditorView.updateListener.of((update) => {
					if (update.docChanged || update.selectionSet) onChange();
				})
			)
		)
	});
	return () => {
		try {
			view.dispatch({ effects: compartment.reconfigure([]) });
		} catch {}
	};
}
