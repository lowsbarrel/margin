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
 * Mouse-based drag-to-move for editor content + block drag handle.
 *
 * WKWebView (Tauri on macOS) never fires the DOM `drop` event for
 * internal text drags in contenteditable, so native DnD is unusable.
 * This plugin intercepts mousedown inside an existing selection and
 * implements move/copy via plain mouse events.
 *
 * The block handle appears on hover at the left of every top-level
 * block, enabling whole-block selection and drag-to-move.
 */

const dragKey = new PluginKey("contentDrag");
const handleKey = new PluginKey("blockHandle");

const THRESHOLD = 5; // px before a click becomes a drag

const HANDLE_SVG = `<svg width="8" height="14" viewBox="0 0 8 14" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><circle cx="1.5" cy="2" r="1.2"/><circle cx="6.5" cy="2" r="1.2"/><circle cx="1.5" cy="7" r="1.2"/><circle cx="6.5" cy="7" r="1.2"/><circle cx="1.5" cy="12" r="1.2"/><circle cx="6.5" cy="12" r="1.2"/></svg>`;

interface DragState {
  /** Document position the drop indicator is at, or null */
  dropPos: number | null;
}

export const ContentDrag = Extension.create({
  name: "contentDrag",

  addProseMirrorPlugins() {
    /* ── Shared drag helper ────────────────────────── */

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

    /* ── Plugin 1: content drag (text + node) ──────── */

    const contentDragPlugin = new Plugin<DragState>({
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
            if (
              event.metaKey ||
              event.ctrlKey ||
              event.shiftKey ||
              event.altKey
            )
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
              if (
                after &&
                (after.type.spec.draggable || after.type.spec.atom)
              ) {
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
    });

    /* ── Plugin 2: block drag handle ───────────────── */

    const blockHandlePlugin = new Plugin({
      key: handleKey,

      view(editorView) {
        // Build the handle DOM element
        const handle = document.createElement("div");
        handle.className = "block-drag-handle";
        handle.contentEditable = "false";
        handle.setAttribute("draggable", "false");
        handle.innerHTML = HANDLE_SVG;

        const editorDom = editorView.dom;
        const wrap = editorDom.parentElement!;
        wrap.style.position = "relative";
        wrap.appendChild(handle);

        let activeNodePos: number | null = null;
        let activeNodeEnd: number = 0;
        let hoveredDom: HTMLElement | null = null;
        let isHandleHovered = false;

        /** Resolve the top-level block at the given viewport coordinates. */
        function findTopBlock(view: EditorView, x: number, y: number) {
          const coords = view.posAtCoords({ left: x, top: y });
          if (!coords) return null;

          const $pos = view.state.doc.resolve(coords.pos);
          if ($pos.depth === 0) {
            // Cursor is between blocks — pick the nearest node
            const idx = $pos.index(0);
            if (idx >= view.state.doc.childCount) return null;
            let pos = 0;
            for (let i = 0; i < idx; i++) pos += view.state.doc.child(i).nodeSize;
            const node = view.state.doc.child(idx);
            const dom = view.nodeDOM(pos);
            if (!dom || !(dom instanceof HTMLElement)) return null;
            return { pos, node, dom };
          }

          const topPos = $pos.before(1);
          const topNode = $pos.node(1);
          const dom = view.nodeDOM(topPos);
          if (!dom || !(dom instanceof HTMLElement)) return null;
          return { pos: topPos, node: topNode, dom };
        }

        function positionHandle(blockDom: HTMLElement) {
          const wrapRect = wrap.getBoundingClientRect();
          const blockRect = blockDom.getBoundingClientRect();

          handle.style.display = "flex";
          handle.style.top = `${blockRect.top - wrapRect.top}px`;
          handle.style.left = `${blockRect.left - wrapRect.left - 24}px`;
        }

        function hideHandle() {
          if (isHandleHovered) return;
          handle.style.display = "";
          activeNodePos = null;
          hoveredDom = null;
        }

        // ── Editor mouse tracking ──
        const onEditorMouseMove = (e: MouseEvent) => {
          if (document.body.classList.contains("content-dragging")) return;

          const result = findTopBlock(editorView, e.clientX, e.clientY);
          if (!result) {
            if (!isHandleHovered) hideHandle();
            return;
          }

          if (result.dom === hoveredDom) return;

          hoveredDom = result.dom;
          activeNodePos = result.pos;
          activeNodeEnd = result.pos + result.node.nodeSize;
          positionHandle(result.dom);
        };

        const onEditorMouseLeave = () => {
          setTimeout(() => {
            if (!isHandleHovered) hideHandle();
          }, 80);
        };

        // ── Handle hover ──
        handle.addEventListener("mouseenter", () => {
          isHandleHovered = true;
        });
        handle.addEventListener("mouseleave", () => {
          isHandleHovered = false;
          if (!editorDom.matches(":hover")) hideHandle();
        });

        // ── Handle click / drag ──
        handle.addEventListener("mousedown", (e: MouseEvent) => {
          e.preventDefault();
          e.stopPropagation();
          if (activeNodePos == null) return;

          const from = activeNodePos;
          const to = activeNodeEnd;

          // Select the block — prefer NodeSelection, fall back to TextSelection
          try {
            const tr = editorView.state.tr.setSelection(
              NodeSelection.create(editorView.state.doc, from),
            );
            editorView.dispatch(tr);
          } catch {
            try {
              const tr = editorView.state.tr.setSelection(
                TextSelection.create(editorView.state.doc, from + 1, to - 1),
              );
              editorView.dispatch(tr);
            } catch {
              /* ignore */
            }
          }

          editorView.focus();
          startDrag(editorView, e, from, to, true);
        });

        // ── Scroll hides handle ──
        const scrollParent = wrap.closest(".editor-container");
        const onScroll = () => hideHandle();

        editorDom.addEventListener("mousemove", onEditorMouseMove);
        editorDom.addEventListener("mouseleave", onEditorMouseLeave);
        scrollParent?.addEventListener("scroll", onScroll, { passive: true });

        return {
          update() {
            if (hoveredDom && activeNodePos != null) {
              if (document.body.contains(hoveredDom)) {
                positionHandle(hoveredDom);
              } else {
                hideHandle();
              }
            }
          },
          destroy() {
            editorDom.removeEventListener("mousemove", onEditorMouseMove);
            editorDom.removeEventListener("mouseleave", onEditorMouseLeave);
            scrollParent?.removeEventListener("scroll", onScroll);
            handle.remove();
          },
        };
      },
    });

    return [contentDragPlugin, blockHandlePlugin];
  },
});
