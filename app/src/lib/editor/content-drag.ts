import { Extension } from "@tiptap/core";
import {
  Plugin,
  PluginKey,
  TextSelection,
  NodeSelection,
} from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import { dropPoint } from "@tiptap/pm/transform";
import type { EditorView } from "@tiptap/pm/view";

/**
 * Mouse-based drag-to-move for editor content.
 *
 * WKWebView (Tauri on macOS) never fires the DOM `drop` event for
 * internal text drags in contenteditable, so native DnD is unusable.
 * This plugin intercepts mousedown inside an existing selection and
 * implements move/copy via plain mouse events.
 */

const dragKey = new PluginKey("contentDrag");

const THRESHOLD = 5; // px before a click becomes a drag

interface DragState {
  /** Document position the drop indicator is at, or null */
  dropPos: number | null;
}

export const ContentDrag = Extension.create({
  name: "contentDrag",

  addProseMirrorPlugins() {
    return [
      new Plugin<DragState>({
        key: dragKey,

        state: {
          init(): DragState {
            return { dropPos: null };
          },
          apply(tr, prev): DragState {
            const meta = tr.getMeta(dragKey);
            if (meta !== undefined) return meta as DragState;
            if (tr.docChanged && prev.dropPos != null) {
              return { dropPos: tr.mapping.map(prev.dropPos) };
            }
            return prev;
          },
        },

        props: {
          decorations(state) {
            const ps = dragKey.getState(state) as DragState | undefined;
            if (!ps || ps.dropPos == null) return DecorationSet.empty;
            return DecorationSet.create(state.doc, [
              Decoration.widget(
                ps.dropPos,
                () => {
                  const bar = document.createElement("span");
                  bar.className = "content-drop-cursor";
                  return bar;
                },
                { side: -1 },
              ),
            ]);
          },

          handleDOMEvents: {
            mousedown(view: EditorView, event: MouseEvent) {
              if (event.button !== 0) return false;
              if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey)
                return false;

              const { state } = view;

              // ── Node drag: click on a draggable atom node ──
              const target = event.target as HTMLElement;
              const nodeEl =
                target.closest("img") ||
                target.closest(".file-embed") ||
                target.closest('[data-type="mathBlock"]') ||
                target.closest('[data-type="callout"]');

              if (nodeEl && view.dom.contains(nodeEl)) {
                const nodePos = view.posAtDOM(nodeEl, 0);
                const resolved = state.doc.resolve(nodePos);
                let from = nodePos;
                let to = nodePos;

                // Find the draggable node range
                const after = resolved.nodeAfter;
                if (after && (after.type.spec.draggable || after.type.spec.atom)) {
                  to = nodePos + after.nodeSize;
                } else {
                  // walk up
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

                // Select the node
                try {
                  const tr = state.tr.setSelection(
                    NodeSelection.create(state.doc, from),
                  );
                  view.dispatch(tr);
                } catch {
                  /* not selectable as NodeSelection */
                }

                startDrag(view, event, from, to, true);
                event.preventDefault();
                return true;
              }

              // ── Text drag: click inside an existing non-empty selection ──
              if (state.selection.empty) return false;
              if (!(state.selection instanceof TextSelection)) return false;

              const pos = view.posAtCoords({
                left: event.clientX,
                top: event.clientY,
              });
              if (!pos) return false;

              const { from, to } = state.selection;
              if (pos.pos < from || pos.pos > to) return false;

              // This click is inside the selection — start potential drag
              startDrag(view, event, from, to, false);
              event.preventDefault();
              return true;
            },
          },
        },
      }),
    ];

    function startDrag(
      view: EditorView,
      initEvent: MouseEvent,
      srcFrom: number,
      srcTo: number,
      isNode: boolean,
    ) {
      const startX = initEvent.clientX;
      const startY = initEvent.clientY;
      let dragging = false;
      let lastDropPos: number | null = null;

      const slice = view.state.doc.slice(srcFrom, srcTo);
      let rafScheduled = false;
      let latestMoveEvent: MouseEvent | null = null;

      const flushMove = () => {
        rafScheduled = false;
        const e = latestMoveEvent;
        if (!e) return;

        const dx = e.clientX - startX;
        const dy = e.clientY - startY;

        if (!dragging && Math.abs(dx) + Math.abs(dy) < THRESHOLD) return;

        if (!dragging) {
          dragging = true;
          document.body.classList.add("content-dragging");
        }

        // Compute drop position
        const result = view.posAtCoords({
          left: e.clientX,
          top: e.clientY,
        });
        if (result) {
          let dp = result.pos;
          // For nodes, find a valid block drop point
          if (isNode) {
            const best = dropPoint(view.state.doc, dp, slice);
            if (best != null) dp = best;
          }
          dp = Math.max(0, Math.min(dp, view.state.doc.content.size));
          lastDropPos = dp;
        }

        // Update drop cursor decoration
        const tr = view.state.tr;
        tr.setMeta(dragKey, { dropPos: lastDropPos } satisfies DragState);
        view.dispatch(tr);
      };

      const onMove = (e: MouseEvent) => {
        latestMoveEvent = e;
        if (!rafScheduled) {
          rafScheduled = true;
          requestAnimationFrame(flushMove);
        }
      };

      const onUp = (e: MouseEvent) => {
        window.removeEventListener("mousemove", onMove, true);
        window.removeEventListener("mouseup", onUp, true);
        document.body.classList.remove("content-dragging");

        // Clear drop cursor
        const clearTr = view.state.tr;
        clearTr.setMeta(dragKey, { dropPos: null } satisfies DragState);

        if (!dragging) {
          // Was a click, not a drag — collapse selection to click point
          if (!isNode) {
            const clickPos = view.posAtCoords({
              left: e.clientX,
              top: e.clientY,
            });
            if (clickPos) {
              try {
                clearTr.setSelection(
                  TextSelection.create(clearTr.doc, clickPos.pos),
                );
              } catch {
                /* ignore */
              }
            }
          }
          view.dispatch(clearTr);
          view.focus();
          return;
        }

        if (lastDropPos == null) {
          view.dispatch(clearTr);
          return;
        }

        // Don't drop inside the source range
        if (lastDropPos >= srcFrom && lastDropPos <= srcTo) {
          view.dispatch(clearTr);
          return;
        }

        // ── Execute the move ──
        const tr = view.state.tr;
        tr.setMeta(dragKey, { dropPos: null } satisfies DragState);

        let insertAt = lastDropPos;

        if (insertAt <= srcFrom) {
          // Dropping before source: insert at target, then delete shifted source
          tr.insert(insertAt, slice.content);
          const shift = slice.content.size;
          tr.delete(srcFrom + shift, srcTo + shift);
        } else if (insertAt >= srcTo) {
          // Dropping after source: delete source first, then insert at adjusted pos
          tr.delete(srcFrom, srcTo);
          insertAt -= srcTo - srcFrom;
          const safePos = Math.min(insertAt, tr.doc.content.size);
          tr.insert(safePos, slice.content);
        } else {
          // Inside source range — shouldn't reach here after the check above
          view.dispatch(clearTr);
          return;
        }

        view.dispatch(tr.setMeta("uiEvent", "drop"));
        view.focus();
      };

      window.addEventListener("mousemove", onMove, true);
      window.addEventListener("mouseup", onUp, true);
    }
  },
});
