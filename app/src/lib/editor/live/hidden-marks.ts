import { syntaxTree } from '@codemirror/language';
import type { EditorState } from '@codemirror/state';
import { near, revealPoints } from './reveal';
import { HIGHLIGHT, HIGHLIGHT_MARK, WIKI_LINK, WIKI_LINK_MARK, WIKI_LINK_TARGET } from './syntax';

export interface HiddenRun {
	from: number;
	to: number;
	opens: boolean;
}

const OWNERS: Record<string, readonly string[]> = {
	EmphasisMark: ['Emphasis', 'StrongEmphasis'],
	StrikethroughMark: ['Strikethrough'],
	[HIGHLIGHT_MARK]: [HIGHLIGHT],
	CodeMark: ['InlineCode'],
	LinkMark: ['Link', 'Autolink'],
	URL: ['Link'],
	LinkTitle: ['Link'],
	[WIKI_LINK_MARK]: [WIKI_LINK],
	[WIKI_LINK_TARGET]: [WIKI_LINK]
};

export function hiddenRuns(state: EditorState, from: number, to: number): HiddenRun[] {
	const points = revealPoints(state);
	const runs: HiddenRun[] = [];
	syntaxTree(state).iterate({
		from: Math.max(0, from - 1),
		to: Math.min(state.doc.length, to + 1),
		enter(ref) {
			const owners = OWNERS[ref.name];
			const parent = owners ? ref.node.parent : null;
			if (!parent || !owners?.includes(parent.name) || near(points, parent.from, parent.to)) return;
			if (parent.name === 'Link' && !parent.getChild('URL')) return;
			if (ref.name === WIKI_LINK_TARGET && parent.getChildren(WIKI_LINK_MARK).length < 3) return;
			const last = runs.at(-1);
			if (last && last.to === ref.from) last.to = ref.to;
			else runs.push({ from: ref.from, to: ref.to, opens: ref.from === parent.from });
		}
	});
	return runs.filter((run) => run.to >= from && run.from <= to);
}

export function coveredByHidden(state: EditorState, from: number, to: number): boolean {
	let at = from;
	for (const run of hiddenRuns(state, from, to)) {
		if (run.from > at) return false;
		at = Math.max(at, run.to);
		if (at >= to) return true;
	}
	return at >= to;
}

export function outsideHidden(state: EditorState, pos: number): number {
	const run = hiddenRuns(state, pos, pos).find((each) => each.from <= pos && pos <= each.to);
	if (!run) return pos;
	return run.opens ? run.from : run.to;
}
