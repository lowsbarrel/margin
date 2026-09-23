import { FileText, FileType, Hash, Image } from '@lucide/svelte';
import type { FsEntry, SearchHit, TagInfo } from '$lib/fs/bridge';
import { IMAGE_EXTS } from '$lib/utils/mime';

export type SpotlightItem =
	| { kind: 'name'; path: string; entry: FsEntry }
	| { kind: 'content'; path: string; hit: SearchHit }
	| { kind: 'tag'; tag: string; count: number }
	| { kind: 'tagfile'; path: string; tag: string };

export function itemKey(item: SpotlightItem): string {
	return item.kind + ':' + (item.kind === 'tag' ? item.tag : item.path);
}

export function fileIcon(path: string) {
	const ext = path.slice(path.lastIndexOf('.')).toLowerCase();
	if (ext === '.md') return FileText;
	if (IMAGE_EXTS.has(ext)) return Image;
	if (ext === '.pdf') return FileType;
	return Hash;
}

interface ItemSource {
	askMode: boolean;
	tagMode: boolean;
	selectedTag: string | null;
	tags: TagInfo[];
	tagFilter: string;
	names: FsEntry[];
	hits: SearchHit[];
}

export function buildItems(source: ItemSource): SpotlightItem[] {
	if (source.askMode) return [];

	if (source.tagMode) {
		const tag = source.selectedTag;
		if (tag) {
			const paths = source.tags.find((t) => t.tag === tag)?.files ?? [];
			return paths.map((path): SpotlightItem => ({ kind: 'tagfile', path, tag }));
		}
		return source.tags
			.filter((t) => !source.tagFilter || t.tag.includes(source.tagFilter))
			.map((t): SpotlightItem => ({ kind: 'tag', tag: t.tag, count: t.count }));
	}

	const out: SpotlightItem[] = [];
	const named = source.names.map((entry) => entry.path);
	for (const entry of source.names) out.push({ kind: 'name', path: entry.path, entry });
	for (const hit of source.hits) {
		if (named.includes(hit.path)) continue;
		out.push({ kind: 'content', path: hit.path, hit });
	}
	return out;
}
