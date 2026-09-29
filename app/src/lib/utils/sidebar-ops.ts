import { listDirectory } from '$lib/fs/bridge';
import { validateName } from '$lib/utils/filename';
import { toast } from '$lib/stores/toast.svelte';

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

export type CollisionSuffix = 'numeric' | 'copy';

// Both macOS and Windows hold `Photo.PNG` and `photo.png` to be the same name.
function uniqueName(name: string, existing: Set<string>, suffix: CollisionSuffix): string {
	const extIndex = name.lastIndexOf('.');
	const stem = extIndex > 0 ? name.slice(0, extIndex) : name;
	const ext = extIndex > 0 ? name.slice(extIndex) : '';
	const taken = new Set([...existing].map((entry) => entry.toLowerCase()));
	const isTaken = (candidate: string) => taken.has(candidate.toLowerCase());

	if (suffix === 'copy') {
		for (let i = 1; ; i++) {
			const candidate = `${stem} copy${i > 1 ? ` ${i}` : ''}${ext}`;
			if (!isTaken(candidate)) return candidate;
		}
	}

	if (!isTaken(name)) return name;
	for (let i = 1; ; i++) {
		const candidate = `${stem} ${i}${ext}`;
		if (!isTaken(candidate)) return candidate;
	}
}

export async function createUniquePath(
	dir: string,
	name: string,
	suffix: CollisionSuffix = 'numeric'
): Promise<string> {
	let existing: Set<string>;
	try {
		const entries = await listDirectory(dir);
		existing = new Set(entries.map((entry) => entry.name));
	} catch {
		existing = new Set();
	}

	return `${dir}/${uniqueName(name, existing, suffix)}`;
}

export async function createUniqueFilePath(
	base: string,
	desiredName?: string
): Promise<string | null> {
	const name = desiredName ? normalizeFileName(desiredName) : 'Untitled.md';
	if (!name) return null;

	return createUniquePath(base, name);
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

export function splitHighlight(text: string, query: string): HighlightSegment[] {
	const terms = query
		.trim()
		.split(/\s+/)
		.filter(Boolean)
		.map(escapeRegex)
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
