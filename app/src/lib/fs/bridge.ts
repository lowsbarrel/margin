import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { initWriteQueue, queuedWrite } from "./writeQueue";

export interface FsEntry {
  name: string;
  is_dir: boolean;
  path: string;
  /** Seconds since UNIX epoch (file modification time). 0 if unavailable. */
  modified: number;
}

export async function setVaultDirectory(path: string): Promise<void> {
  return invoke("set_vault_directory", { path });
}

export async function readFileBytes(path: string): Promise<Uint8Array> {
  const buffer = await invoke<ArrayBuffer>("read_file_bytes", { path });
  return new Uint8Array(buffer);
}

async function rawWriteFileBytes(
  path: string,
  content: Uint8Array,
): Promise<void> {
  return invoke("write_file_bytes", content, {
    headers: { "x-path": path },
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

export async function listDirectory(path: string): Promise<FsEntry[]> {
  return invoke<FsEntry[]>("list_directory", { path });
}

/** Walk entire directory tree in one IPC call. Hidden entries excluded unless includeHidden. */
export async function walkDirectory(
  root: string,
  includeHidden = false,
): Promise<FsEntry[]> {
  return invoke<FsEntry[]>("walk_directory", { root, includeHidden });
}

export interface LinkEntry {
  path: string;
  links: string[];
}

/** Extract [[wiki-links]] from multiple Markdown files in a single native call. */
export async function readLinkBatch(paths: string[]): Promise<LinkEntry[]> {
  return invoke<LinkEntry[]>("read_link_batch", { paths });
}

export interface TreeEntry {
  name: string;
  path: string;
  is_dir: boolean;
  modified: number;
  /** Nesting depth — 0 = vault root level. */
  depth: number;
}

/** Build a flat sorted list of visible tree rows in one native call. */
export async function buildVisibleTree(
  root: string,
  expanded: string[],
  sortBy: string,
): Promise<TreeEntry[]> {
  return invoke<TreeEntry[]>("build_visible_tree", { root, expanded, sortBy });
}

/** Build subtree for a single folder — used for incremental expand. */
export async function buildSubtree(
  folder: string,
  depthOffset: number,
  expanded: string[],
  sortBy: string,
): Promise<TreeEntry[]> {
  return invoke<TreeEntry[]>("build_subtree", { folder, depthOffset, expanded, sortBy });
}

export async function deleteEntry(path: string): Promise<void> {
  return invoke("delete_entry", { path });
}

export async function renameEntry(from: string, to: string): Promise<void> {
  return invoke("rename_entry", { from, to });
}

export async function createDirectory(path: string): Promise<void> {
  return invoke("create_directory", { path });
}

export async function fileExists(path: string): Promise<boolean> {
  return invoke<boolean>("file_exists", { path });
}

export async function copyFile(from: string, to: string): Promise<void> {
  return invoke("copy_file", { from, to });
}

export async function copyDirectory(from: string, to: string): Promise<void> {
  return invoke("copy_directory", { from, to });
}

export async function watchFile(path: string): Promise<void> {
  return invoke("watch_file", { path });
}

export async function unwatchFile(): Promise<void> {
  return invoke("unwatch_file");
}

export async function onFileChanged(
  callback: (path: string) => void,
): Promise<UnlistenFn> {
  return listen<string>("file-changed", (event) => {
    callback(event.payload);
  });
}

export async function watchVault(path: string): Promise<void> {
  return invoke("watch_vault", { path });
}

export async function unwatchVault(): Promise<void> {
  return invoke("unwatch_vault");
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
  return invoke<boolean>("has_unsynced_changes", { vaultPath, encryptionKey });
}

export async function searchFiles(
  root: string,
  query: string,
): Promise<FsEntry[]> {
  return invoke<FsEntry[]>("search_files", { root, query });
}

export interface ContentMatch {
  path: string;
  name: string;
  line: number;
  column: number;
  context: string;
}

export async function searchFileContents(
  root: string,
  query: string,
  caseSensitive: boolean = false,
): Promise<ContentMatch[]> {
  return invoke<ContentMatch[]>("search_file_contents", {
    root,
    query,
    caseSensitive,
  });
}

export async function replaceInFile(
  path: string,
  search: string,
  replace: string,
  caseSensitive: boolean = false,
): Promise<number> {
  return invoke<number>("replace_in_file", {
    path,
    search,
    replace,
    caseSensitive,
  });
}

export async function revealInFileManager(path: string): Promise<void> {
  return invoke("reveal_in_file_manager", { path });
}

export async function setMtime(path: string, mtime: number): Promise<void> {
  return invoke("set_mtime", { path, mtime });
}

export interface TagInfo {
  tag: string;
  count: number;
  files: string[];
}

export async function listAllTags(root: string): Promise<TagInfo[]> {
  return invoke<TagInfo[]>("list_all_tags", { root });
}

export async function exportVaultZip(
  vaultPath: string,
  destPath: string,
): Promise<void> {
  return invoke("export_vault_zip", { vaultPath, destPath });
}

// ── Text processing (Rust-accelerated) ──

export interface TextMatch {
  from: number;
  to: number;
}

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
  return invoke<TextMatch[]>("search_in_text", {
    text,
    pmOffsets,
    gaps,
    needle,
    caseSensitive,
  });
}

export interface TextNode {
  text: string;
  pos: number;
}

export interface WikiLinkMatch {
  from: number;
  to: number;
  title: string;
}

/** Extract [[wiki-links]] from ProseMirror text nodes in a single Rust call. */
export async function extractWikiLinks(
  nodes: TextNode[],
): Promise<WikiLinkMatch[]> {
  return invoke<WikiLinkMatch[]>("extract_wiki_links", { nodes });
}
