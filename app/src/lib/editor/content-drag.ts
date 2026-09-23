// WKWebView never fires the DOM `drop` event for internal contenteditable drags, so move/copy is reimplemented on plain mouse events.
import { Extension } from '@tiptap/core';
import { NodeSelection, Plugin, TextSelection } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { EditorView } from '@tiptap/pm/view';

import { createBlockHandlePlugin } from './content-block-handle';
import { contentDragKey, startDrag, type DragState } from './content-drag-move';

function createContentDragPlugin(): Plugin<DragState> {
	return new Plugin<DragState>({
		key: contentDragKey,

		state: {
			init(): DragState {
				return { dropPos: null };
			},
			apply(tr, prev): DragState {
				const meta = tr.getMeta(contentDragKey);
				if (meta !== undefined) return meta as DragState;
				if (tr.docChanged && prev.dropPos != null) {
					return { dropPos: tr.mapping.map(prev.dropPos) };
				}
				return prev;
			}
		},

		props: {
			decorations(state) {
				const ps = contentDragKey.getState(state) as DragState | undefined;
				if (!ps || ps.dropPos == null) return DecorationSet.empty;
				return DecorationSet.create(state.doc, [
					Decoration.widget(
						ps.dropPos,
						() => {
							const bar = document.createElement('span');
							bar.className = 'content-drop-cursor';
							return bar;
						},
						{ side: -1 }
					)
				]);
			},

			handleDOMEvents: {
				mousedown(view: EditorView, event: MouseEvent) {
					if (event.button !== 0) return false;
					if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return false;

					const { state } = view;

					const target = event.target as HTMLElement;
					const nodeEl =
						target.closest('img') ||
						target.closest('.file-embed') ||
						target.closest('[data-type="mathBlock"]') ||
						target.closest('[data-type="callout"]');

					if (nodeEl && view.dom.contains(nodeEl)) {
						const nodePos = view.posAtDOM(nodeEl, 0);
						const resolved = state.doc.resolve(nodePos);
						let from = nodePos;
						let to = nodePos;

						const after = resolved.nodeAfter;
						if (after && (after.type.spec.draggable || after.type.spec.atom)) {
							to = nodePos + after.nodeSize;
						} else {
							for (let d = resolved.depth; d >= 1; d--) {
								const n = resolved.node(d);
								if (n.type.spec.draggable) {
									from = resolved.before(d);
									to = resolved.after(d);
									break;
								}
							}
						}

						if (from === to) return false;

						try {
							const tr = state.tr.setSelection(NodeSelection.create(state.doc, from));
							view.dispatch(tr);
						} catch {}

						startDrag(view, event, from, to, true);
						event.preventDefault();
						return true;
					}

					if (state.selection.empty) return false;
					if (!(state.selection instanceof TextSelection)) return false;

					const pos = view.posAtCoords({
						left: event.clientX,
						top: event.clientY
					});
					if (!pos) return false;

					const { from, to } = state.selection;
					if (pos.pos < from || pos.pos > to) return false;

					startDrag(view, event, from, to, false);
					event.preventDefault();
					return true;
				}
			}
		}
	});
}

export const ContentDrag = Extension.create({
	name: 'contentDrag',

	addProseMirrorPlugins() {
		return [createContentDragPlugin(), createBlockHandlePlugin()];
	}
});
