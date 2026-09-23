import { mimeForPath } from '$lib/utils/mime';
import { readFileBytes, fileMetadata } from '$lib/fs/bridge';
import type { Tab, TabType, ViewMode } from '$lib/stores/panes.svelte';

export interface TabContent {
	content: string;
	blobUrl?: string;
	pdfData?: Uint8Array;
	size?: number;
	modified?: number;
}

export interface TabOptions {
	viewMode?: ViewMode;
	pinned?: boolean;
	cursorPos?: number;
}

export async function loadTabContent(path: string, type: TabType): Promise<TabContent> {
	const loaded: TabContent = { content: '' };
	if (type === 'unknown') {
		const stats = await fileMetadata(path);
		loaded.size = stats.size ?? undefined;
		loaded.modified = stats.modified;
		return loaded;
	}
	const bytes = await readFileBytes(path);
	loaded.size = bytes.length;
	if (type === 'markdown' || type === 'canvas') {
		loaded.content = new TextDecoder().decode(bytes);
	} else if (type === 'pdf') {
		loaded.pdfData = new Uint8Array(bytes);
	} else {
		const blob = new Blob([bytes.buffer as ArrayBuffer], { type: mimeForPath(path) });
		loaded.blobUrl = URL.createObjectURL(blob);
	}
	return loaded;
}

export function tabFromContent(
	id: number,
	path: string,
	type: TabType,
	loaded: TabContent,
	options: TabOptions = {}
): Tab {
	return {
		id,
		path,
		content: loaded.content,
		type,
		viewMode: options.viewMode ?? 'rich',
		blobUrl: loaded.blobUrl,
		pdfData: loaded.pdfData,
		size: loaded.size,
		modified: loaded.modified,
		pinned: options.pinned ?? false,
		cursorPos: options.cursorPos
	};
}
