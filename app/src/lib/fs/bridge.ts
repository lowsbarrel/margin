import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { commands } from '$lib/bindings';
import { initWriteQueue, queuedWrite, flushWriteQueue, remapRecentWrites } from './write-queue';

export { flushWriteQueue };

export type {
	Backlink,
	FileMetadata,
	FsEntry,
	TreeEntry,
	SearchHit,
	TagInfo,
	TextMatch,
	TextNode,
	WikiLinkMatch
} from '$lib/bindings';
import type {
	Backlink,
	FileMetadata,
	FsEntry,
	TreeEntry,
	SearchHit,
	TagInfo,
	TextMatch,
	TextNode,
	WikiLinkMatch
} from '$lib/bindings';

const X_PATH_HEADER = 'x-path';

// Header values are Latin-1, so arbitrary UTF-8 names are percent-encoded here.
const X_FOLDER_HEADER = 'x-folder';
const X_NAME_HEADER = 'x-name';

export async function setVaultDirectory(path: string): Promise<void> {
	const r = await commands.setVaultDirectory(path);
	if (r.status === 'error') throw r.error;
}

export async function readFileBytes(path: string): Promise<Uint8Array> {
	const buffer = await invoke<ArrayBuffer>('read_file_bytes', { path });
	return new Uint8Array(buffer);
}

async function rawWriteFileBytes(path: string, content: Uint8Array): Promise<void> {
	return invoke<void>('write_file_bytes', content, {
		headers: { [X_PATH_HEADER]: path }
	});
}

initWriteQueue(rawWriteFileBytes);

export function writeFileBytes(path: string, content: Uint8Array): Promise<void> {
	return queuedWrite(path, content);
}

export async function writeFileBytesRaw(path: string, content: Uint8Array): Promise<void> {
	return rawWriteFileBytes(path, content);
}

export async function storeAttachmentBytes(
	folder: string,
	name: string,
	content: Uint8Array
): Promise<string> {
	return invoke<string>('store_attachment_bytes', content, {
		headers: {
			[X_FOLDER_HEADER]: encodeURIComponent(folder),
			[X_NAME_HEADER]: encodeURIComponent(name)
		}
	});
}

export async function saveFileBytes(path: string, content: Uint8Array): Promise<void> {
	return invoke<void>('save_file_bytes', content, {
		headers: { [X_PATH_HEADER]: path }
	});
}

export async function listDirectory(path: string): Promise<FsEntry[]> {
	const r = await commands.listDirectory(path);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function walkDirectory(root: string, includeHidden = false): Promise<FsEntry[]> {
	const r = await commands.walkDirectory(root, includeHidden);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function buildVisibleTree(
	root: string,
	expanded: string[],
	sortBy: string,
	hidden: string[]
): Promise<TreeEntry[]> {
	const r = await commands.buildVisibleTree(root, expanded, sortBy, hidden);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function buildSubtree(
	folder: string,
	depthOffset: number,
	expanded: string[],
	sortBy: string,
	hidden: string[]
): Promise<TreeEntry[]> {
	const r = await commands.buildSubtree(folder, depthOffset, expanded, sortBy, hidden);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function deleteEntry(path: string): Promise<void> {
	const r = await commands.deleteEntry(path);
	if (r.status === 'error') throw r.error;
}

export async function renameEntry(from: string, to: string): Promise<void> {
	const r = await commands.renameEntry(from, to);
	if (r.status === 'error') throw r.error;
	remapRecentWrites(from, to);
}

export async function createDirectory(path: string): Promise<void> {
	const r = await commands.createDirectory(path);
	if (r.status === 'error') throw r.error;
}

export async function fileExists(path: string): Promise<boolean> {
	return commands.fileExists(path);
}

export async function fileMetadata(path: string): Promise<FileMetadata> {
	const r = await commands.fileMetadata(path);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function copyFile(from: string, to: string): Promise<void> {
	const r = await commands.copyFile(from, to);
	if (r.status === 'error') throw r.error;
}

export async function importExternalFile(from: string, to: string): Promise<void> {
	const r = await commands.importExternalFile(from, to);
	if (r.status === 'error') throw r.error;
}

export async function copyDirectory(from: string, to: string): Promise<void> {
	const r = await commands.copyDirectory(from, to);
	if (r.status === 'error') throw r.error;
}

export async function importAttachment(from: string, folder: string): Promise<string> {
	const r = await commands.importAttachment(from, folder);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function sweepUnusedAttachments(folder: string): Promise<number> {
	const r = await commands.sweepUnusedAttachments(folder);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function importExternalDirectory(from: string, to: string): Promise<void> {
	const r = await commands.importExternalDirectory(from, to);
	if (r.status === 'error') throw r.error;
}

export async function watchFile(path: string): Promise<void> {
	const r = await commands.watchFile(path);
	if (r.status === 'error') throw r.error;
}

export async function unwatchFile(): Promise<void> {
	const r = await commands.unwatchFile();
	if (r.status === 'error') throw r.error;
}

export async function onFileChanged(callback: (path: string) => void): Promise<UnlistenFn> {
	return listen<string>('file-changed', (event) => {
		callback(event.payload);
	});
}

export async function watchVault(path: string): Promise<void> {
	const r = await commands.watchVault(path);
	if (r.status === 'error') throw r.error;
}

export async function unwatchVault(): Promise<void> {
	const r = await commands.unwatchVault();
	if (r.status === 'error') throw r.error;
}

export async function onVaultFsChanged(callback: () => void): Promise<UnlistenFn> {
	return listen<void>('vault-fs-changed', () => {
		callback();
	});
}

export async function hasUnsyncedChanges(
	vaultPath: string,
	encryptionKey: number[]
): Promise<boolean> {
	const r = await commands.hasUnsyncedChanges(vaultPath, encryptionKey);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function searchFiles(root: string, query: string): Promise<FsEntry[]> {
	const r = await commands.searchFiles(root, query);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function searchIndex(
	root: string,
	query: string,
	limit: number = 100
): Promise<SearchHit[]> {
	const r = await commands.indexSearch(root, query, limit);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function rebuildIndex(root: string): Promise<number> {
	const r = await commands.indexRebuild(root);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function replaceInFile(
	path: string,
	search: string,
	replace: string,
	caseSensitive: boolean = false
): Promise<number> {
	const r = await commands.replaceInFile(path, search, replace, caseSensitive);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function revealInFileManager(path: string): Promise<void> {
	const r = await commands.revealInFileManager(path);
	if (r.status === 'error') throw r.error;
}

export async function setMtime(path: string, mtime: number): Promise<void> {
	const r = await commands.setMtime(path, mtime);
	if (r.status === 'error') throw r.error;
}

export async function listAllTags(root: string): Promise<TagInfo[]> {
	const r = await commands.indexTags(root);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function listBacklinks(root: string, path: string): Promise<Backlink[]> {
	const r = await commands.indexBacklinks(root, path);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function exportVaultZip(vaultPath: string, destPath: string): Promise<void> {
	const r = await commands.exportVaultZip(vaultPath, destPath);
	if (r.status === 'error') throw r.error;
}

export async function searchInText(
	text: string,
	pmOffsets: number[],
	gaps: number[],
	needle: string,
	caseSensitive: boolean
): Promise<TextMatch[]> {
	return commands.searchInText(text, pmOffsets, gaps, needle, caseSensitive);
}

export async function extractWikiLinks(nodes: TextNode[]): Promise<WikiLinkMatch[]> {
	return commands.extractWikiLinks(nodes);
}
