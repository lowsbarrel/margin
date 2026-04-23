import type { Editor } from "@tiptap/core";
import { TextSelection } from "@tiptap/pm/state";
import { insertDroppedFile } from "$lib/editor/attachments";
import { toast } from "$lib/stores/toast.svelte";
import * as m from "$lib/paraglide/messages.js";

/** Move the editor cursor to a screen coordinate */
export function setCursorAtCoords(editor: Editor, x: number, y: number): void {
  const result = editor.view.posAtCoords({ left: x, top: y });
  if (result == null) return;
  const pos = Math.min(result.pos, editor.view.state.doc.content.size);
  try {
    const tr = editor.view.state.tr.setSelection(
      TextSelection.create(editor.view.state.doc, pos),
    );
    editor.view.dispatch(tr);
  } catch {
    /* pos may be invalid for non-text nodes */
  }
}

/** Insert a file (from sidebar drag) at the current cursor position */
export async function insertFileAtCursor(
  path: string,
  editor: Editor,
  vaultPath: string,
  attachmentFolder: string | null,
): Promise<void> {
  const name = path.split("/").pop() ?? path;
  const ext = name.split(".").pop()?.toLowerCase() ?? "";

  if (ext === "md") {
    const title = name.slice(0, -3);
    editor.chain().insertWikiLink(title).run();
    return;
  }

  if (ext === "canvas") {
    const title = name.slice(0, -7);
    editor.chain().insertWikiLink(title).run();
    return;
  }

  if (!attachmentFolder) {
    toast.error(m.toast_set_attachment_folder());
    return;
  }

  try {
    await insertDroppedFile(path, {
      editor,
      vaultPath,
      attachmentFolder,
    });
  } catch (err) {
    toast.error(m.toast_insert_file_failed({ error: String(err) }));
  }
}

/** Handle Tauri drag-drop events (OS-level file drops) */
export async function handleTauriFileDrop(
  paths: string[],
  position: { x: number; y: number } | undefined,
  editor: Editor,
  container: HTMLElement,
  vaultPath: string,
  attachmentFolder: string | null,
): Promise<void> {
  // Only handle drops that land on the editor container
  if (position && container) {
    const rect = container.getBoundingClientRect();
    if (
      position.x < rect.left ||
      position.x > rect.right ||
      position.y < rect.top ||
      position.y > rect.bottom
    )
      return;
  }

  // Place cursor at drop position before inserting
  if (position) setCursorAtCoords(editor, position.x, position.y);

  if (!attachmentFolder) {
    toast.error(m.toast_set_attachment_folder());
    return;
  }

  for (const srcPath of paths) {
    const name = srcPath.replace(/\\/g, "/").split("/").pop() ?? srcPath;
    const ext = name.split(".").pop()?.toLowerCase() ?? "";

    if (ext === "md") {
      const title = name.slice(0, -3);
      editor.chain().insertWikiLink(title).run();
      continue;
    }

    try {
      await insertDroppedFile(srcPath, {
        editor,
        vaultPath,
        attachmentFolder,
      });
    } catch (err) {
      toast.error(m.toast_insert_file_failed({ error: String(err) }));
    }
  }
}
