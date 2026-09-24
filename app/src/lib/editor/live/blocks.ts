import type { EditorState, Range } from '@codemirror/state';
import { StateField } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';
import { buildLocalfileUrl } from '$lib/editor/image-url';
import { assetsOf } from './assets';
import { refreshDecorations } from './decorate';
import { ImageWidget, RuleWidget } from './widgets';

// CodeMirror only accepts block widgets from a state field, never from a view plugin.
function blockDecorations(state: EditorState): DecorationSet {
	const ranges: Range<Decoration>[] = [];
	for (const asset of assetsOf(state)) {
		const widget =
			asset.kind === 'rule'
				? new RuleWidget()
				: new ImageWidget(buildLocalfileUrl(asset.src ?? ''), asset.alt);
		ranges.push(Decoration.widget({ block: true, widget, side: 1 }).range(asset.line.to));
	}
	return Decoration.set(ranges, true);
}

export const blockWidgets = StateField.define<DecorationSet>({
	create(state) {
		return blockDecorations(state);
	},
	update(value, tr) {
		if (tr.docChanged || tr.effects.some((effect) => effect.is(refreshDecorations))) {
			return blockDecorations(tr.state);
		}
		return value.map(tr.changes);
	},
	provide: (field) => EditorView.decorations.from(field)
});
