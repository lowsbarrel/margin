import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands } from "$lib/bindings";
import { initWriteQueue, queuedWrite, flushWriteQueue } from "./writeQueue";

/**
 * Re-exported so call sites (e.g. window-close handlers) can await all pending
 * file writes via the fs bridge. Resolves once the latest queued content for
 * every path has landed on disk.
 */
export { flushWriteQueue };

/**
 * Boundary types generated from the Rust structs by tauri-specta. Re-exported
 * so existing call sites that import them from this bridge keep working.
 */
export type {
  FsEntry,
  LinkEntry,
  TreeEntry,
  ContentMatch,
  TagInfo,
  TextMatch,
  TextNode,
  WikiLinkMatch,
} from "$lib/bindings";
import type {
  FsEntry,
  LinkEntry,
  TreeEntry,
  ContentMatch,
  TagInfo,
  TextMatch,
  TextNode,
  WikiLinkMatch,
} from "$lib/bindings";

/**
 * Header key used to smuggle the destination path of `write_file_bytes`.
 * The raw bytes are passed as the invoke args body (avoiding base64 JSON
 * encoding), so the path travels in this request header instead. The Rust
 * `write_file_bytes` command reads the same header — keep the two in sync.
 */
const X_PATH_HEADER = "x-path";

export async function setVaultDirectory(path: string): Promise<void> {
  const r = await commands.setVaultDirectory(path);
  if (r.status === "error") throw r.error;
}

export async function readFileBytes(path: string): Promise<Uint8Array> {
  const buffer = await invoke<ArrayBuffer>("read_file_bytes", { path });
  return new Uint8Array(buffer);
}

async function rawWriteFileBytes(
  path: string,
  content: Uint8Array,
): Promise<void> {
  return invoke<void>("write_file_bytes", content, {
    headers: { [X_PATH_HEADER]: path },
  });
}

initWriteQueue(rawWriteFileBytes);

/**
 * Write file bytes with per-path serialization.
 * Concurrent writes to the same path are queued so only the latest content
 * is written once the current in-flight write completes. No delays.
 */
export function writeFileBytes(
  path: string,
  content: Uint8Array,
): Promise<void> {
  return queuedWrite(path, content);
}

/** Bypass the write queue — use only when serialization is not needed (e.g. writing to a new unique path). */
export async function writeFileBytesRaw(
  path: string,
  content: Uint8Array,
): Promise<void> {
  return rawWriteFileBytes(path, content);
}

/**
 * Save raw bytes to an arbitrary path *outside* the vault containment check.
 * For explicit "save as / export" flows where the destination was chosen by
 * the user through the native save dialog (e.g. exporting a note to PDF onto
 * the Desktop). Mirrors `rawWriteFileBytes` but targets the unguarded
 * `save_file_bytes` command — see the matching Rust command, kept in sync.
 */
export async function saveFileBytes(
  path: string,
  content: Uint8Array,
): Promise<void> {
  return invoke<void>("save_file_bytes", content, {
    headers: { [X_PATH_HEADER]: path },
  });
}

export async function listDirectory(path: string): Promise<FsEntry[]> {
  const r = await commands.listDirectory(path);
  if (r.status === "error") throw r.error;
  return r.data;
}

/** Walk entire directory tree in one IPC call. Hidden entries excluded unless includeHidden. */
export async function walkDirectory(
  root: string,
  includeHidden = false,
): Promise<FsEntry[]> {
  const r = await commands.walkDirectory(root, includeHidden);
  if (r.status === "error") throw r.error;
  return r.data;
}

/** Extract [[wiki-links]] from multiple Markdown files in a single native call. */
export async function readLinkBatch(paths: string[]): Promise<LinkEntry[]> {
  return commands.readLinkBatch(paths);
}

/** Build a flat sorted list of visible tree rows in one native call. */
export async function buildVisibleTree(
  root: string,
  expanded: string[],
  sortBy: string,
): Promise<TreeEntry[]> {
  const r = await commands.buildVisibleTree(root, expanded, sortBy);
  if (r.status === "error") throw r.error;
  return r.data;
}

/** Build subtree for a single folder — used for incremental expand. */
export async function buildSubtree(
  folder: string,
  depthOffset: number,
  expanded: string[],
  sortBy: string,
): Promise<TreeEntry[]> {
  const r = await commands.buildSubtree(folder, depthOffset, expanded, sortBy);
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function deleteEntry(path: string): Promise<void> {
  const r = await commands.deleteEntry(path);
  if (r.status === "error") throw r.error;
}

export async function renameEntry(from: string, to: string): Promise<void> {
  const r = await commands.renameEntry(from, to);
  if (r.status === "error") throw r.error;
}

export async function createDirectory(path: string): Promise<void> {
  const r = await commands.createDirectory(path);
  if (r.status === "error") throw r.error;
}

export async function fileExists(path: string): Promise<boolean> {
  return commands.fileExists(path);
}

export async function copyFile(from: string, to: string): Promise<void> {
  const r = await commands.copyFile(from, to);
  if (r.status === "error") throw r.error;
}

/**
 * Copy a file from an arbitrary source *outside* the vault into a
 * vault-contained destination — for drag-drop / import of an external file as
 * an attachment. Unlike `copyFile`, the source is NOT containment-checked (the
 * user explicitly chose it); only the destination must resolve inside the
 * vault. Mirrors the `import_external_file` Rust command — keep the two in sync.
 */
export async function importExternalFile(from: string, to: string): Promise<void> {
  return invoke<void>("import_external_file", { from, to });
}

export async function copyDirectory(from: string, to: string): Promise<void> {
  const r = await commands.copyDirectory(from, to);
  if (r.status === "error") throw r.error;
}

export async function watchFile(path: string): Promise<void> {
  const r = await commands.watchFile(path);
  if (r.status === "error") throw r.error;
}

export async function unwatchFile(): Promise<void> {
  const r = await commands.unwatchFile();
  if (r.status === "error") throw r.error;
}

export async function onFileChanged(
  callback: (path: string) => void,
): Promise<UnlistenFn> {
  return listen<string>("file-changed", (event) => {
    callback(event.payload);
  });
}

export async function watchVault(path: string): Promise<void> {
  const r = await commands.watchVault(path);
  if (r.status === "error") throw r.error;
}

export async function unwatchVault(): Promise<void> {
  const r = await commands.unwatchVault();
  if (r.status === "error") throw r.error;
}

export async function onVaultFsChanged(
  callback: () => void,
): Promise<UnlistenFn> {
  return listen<void>("vault-fs-changed", () => {
    callback();
  });
}

/** O(n) mtime comparison — no hashing, no per-file IPC. */
export async function hasUnsyncedChanges(
  vaultPath: string,
  encryptionKey: number[],
): Promise<boolean> {
  const r = await commands.hasUnsyncedChanges(vaultPath, encryptionKey);
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function searchFiles(
  root: string,
  query: string,
): Promise<FsEntry[]> {
  const r = await commands.searchFiles(root, query);
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function searchFileContents(
  root: string,
  query: string,
  caseSensitive: boolean = false,
): Promise<ContentMatch[]> {
  const r = await commands.searchFileContents(root, query, caseSensitive);
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function replaceInFile(
  path: string,
  search: string,
  replace: string,
  caseSensitive: boolean = false,
): Promise<number> {
  const r = await commands.replaceInFile(path, search, replace, caseSensitive);
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function revealInFileManager(path: string): Promise<void> {
  const r = await commands.revealInFileManager(path);
  if (r.status === "error") throw r.error;
}

export async function setMtime(path: string, mtime: number): Promise<void> {
  const r = await commands.setMtime(path, mtime);
  if (r.status === "error") throw r.error;
}

export async function listAllTags(root: string): Promise<TagInfo[]> {
  const r = await commands.listAllTags(root);
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function exportVaultZip(
  vaultPath: string,
  destPath: string,
): Promise<void> {
  const r = await commands.exportVaultZip(vaultPath, destPath);
  if (r.status === "error") throw r.error;
}

// ── Text processing (Rust-accelerated) ──

/**
 * Fast substring search on flattened ProseMirror text via Rust memchr.
 * @param pmOffsets Parallel array mapping each char index to its PM position.
 * @param gaps Sorted char indices where a block boundary exists.
 */
export async function searchInText(
  text: string,
  pmOffsets: number[],
  gaps: number[],
  needle: string,
  caseSensitive: boolean,
): Promise<TextMatch[]> {
  return commands.searchInText(text, pmOffsets, gaps, needle, caseSensitive);
}

/** Extract [[wiki-links]] from ProseMirror text nodes in a single Rust call. */
export async function extractWikiLinks(
  nodes: TextNode[],
): Promise<WikiLinkMatch[]> {
  return commands.extractWikiLinks(nodes);
}
