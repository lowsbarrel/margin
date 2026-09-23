import { IS_ANDROID, IS_WINDOWS } from '$lib/utils/platform';

// Tauri 2 serves custom schemes as `http://<scheme>.localhost` on Windows/Android; mirrors the `localfile` protocol in lib.rs and the CSP in tauri.conf.json.
export const LOCALFILE_URL_PREFIX =
	IS_WINDOWS || IS_ANDROID ? 'http://localfile.localhost' : 'localfile://localhost';

const LEGACY_PREFIXES = ['http://localfile.localhost', 'localfile://localhost'];

export function isLocalfileUrl(url: string): boolean {
	return LEGACY_PREFIXES.some((p) => url.startsWith(p));
}

export function stripLocalfilePrefix(url: string): string | null {
	for (const p of LEGACY_PREFIXES) {
		if (url.startsWith(p)) return url.slice(p.length);
	}
	return null;
}

// A URL stripped on Windows yields `/C:/Users/…`, which the shell rejects with os error 123.
export function toOsPath(path: string): string {
	const m = path.match(/^\/?([A-Za-z]:[\\/].*)$/);
	if (m) return m[1].replace(/\//g, '\\');
	return path;
}

export function encodeLocalfileSpaces(path: string): string {
	return path.replace(/ /g, '%20');
}

export const LOCALFILE_IMAGE_RE =
	/!\[([^\]]*)\]\(((?:localfile:\/\/|http:\/\/localfile\.localhost)[^)]+)\)/g;

export function encodeLocalfileImageSpaces(md: string): string {
	return md.replace(
		LOCALFILE_IMAGE_RE,
		(_m, alt: string, url: string) => `![${alt}](${encodeLocalfileSpaces(url)})`
	);
}

export function buildLocalfileUrl(absPath: string): string {
	const prefix = absPath.startsWith('/') ? '' : '/';
	return `${LOCALFILE_URL_PREFIX}${prefix}${encodeLocalfileSpaces(absPath)}`;
}
