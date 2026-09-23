import { Extension } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';

export const SelectionPreserve = Extension.create({
	name: 'selectionPreserve',

	addProseMirrorPlugins() {
		let isFocused = true;

		return [
			new Plugin({
				key: new PluginKey('selectionPreserve'),
				props: {
					decorations(state) {
						if (isFocused) return DecorationSet.empty;
						const { from, to } = state.selection;
						if (from === to) return DecorationSet.empty;
						return DecorationSet.create(state.doc, [
							Decoration.inline(from, to, { class: 'selection-preserved' })
						]);
					},
					handleDOMEvents: {
						focus: (view) => {
							isFocused = true;
							view.dispatch(view.state.tr.setMeta('selectionPreserve', true));
							return false;
						},
						blur: (view) => {
							isFocused = false;
							view.dispatch(view.state.tr.setMeta('selectionPreserve', true));
							return false;
						}
					}
				}
			})
		];
	}
});
