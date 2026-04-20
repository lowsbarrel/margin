/**
 * Table DnD extension — drag handles for row/column reordering.
 * Uses pointer events instead of native DnD (which doesn't work in Tauri WebView).
 * Adapted from Docmost (Apache 2.0).
 */

import { Extension } from "@tiptap/core";
import type { Editor } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import { computePosition, offset } from "@floating-ui/dom";

import {
  type DraggingDOMs,
  type HoveringCellInfo,
  getDndRelatedDOMs,
  getHoveringCell,
  getDragOverColumn,
  getDragOverRow,
  moveColumn,
  moveRow,
} from "./table-dnd-utils";

// ─── Constants ──────────────────────────────────────────────────────────

const DROP_INDICATOR_WIDTH = 2;
const EDGE_THRESHOLD = 80;
const SCROLL_SPEED = 12;
const DRAG_THRESHOLD = 4; // px before mousedown becomes a drag

// ─── Extension export ───────────────────────────────────────────────────

const TableDndKey = new PluginKey("table-drag-and-drop");

export const TableDndExtension = Extension.create({
  name: "tableDragAndDrop",
  addProseMirrorPlugins() {
    const editor = this.editor;

    // ── DOM elements ──────────────────────────────────

    function createHandle(type: "col" | "row"): HTMLElement {
      const el = document.createElement("div");
      el.classList.add("table-drag-handle");
      el.dataset.direction = type === "col" ? "horizontal" : "vertical";
      el.dataset.handleType = type;
      Object.assign(el.style, {
        position: "absolute",
        top: "-999px",
        left: "-999px",
        display: "none",
        cursor: "grab",
        userSelect: "none",
      });
      return el;
    }

    const colHandle = createHandle("col");
    const rowHandle = createHandle("row");

    const preview = document.createElement("div");
    preview.classList.add("table-dnd-preview", "ProseMirror");
    Object.assign(preview.style, {
      position: "absolute",
      pointerEvents: "none",
      display: "none",
    });

    const dropIndicator = document.createElement("div");
    dropIndicator.classList.add("table-dnd-drop-indicator");
    Object.assign(dropIndicator.style, {
      position: "absolute",
      pointerEvents: "none",
      display: "none",
    });

    // ── State ─────────────────────────────────────────

    let hoveringCell: HoveringCellInfo | undefined;
    let dragging = false;
    let draggingDirection: "col" | "row" = "col";
    let draggingIndex = -1;
    let droppingIndex = -1;
    let dragCellPos: number | undefined;
    let draggingDOMs: DraggingDOMs | undefined;
    let startCoords = { x: 0, y: 0 };
    let scrollInterval: number | undefined;

    // ── Handle visibility ─────────────────────────────

    function hideHandles() {
      const h = { display: "none", left: "-999px", top: "-999px" };
      Object.assign(colHandle.style, h);
      Object.assign(rowHandle.style, h);
    }

    function showHandles(cell: HoveringCellInfo) {
      // Column handle above
      const colRef = editor.view.nodeDOM(cell.colFirstCellPos);
      if (colRef) {
        const yOff = -parseInt(getComputedStyle(colHandle).height || "18") / 2;
        computePosition(colRef as HTMLElement, colHandle, {
          placement: "top",
          middleware: [offset(yOff)],
        }).then(({ x, y }) => {
          Object.assign(colHandle.style, { display: "block", top: `${y}px`, left: `${x}px` });
        });
      }
      // Row handle to the left
      const rowRef = editor.view.nodeDOM(cell.rowFirstCellPos);
      if (rowRef) {
        const xOff = -parseInt(getComputedStyle(rowHandle).width || "18") / 2;
        computePosition(rowRef as HTMLElement, rowHandle, {
          placement: "left",
          middleware: [offset(xOff)],
        }).then(({ x, y }) => {
          Object.assign(rowHandle.style, { display: "block", top: `${y}px`, left: `${x}px` });
        });
      }
    }

    // ── Preview ───────────────────────────────────────

    function showPreview(doms: DraggingDOMs, index: number, type: "col" | "row") {
      while (preview.firstChild) preview.removeChild(preview.firstChild);

      const tRect = doms.table.getBoundingClientRect();
      const cRect = doms.cell.getBoundingClientRect();

      if (type === "col") {
        Object.assign(preview.style, { display: "block", width: `${cRect.width}px`, height: `${tRect.height}px` });
      } else {
        Object.assign(preview.style, { display: "block", width: `${tRect.width}px`, height: `${cRect.height}px` });
      }

      const previewTable = document.createElement("table");
      const body = document.createElement("tbody");
      previewTable.appendChild(body);
      preview.appendChild(previewTable);
      const rows = doms.table.querySelectorAll("tr");

      if (type === "row") {
        const row = rows[index];
        if (row) body.appendChild(row.cloneNode(true));
      } else {
        rows.forEach((row) => {
          const rowDOM = row.cloneNode(false) as HTMLElement;
          const cells = row.querySelectorAll("th,td");
          if (cells[index]) {
            rowDOM.appendChild(cells[index].cloneNode(true));
            body.appendChild(rowDOM);
          }
        });
      }

      computePosition(doms.cell, preview, {
        placement: type === "row" ? "right" : "bottom",
        middleware: [offset(({ rects }) => type === "col" ? -rects.reference.height : -rects.reference.width)],
      }).then(({ x, y }) => {
        Object.assign(preview.style, { left: `${x}px`, top: `${y}px` });
      });
    }

    function updatePreviewPosition(x: number, y: number, cell: HTMLElement, type: "col" | "row") {
      const vEl = {
        contextElement: cell,
        getBoundingClientRect: () => {
          const r = cell.getBoundingClientRect();
          return {
            width: r.width, height: r.height,
            right: x + r.width / 2, bottom: y + r.height / 2,
            top: y - r.height / 2, left: x - r.width / 2,
            x: x - r.width / 2, y: y - r.height / 2,
          };
        },
      };
      computePosition(vEl, preview, {
        placement: type === "row" ? "right" : "bottom",
      }).then(({ x: px, y: py }) => {
        if (type === "row") Object.assign(preview.style, { top: `${py}px` });
        else Object.assign(preview.style, { left: `${px}px` });
      });
    }

    function hidePreview() {
      while (preview.firstChild) preview.removeChild(preview.firstChild);
      Object.assign(preview.style, { display: "none" });
    }

    // ── Drop indicator ────────────────────────────────

    function showDropIndicator(doms: DraggingDOMs, type: "col" | "row") {
      const tRect = doms.table.getBoundingClientRect();
      if (type === "col") {
        Object.assign(dropIndicator.style, { display: "block", width: `${DROP_INDICATOR_WIDTH}px`, height: `${tRect.height}px` });
      } else {
        Object.assign(dropIndicator.style, { display: "block", width: `${tRect.width}px`, height: `${DROP_INDICATOR_WIDTH}px` });
      }
      computePosition(doms.cell, dropIndicator, {
        placement: type === "row" ? "right" : "bottom",
        middleware: [offset(({ rects }) => type === "col" ? -rects.reference.height : -rects.reference.width)],
      }).then(({ x, y }) => {
        Object.assign(dropIndicator.style, { left: `${x}px`, top: `${y}px` });
      });
      dropIndicator.dataset.dragging = "true";
    }

    function updateDropIndicator(target: Element, direction: string, type: "col" | "row") {
      if (type === "col") {
        computePosition(target, dropIndicator, {
          placement: direction === "left" ? "left" : "right",
          middleware: [offset(direction === "left" ? -DROP_INDICATOR_WIDTH : 0)],
        }).then(({ x }) => {
          Object.assign(dropIndicator.style, { left: `${x}px` });
        });
      } else {
        computePosition(target, dropIndicator, {
          placement: direction === "up" ? "top" : "bottom",
          middleware: [offset(direction === "up" ? -DROP_INDICATOR_WIDTH : 0)],
        }).then(({ y }) => {
          Object.assign(dropIndicator.style, { top: `${y}px` });
        });
      }
    }

    function hideDropIndicator() {
      Object.assign(dropIndicator.style, { display: "none" });
      dropIndicator.dataset.dragging = "false";
    }

    // ── Auto-scroll ───────────────────────────────────

    function stopAutoScroll() {
      if (scrollInterval) { clearInterval(scrollInterval); scrollInterval = undefined; }
    }

    function checkAutoScroll(clientX: number, clientY: number, doms?: DraggingDOMs) {
      stopAutoScroll();
      if (clientY < EDGE_THRESHOLD) {
        scrollInterval = window.setInterval(() => { document.documentElement.scrollTop -= SCROLL_SPEED; }, 16);
        return;
      }
      if (clientY > window.innerHeight - EDGE_THRESHOLD) {
        scrollInterval = window.setInterval(() => { document.documentElement.scrollTop += SCROLL_SPEED; }, 16);
        return;
      }
      if (doms) {
        const wrapper = doms.table.closest<HTMLElement>(".tableWrapper");
        if (wrapper) {
          const rect = wrapper.getBoundingClientRect();
          if (clientX < rect.left + EDGE_THRESHOLD) {
            scrollInterval = window.setInterval(() => { wrapper.scrollLeft -= SCROLL_SPEED; }, 16);
            return;
          }
          if (clientX > rect.right - EDGE_THRESHOLD) {
            scrollInterval = window.setInterval(() => { wrapper.scrollLeft += SCROLL_SPEED; }, 16);
          }
        }
      }
    }

    // ── Mouse drag logic ──────────────────────────────

    function onHandleMouseDown(type: "col" | "row", e: MouseEvent) {
      e.preventDefault();
      e.stopPropagation();

      if (!hoveringCell) return;

      const initX = e.clientX;
      const initY = e.clientY;
      let started = false;

      draggingDirection = type;
      dragCellPos = hoveringCell.cellPos;
      draggingIndex = type === "col" ? hoveringCell.colIndex : hoveringCell.rowIndex;
      droppingIndex = -1;

      const doms = getDndRelatedDOMs(editor.view, hoveringCell.cellPos, draggingIndex, type);
      draggingDOMs = doms;
      const savedCell = hoveringCell;

      const onMouseMove = (ev: MouseEvent) => {
        const dx = ev.clientX - initX;
        const dy = ev.clientY - initY;

        if (!started) {
          if (Math.abs(dx) + Math.abs(dy) < DRAG_THRESHOLD) return;
          started = true;
          dragging = true;
          startCoords = { x: initX, y: initY };
          document.body.style.cursor = "grabbing";
          document.body.style.userSelect = "none";

          if (doms) {
            const idx = type === "col" ? savedCell.colIndex : savedCell.rowIndex;
            showPreview(doms, idx, type);
            showDropIndicator(doms, type);
          }
        }

        if (!doms) return;

        updatePreviewPosition(ev.clientX, ev.clientY, doms.cell, type);
        checkAutoScroll(ev.clientX, ev.clientY, doms);

        if (type === "col") {
          const dir = startCoords.x > ev.clientX ? "left" : "right";
          const over = getDragOverColumn(doms.table, ev.clientX);
          if (over) {
            droppingIndex = over[1];
            updateDropIndicator(over[0], dir, "col");
          }
        } else {
          const dir = startCoords.y > ev.clientY ? "up" : "down";
          const over = getDragOverRow(doms.table, ev.clientY);
          if (over) {
            droppingIndex = over[1];
            updateDropIndicator(over[0], dir, "row");
          }
        }
      };

      const onMouseUp = () => {
        window.removeEventListener("mousemove", onMouseMove, true);
        window.removeEventListener("mouseup", onMouseUp, true);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
        stopAutoScroll();
        hidePreview();
        hideDropIndicator();

        if (started && dragging && dragCellPos != null && droppingIndex >= 0) {
          const tr = editor.state.tr;
          const pos = dragCellPos;
          if (draggingDirection === "col") {
            if (moveColumn({ tr, originIndex: draggingIndex, targetIndex: droppingIndex, select: true, pos })) {
              editor.view.dispatch(tr);
            }
          } else {
            if (moveRow({ tr, originIndex: draggingIndex, targetIndex: droppingIndex, select: true, pos })) {
              editor.view.dispatch(tr);
            }
          }
        }

        dragging = false;
        draggingIndex = -1;
        droppingIndex = -1;
        dragCellPos = undefined;
        draggingDOMs = undefined;
      };

      window.addEventListener("mousemove", onMouseMove, true);
      window.addEventListener("mouseup", onMouseUp, true);
    }

    // ── Event listeners on handles ────────────────────

    const onColDown = (e: MouseEvent) => onHandleMouseDown("col", e);
    const onRowDown = (e: MouseEvent) => onHandleMouseDown("row", e);
    colHandle.addEventListener("mousedown", onColDown);
    rowHandle.addEventListener("mousedown", onRowDown);

    // ── ProseMirror plugin ────────────────────────────

    const plugin = new Plugin({
      key: TableDndKey,

      view() {
        const wrapper = editor.options.element as HTMLElement | undefined;
        if (wrapper) {
          wrapper.appendChild(colHandle);
          wrapper.appendChild(rowHandle);
          wrapper.appendChild(preview);
          wrapper.appendChild(dropIndicator);
        }
        return {
          destroy() {
            colHandle.removeEventListener("mousedown", onColDown);
            rowHandle.removeEventListener("mousedown", onRowDown);
            colHandle.remove();
            rowHandle.remove();
            preview.remove();
            dropIndicator.remove();
            stopAutoScroll();
          },
        };
      },

      props: {
        handleDOMEvents: {
          pointerover(view: EditorView, event: PointerEvent) {
            if (dragging) return;
            if (!editor.isEditable) {
              hideHandles();
              return;
            }
            const cell = getHoveringCell(view, event);
            hoveringCell = cell;
            if (!cell) hideHandles();
            else showHandles(cell);
          },
        },
      },
    });

    return [plugin];
  },
});
