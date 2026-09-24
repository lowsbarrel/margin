import {
	buildLocalfileUrl,
	isLocalfileUrl,
	stripLocalfilePrefix,
	toOsPath
} from '$lib/editor/image-url';

const SAFE_DESTINATION = /[A-Za-z0-9\-_.~/]/;
const PERCENT_ESCAPE = /%([0-9A-Fa-f]{2})/g;

export interface ResolveSources {
	vaultPath(): string | null;
	noteDir(): string;
	attachmentFolder(): string;
	exists(relPath: string): boolean;
	findByName(name: string): string | null;
	hasIndex?(): boolean;
}

export interface ResolvedAsset {
	abs: string;
	rel: string;
}

export function decodeDestination(path: string): string {
	if (!path.includes('%')) return path;
	const decoder = new TextDecoder();
	const encoder = new TextEncoder();
	const bytes: number[] = [];
	let last = 0;
	for (const match of path.matchAll(PERCENT_ESCAPE)) {
		bytes.push(...encoder.encode(path.slice(last, match.index)), parseInt(match[1], 16));
		last = (match.index ?? 0) + 3;
	}
	bytes.push(...encoder.encode(path.slice(last)));
	return decoder.decode(Uint8Array.from(bytes));
}

// A space is invalid in a Markdown destination and a `)` ends it early; already-escaped input is decoded first.
export function encodeDestination(path: string): string {
	const bytes = new TextEncoder().encode(decodeDestination(path));
	let out = '';
	for (const byte of bytes) {
		const char = String.fromCharCode(byte);
		out += SAFE_DESTINATION.test(char)
			? char
			: `%${byte.toString(16).toUpperCase().padStart(2, '0')}`;
	}
	return out;
}

export function normalizeRel(path: string): string {
	const parts: string[] = [];
	for (const part of path.replace(/\\/g, '/').split('/')) {
		if (part === '' || part === '.') continue;
		if (part === '..') parts.pop();
		else parts.push(part);
	}
	return parts.join('/');
}

export function noteDir(notePath: string, vaultPath: string | null): string {
	if (!vaultPath) return '';
	const target = notePath.replace(/\\/g, '/').replace(/\/+$/, '');
	const root = vaultPath.replace(/\\/g, '/').replace(/\/+$/, '');
	const rel = target.startsWith(`${root}/`) ? target.slice(root.length + 1) : target;
	const slash = rel.lastIndexOf('/');
	return slash < 0 ? '' : rel.slice(0, slash);
}

export function vaultRelative(absPath: string, vaultPath: string | null): string | null {
	if (!vaultPath) return null;
	const target = absPath.replace(/\\/g, '/').replace(/\/+$/, '');
	const root = vaultPath.replace(/\\/g, '/').replace(/\/+$/, '');
	return target.startsWith(`${root}/`) ? target.slice(root.length + 1) : null;
}

function fromAbs(abs: string, host: ResolveSources): ResolvedAsset | null {
	const vault = host.vaultPath();
	if (!vault) return null;
	const rel = vaultRelative(abs, vault);
	return rel === null ? null : { abs, rel };
}

function pick(candidates: string[], host: ResolveSources): ResolvedAsset | null {
	const vault = host.vaultPath();
	if (!vault) return null;
	const unique: string[] = [];
	for (const rel of candidates) {
		if (!rel) continue;
		const normalized = normalizeRel(rel);
		if (!normalized || normalized.startsWith('..') || unique.includes(normalized)) continue;
		if (host.exists(normalized)) return fromAbs(`${vault}/${normalized}`, host);
		unique.push(normalized);
	}
	// An ambiguous relative path waits for the file index rather than requesting the first candidate.
	if (unique.length !== 1 && host.hasIndex && !host.hasIndex()) return null;
	return unique.length ? fromAbs(`${vault}/${unique[0]}`, host) : null;
}

export function resolveAsset(src: string, host: ResolveSources): ResolvedAsset | null {
	const raw = src.trim();
	if (!raw || /^(https?:|data:|blob:)/i.test(raw)) return null;
	const vault = host.vaultPath();
	if (!vault) return null;

	if (isLocalfileUrl(raw)) {
		const tail = stripLocalfilePrefix(raw) ?? '';
		return fromAbs(toOsPath(decodeDestination(tail)), host);
	}
	const path = decodeDestination(raw);
	if (path.startsWith('/')) return fromAbs(toOsPath(`${vault}${path}`), host);

	const folder = host.noteDir();
	return pick(
		[folder ? `${folder}/${path}` : '', path, `${host.attachmentFolder()}/${path}`],
		host
	);
}

export function imageSource(src: string, host: ResolveSources): string {
	if (/^(https?:|data:|blob:)/i.test(src.trim())) return src;
	if (isLocalfileUrl(src)) return src;
	const asset = resolveAsset(src, host);
	return asset ? buildLocalfileUrl(asset.abs) : src;
}

export function resolveEmbed(name: string, host: ResolveSources): ResolvedAsset | null {
	const target = name.split('|')[0].split('"')[0].trim();
	if (!target || target.startsWith('#')) return null;
	const direct = pick([`${host.attachmentFolder()}/${target}`, target], host);
	if (direct) return direct;
	if (target.includes('/')) return null;
	const found = host.findByName(target);
	if (!found) return null;
	return fromAbs(`${host.vaultPath()}/${found}`, host);
}

export function embedExtension(name: string): string {
	const clean = name.split('|')[0].trim();
	const dot = clean.lastIndexOf('.');
	return dot < 0 ? '' : clean.slice(dot + 1).toLowerCase();
}
