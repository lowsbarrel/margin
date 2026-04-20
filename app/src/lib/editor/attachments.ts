import type { Editor } from "@tiptap/core";
import {
  writeFileBytes,
  fileExists,
  createDirectory,
  copyFile,
} from "$lib/fs/bridge";
import { isImageFile } from "$lib/utils/mime";
import { buildLocalfileUrl } from "$lib/editor/image-url";

function makeSafeFileName(name: string): string {
  const ts = Date.now();
  const safeName = name.replace(/[^a-zA-Z0-9._-]/g, "_");
  return `${ts}-${safeName}`;
}

async function ensureAttachmentDir(attDir: string): Promise<void> {
  const dirExists = await fileExists(attDir);
  if (!dirExists) await createDirectory(attDir);
}

interface InsertOptions {
  editor: Editor;
  vaultPath: string;
  attachmentFolder: string;
}

/** Insert a pasted File (from clipboard) as an attachment */
export async function insertPastedFile(
  file: File,
  { editor, vaultPath, attachmentFolder }: InsertOptions,
): Promise<void> {
  const attDir = `${vaultPath}/${attachmentFolder}`;
  await ensureAttachmentDir(attDir);

  const fileName = makeSafeFileName(file.name);
  const destPath = `${attDir}/${fileName}`;

  const buffer = await file.arrayBuffer();
  await writeFileBytes(destPath, new Uint8Array(buffer));

  const relPath = `${attachmentFolder}/${fileName}`;
  insertIntoEditor(
    editor,
    relPath,
    file.name,
    isImageFile(file.name, file.type),
    vaultPath,
  );
}

/** Insert a dropped file (from OS drag-drop, given as a path) as an attachment */
export async function insertDroppedFile(
  srcPath: string,
  { editor, vaultPath, attachmentFolder }: InsertOptions,
): Promise<void> {
  const attDir = `${vaultPath}/${attachmentFolder}`;
  await ensureAttachmentDir(attDir);

  const name = srcPath.replace(/\\/g, "/").split("/").pop() ?? "file";
  const fileName = makeSafeFileName(name);
  const destPath = `${attDir}/${fileName}`;

  await copyFile(srcPath, destPath);

  const relPath = `${attachmentFolder}/${fileName}`;
  insertIntoEditor(editor, relPath, name, isImageFile(name), vaultPath);
}

function insertIntoEditor(
  editor: Editor,
  relPath: string,
  displayName: string,
  isImage: boolean,
  vaultPath: string,
): void {
  if (isImage) {
    const src = buildLocalfileUrl(`${vaultPath}/${relPath}`);
    editor.chain().focus().setImage({ src, alt: displayName }).run();
  } else {
    editor
      .chain()
      .focus()
      .setFileEmbed({ src: relPath, filename: displayName })
      .run();
  }
}
