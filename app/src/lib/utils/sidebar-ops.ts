import { listDirectory } from '$lib/fs/bridge';
import { validateName, displayName } from '$lib/utils/filename';
import { toast } from '$lib/stores/toast.svelte';

export { displayName };

export function normalizeFileName(input: string): string | null {
	const name = input.trim();
	const error = validateName(name);
	if (error) {
		toast.error(error);
		return null;
	}
	return name.includes('.') ? name : `${name}.md`;
}

export function normalizeDirName(input: string): string | null {
	const name = input.trim();
	const error = validateName(name);
	if (error) {
		toast.error(error);
		return null;
	}
	return name;
}

export async function createUniqueFilePath(
	base: string,
	desiredName?: string
): Promise<string | null> {
	const name = desiredName ? normalizeFileName(desiredName) : 'Untitled.md';
	if (!name) return null;

	const extIndex = name.lastIndexOf('.');
	const stem = extIndex > 0 ? name.slice(0, extIndex) : name;
	const ext = extIndex > 0 ? name.slice(extIndex) : '.md';

	// List the parent directory once and resolve the free name client-side
	// instead of issuing one fileExists IPC call per candidate.
	let existing: Set<string>;
	try {
		const entries = await listDirectory(base);
		existing = new Set(entries.map((entry) => entry.name));
	} catch {
		// Directory not yet listable (e.g. just created) — assume it is empty.
		existing = new Set();
	}

	if (!existing.has(name)) return `${base}/${name}`;
	let i = 1;
	let candidateName = `${stem} ${i}${ext}`;
	while (existing.has(candidateName)) {
		i++;
		candidateName = `${stem} ${i}${ext}`;
	}
	return `${base}/${candidateName}`;
}

export function displayPath(fullPath: string, vaultPath: string | null): string {
	if (!vaultPath) return fullPath;
	const rel = fullPath.slice(vaultPath.length + 1);
	const parts = rel.split('/');
	if (parts.length <= 1) return '';
	return parts.slice(0, -1).join('/');
}

export interface HighlightSegment {
	text: string;
	match: boolean;
}

/**
 * Split `text` into alternating plain / matched segments for the query terms.
 *
 * This replaces the old `highlightMatch()`, which built an HTML string that the
 * caller had to render with `{@html}`. Returning segments lets the markup do the
 * highlighting, so nothing from a note's body is ever parsed as HTML.
 *
 * Every whitespace-separated term is highlighted independently, which matches
 * what the FTS index actually matched on — a two-word query highlights both
 * words in the snippet rather than only exact adjacent occurrences.
 */
export function splitHighlight(text: string, query: string): HighlightSegment[] {
	const terms = query
		.trim()
		.split(/\s+/)
		.filter(Boolean)
		.map(escapeRegex)
		// Longest first so "foobar" wins over "foo" when both are present.
		.sort((a, b) => b.length - a.length);
	if (terms.length === 0) return [{ text, match: false }];

	const re = new RegExp(`(${terms.join('|')})`, 'gi');
	const segments: HighlightSegment[] = [];
	let last = 0;
	for (const found of text.matchAll(re)) {
		const start = found.index ?? 0;
		if (start > last) segments.push({ text: text.slice(last, start), match: false });
		segments.push({ text: found[0], match: true });
		last = start + found[0].length;
	}
	if (last < text.length) segments.push({ text: text.slice(last), match: false });
	return segments;
}

function escapeRegex(str: string): string {
	return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
