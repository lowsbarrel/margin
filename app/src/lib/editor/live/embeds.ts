import type { EditorState, Extension, Range } from '@codemirror/state';
import { StateField } from '@codemirror/state';
import {
	Decoration,
	EditorView,
	ViewPlugin,
	type DecorationSet,
	type ViewUpdate
} from '@codemirror/view';
import { contextOf, type LiveContext } from './context';
import { embedChain, embedDepth, embedTargetsOf, type CardExtensions } from './embed-targets';
import {
	cardExtensions,
	FileCardWidget,
	NoteEmbedCardWidget,
	NoteLinkWidget
} from './embed-widgets';
import { hide, line, refreshDecorations, touched, touchedLines } from './decorate';

function blockDecorations(state: EditorState, touchedSet: Set<number>): DecorationSet {
	const ctx = contextOf(state);
	const depth = state.facet(embedDepth);
	const chain = state.facet(embedChain);
	const build: CardExtensions = (child: LiveContext, at, seen) =>
		cardExtensions(child, at, seen, liveEmbeds);
	const ranges: Range<Decoration>[] = [];
	for (const target of embedTargetsOf(state, ctx)) {
		if (target.kind !== 'note' || !target.ownLine) continue;
		if (touched(state, touchedSet, target.from, target.to)) continue;
		ranges.push(
			Decoration.widget({
				block: true,
				widget: new NoteEmbedCardWidget(target, depth, chain, build),
				side: 1
			}).range(target.line.to)
		);
	}
	return Decoration.set(ranges, true);
}

const embedBlocks = StateField.define<DecorationSet>({
	create: (state) => blockDecorations(state, touchedLines(state)),
	update(value, tr) {
		if (
			tr.docChanged ||
			!tr.newSelection.eq(tr.startState.selection) ||
			tr.effects.some((e) => e.is(refreshDecorations))
		)
			return blockDecorations(tr.state, touchedLines(tr.state));
		return value.map(tr.changes);
	},
	provide: (field) => EditorView.decorations.from(field)
});

function buildEmbeds(state: EditorState): DecorationSet {
	const ctx = contextOf(state);
	const touchedSet = touchedLines(state);
	const ranges: Range<Decoration>[] = [];
	for (const target of embedTargetsOf(state, ctx)) {
		if (touched(state, touchedSet, target.from, target.to)) continue;
		if (target.kind === 'note' && target.ownLine) {
			ranges.push(hide(target.from, target.to));
			ranges.push(line(target.line.from, 'cm-lp-embed-line'));
			continue;
		}
		ranges.push(
			Decoration.replace({
				widget: target.kind === 'note' ? new NoteLinkWidget(target) : new FileCardWidget(target)
			}).range(target.from, target.to)
		);
	}
	return Decoration.set(ranges, true);
}

class LiveEmbeds {
	decorations: DecorationSet;

	constructor(view: EditorView) {
		this.decorations = buildEmbeds(view.state);
	}

	update(update: ViewUpdate) {
		if (
			!update.docChanged &&
			!update.selectionSet &&
			!update.viewportChanged &&
			!update.transactions.some((tr) => tr.effects.some((e) => e.is(refreshDecorations)))
		) {
			return;
		}
		this.decorations = buildEmbeds(update.state);
	}
}

export const liveEmbeds: Extension = [
	embedBlocks,
	ViewPlugin.fromClass(LiveEmbeds, { decorations: (plugin) => plugin.decorations })
];
