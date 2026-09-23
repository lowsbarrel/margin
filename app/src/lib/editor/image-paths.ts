import { IMAGE_EXTS_ARRAY } from '$lib/utils/mime';
import {
	buildLocalfileUrl,
	isLocalfileUrl,
	stripLocalfilePrefix,
	LOCALFILE_URL_PREFIX
} from '$lib/editor/image-url';
import { DEFAULT_ATTACHMENT_FOLDER } from '$lib/editor/attachments';

const LEGACY_LOCALFILE_PREFIX = 'localfile://localhost';

const SAFE_DESTINATION = /[A-Za-z0-9\-_.~/]/;

const PERCENT_ESCAPE = /%([0-9A-Fa-f]{2})/g;

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

export function resolveWikiEmbeds(md: string, attachmentFolder: string | null): string {
	const folder = attachmentFolder || DEFAULT_ATTACHMENT_FOLDER;
	let result = '';
	let pos = 0;

	while (pos < md.length) {
		const start = md.indexOf('![[', pos);
		if (start === -1) {
			result += md.slice(pos);
			break;
		}
		result += md.slice(pos, start);

		const bracket = md.indexOf(']', start + 3);
		if (bracket === -1 || md[bracket + 1] !== ']') {
			result += '![[';
			pos = start + 3;
			continue;
		}

		const filename = md.slice(start + 3, bracket);
		const dot = filename.lastIndexOf('.');
		const ext = dot === -1 ? '' : filename.slice(dot + 1).toLowerCase();
		if (IMAGE_EXTS_ARRAY.includes(ext)) {
			const relPath = filename.includes('/') ? filename : `${folder}/${filename}`;
			result += `![${filename}](${encodeDestination(relPath)})`;
		} else {
			result += md.slice(start, bracket + 2);
		}
		pos = bracket + 2;
	}

	return result;
}

function unescapeImageMarkdown(md: string): string {
	if (!md.includes('!\\[')) return md;
	let result = '';
	let pos = 0;

	while (pos < md.length) {
		const start = md.indexOf('!\\[', pos);
		if (start === -1) {
			result += md.slice(pos);
			break;
		}
		result += md.slice(pos, start);
		const altStart = start + 3;

		let i = altStart;
		let altEnd = -1;
		let skip = 1;
		while (i < md.length) {
			if (md[i] === '\\' && md[i + 1] === ']') {
				altEnd = i;
				skip = 2;
				break;
			}
			if (md[i] === ']') {
				altEnd = i;
				break;
			}
			i += md[i] === '\\' ? 2 : 1;
		}

		const close = altEnd === -1 ? -1 : md.indexOf(')', altEnd + skip + 1);
		if (altEnd !== -1 && md[altEnd + skip] === '(' && close !== -1) {
			result += `![${md.slice(altStart, altEnd)}](${md.slice(altEnd + skip + 1, close)})`;
			pos = close + 1;
			continue;
		}

		result += '!\\[';
		pos = altStart;
	}

	return result;
}

function rewriteLegacyLocalfileUrls(md: string): string {
	if (!md.includes(LEGACY_LOCALFILE_PREFIX)) return md;
	return md.replaceAll(LEGACY_LOCALFILE_PREFIX, LOCALFILE_URL_PREFIX);
}

function encodeSpacesInLocalfileUrls(md: string): string {
	if (!md.includes(`](${LEGACY_LOCALFILE_PREFIX}`) && !md.includes(`](${LOCALFILE_URL_PREFIX}`)) {
		return md;
	}
	return mapDestinations(md, (url) => (isLocalfileUrl(url) ? url.replaceAll(' ', '%20') : null));
}

function mapImageLinks(md: string, transform: (alt: string, url: string) => string | null): string {
	let result = '';
	let pos = 0;

	while (pos < md.length) {
		const start = md.indexOf('![', pos);
		if (start === -1) {
			result += md.slice(pos);
			break;
		}

		if (md[start + 2] === '[') {
			result += md.slice(pos, start + 2);
			pos = start + 2;
			continue;
		}

		const bracket = md.indexOf(']', start + 2);
		const urlStart = bracket + 2;
		const close = bracket === -1 ? -1 : md.indexOf(')', urlStart);
		if (close !== -1 && md[bracket + 1] === '(') {
			const replacement = transform(md.slice(start + 2, bracket), md.slice(urlStart, close));
			if (replacement !== null) {
				result += md.slice(pos, start) + replacement;
				pos = close + 1;
				continue;
			}
		}

		result += md.slice(pos, start + 2);
		pos = start + 2;
	}

	return result;
}

function mapDestinations(md: string, transform: (url: string) => string | null): string {
	let result = '';
	let pos = 0;

	while (pos < md.length) {
		const at = md.indexOf('](', pos);
		if (at === -1) {
			result += md.slice(pos);
			break;
		}
		const urlStart = at + 2;
		const close = md.indexOf(')', urlStart);
		if (close === -1) {
			result += md.slice(pos);
			break;
		}

		const replacement = transform(md.slice(urlStart, close));
		result +=
			replacement === null ? md.slice(pos, close + 1) : md.slice(pos, urlStart) + replacement + ')';
		pos = close + 1;
	}

	return result;
}

export function resolveImagePaths(md: string, vaultPath: string | null): string {
	if (!vaultPath) return md;

	const slashed = `${LEGACY_LOCALFILE_PREFIX}/`;
	const repaired =
		md.includes(LEGACY_LOCALFILE_PREFIX) && !md.includes(slashed)
			? md.replaceAll(LEGACY_LOCALFILE_PREFIX, slashed)
			: md;
	const prepared = encodeSpacesInLocalfileUrls(
		rewriteLegacyLocalfileUrls(unescapeImageMarkdown(repaired))
	);

	return mapImageLinks(prepared, (alt, url) => {
		if (url === '' || /^(https?:\/\/|data:|localfile:\/\/)/.test(url)) return null;
		const absPath = url.startsWith('/') ? `${vaultPath}${url}` : `${vaultPath}/${url}`;
		return `![${alt}](${buildLocalfileUrl(absPath)})`;
	});
}

export function unresolveImagePaths(md: string, vaultPath: string | null): string {
	if (!vaultPath) return md;

	const vault = normalizeAbsolute(vaultPath);
	return mapImageLinks(md, (alt, url) => {
		const tail = stripLocalfilePrefix(url);
		if (tail === null) return null;
		const absPath = normalizeAbsolute(decodeDestination(tail));
		const relPath = absPath.startsWith(`${vault}/`)
			? absPath.slice(vault.length + 1)
			: absPath.replace(/^\//, '');
		return `![${alt}](${encodeDestination(relPath)})`;
	});
}

// The localfile URL and the vault root disagree about a leading slash, so both spellings normalise here.
function normalizeAbsolute(path: string): string {
	const withLead = path.startsWith('/') ? path : `/${path}`;
	return withLead.length > 1 ? withLead.replace(/\/+$/, '') : withLead;
}
