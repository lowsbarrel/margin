import type { EditorState, Range } from '@codemirror/state';
import {
	Decoration,
	ViewPlugin,
	type DecorationSet,
	type EditorView,
	type ViewUpdate
} from '@codemirror/view';
import { assetsOf } from './assets';
import { refreshDecorations, treeChanged } from './decorate';
import { ImageWidget } from './widgets';

function widgets(state: EditorState): DecorationSet {
	const ranges: Range<Decoration>[] = [];
	for (const asset of assetsOf(state)) {
		if (asset.kind !== 'image') continue;
		const widget = new ImageWidget(asset.url, asset.alt, asset.width, asset.height);
		ranges.push(Decoration.widget({ widget, side: 1 }).range(asset.to));
	}
	return Decoration.set(ranges, true);
}

export const cellWidgets = ViewPlugin.fromClass(
	class {
		decorations: DecorationSet;

		constructor(view: EditorView) {
			this.decorations = widgets(view.state);
		}

		update(update: ViewUpdate): void {
			const refresh = update.transactions.some((tr) =>
				tr.effects.some((effect) => effect.is(refreshDecorations))
			);
			if (update.docChanged || refresh || treeChanged(update))
				this.decorations = widgets(update.state);
		}
	},
	{ decorations: (plugin) => plugin.decorations }
);
