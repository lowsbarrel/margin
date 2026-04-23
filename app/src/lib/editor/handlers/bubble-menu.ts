import type { Editor } from "@tiptap/core";
import { computePosition, flip, offset, shift } from "@floating-ui/dom";

/** Toggle `is-active` class on toolbar buttons based on editor state */
export function updateBubbleButtons(
  editor: Editor,
  menuEl: HTMLElement,
): void {
  menuEl
    .querySelectorAll<HTMLButtonElement>("[data-cmd]")
    .forEach((btn) => {
      const cmd = btn.dataset.cmd!;
      btn.classList.toggle("is-active", editor.isActive(cmd));
    });
}

/** Get selection bounding rect from DOM or editor view coordinates */
export function getSelectionRect(
  view: Editor["view"],
  from: number,
  to: number,
): DOMRect | null {
  const selection = window.getSelection();
  if (selection && selection.rangeCount > 0) {
    const range = selection.getRangeAt(0);
    if (!range.collapsed) {
      const rect = range.getBoundingClientRect();
      if (rect.width > 0 || rect.height > 0) {
        return rect;
      }
    }
  }

  const start = view.coordsAtPos(from);
  const end = view.coordsAtPos(to);
  return DOMRect.fromRect({
    x: Math.min(start.left, end.left),
    y: Math.min(start.top, end.top),
    width: Math.max(end.right, start.right) - Math.min(start.left, end.left),
    height:
      Math.max(end.bottom, start.bottom) - Math.min(start.top, end.top),
  });
}

/**
 * Position and show/hide the bubble toolbar using @floating-ui/dom.
 * Returns an object with { x, y } if the menu should be shown, or null to hide.
 */
export async function positionBubbleMenu(
  editor: Editor,
  menuEl: HTMLElement,
): Promise<{ x: number; y: number } | null> {
  const { state, view } = editor;
  const { from, to } = state.selection;

  const shouldShow =
    from !== to &&
    !editor.isActive("codeBlock") &&
    !state.selection.empty &&
    view.hasFocus();

  if (!shouldShow) return null;

  updateBubbleButtons(editor, menuEl);

  const selectionRect = getSelectionRect(view, from, to);
  if (!selectionRect) return null;

  const virtualEl = {
    getBoundingClientRect() {
      return selectionRect;
    },
  };

  try {
    const { x, y } = await computePosition(virtualEl, menuEl, {
      strategy: "fixed",
      placement: "top",
      middleware: [offset(8), flip(), shift({ padding: 8 })],
    });
    return { x, y };
  } catch {
    return null;
  }
}
