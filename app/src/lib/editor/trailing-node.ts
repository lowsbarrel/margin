import { Extension } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';

export const TrailingNode = Extension.create({
	name: 'trailingNode',

	addProseMirrorPlugins() {
		return [
			new Plugin({
				key: new PluginKey(this.name),
				appendTransaction(_transactions, _oldState, newState) {
					const { doc, tr, schema } = newState;
					const lastNode = doc.lastChild;

					if (!lastNode || lastNode.type === schema.nodes.paragraph) {
						return null;
					}

					if (lastNode.type === schema.nodes.heading) {
						return null;
					}

					return tr.insert(doc.content.size, schema.nodes.paragraph.create());
				}
			})
		];
	}
});
