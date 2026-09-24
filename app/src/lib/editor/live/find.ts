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
import { Compartment, StateEffect, type EditorState, type Extension } from '@codemirror/state';
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

function matchRanges(state: EditorState): { from: number; to: number }[] {
	const query = getSearchQuery(state);
	if (!query.valid) return [];
	const ranges: { from: number; to: number }[] = [];
	const cursor = query.getCursor(state);
	for (let match = cursor.next(); !match.done && ranges.length < MATCH_LIMIT; match = cursor.next())
		ranges.push(match.value);
	return ranges;
}

export function findState(view: EditorView): FindStats {
	const selection = view.state.selection.main;
	const ranges = matchRanges(view.state);
	let current = 0;
	let next = 0;
	for (let i = 0; i < ranges.length; i++) {
		if (ranges[i].from === selection.from && ranges[i].to === selection.to) current = i + 1;
		else if (!next && ranges[i].from >= selection.head) next = i + 1;
	}
	return { total: ranges.length, index: ranges.length ? current || next || 1 : 0 };
}

export { findNext as findNextMatch, findPrevious as findPreviousMatch } from '@codemirror/search';
export { replaceAll as replaceEveryMatch } from '@codemirror/search';

export function replaceCurrentMatch(view: EditorView): void {
	const selection = view.state.selection.main;
	const onMatch = matchRanges(view.state).some(
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
