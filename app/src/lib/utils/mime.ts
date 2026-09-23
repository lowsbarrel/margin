/**
 * Single source of truth for recognized image extensions (no leading dot): it
 * decides whether a dropped or pasted file becomes an image node or a file
 * embed. `mime_from_ext` in `src-tauri/src/lib.rs` serves these types.
 */
export const IMAGE_EXTS_ARRAY = [
	'png',
	'jpg',
	'jpeg',
	'gif',
	'webp',
	'svg',
	'bmp',
	'avif',
	'ico',
	'tiff',
	'tif'
];

/** Dotted form of {@link IMAGE_EXTS_ARRAY}; kept in sync from the same source. */
export const IMAGE_EXTS = new Set(IMAGE_EXTS_ARRAY.map((ext) => `.${ext}`));

export function isImagePath(nameOrPath: string): boolean {
	const ext = nameOrPath.split('.').pop()?.toLowerCase() ?? '';
	return IMAGE_EXTS_ARRAY.includes(ext);
}

export function mimeForPath(path: string): string {
	const ext = path.slice(path.lastIndexOf('.')).toLowerCase();
	const map: Record<string, string> = {
		'.png': 'image/png',
		'.jpg': 'image/jpeg',
		'.jpeg': 'image/jpeg',
		'.gif': 'image/gif',
		'.webp': 'image/webp',
		'.svg': 'image/svg+xml',
		'.bmp': 'image/bmp',
		'.avif': 'image/avif',
		'.ico': 'image/x-icon',
		'.tiff': 'image/tiff',
		'.tif': 'image/tiff',
		'.pdf': 'application/pdf'
	};
	return map[ext] ?? 'application/octet-stream';
}

export function getExt(nameOrPath: string): string {
	return nameOrPath.split('.').pop()?.toLowerCase() ?? '';
}

export function isImageFile(name: string, mimeType?: string): boolean {
	if (mimeType) return mimeType.startsWith('image/');
	return IMAGE_EXTS_ARRAY.includes(getExt(name));
}

/**
 * File name for a display source. Image viewers are handed a URL — a blob: from
 * a tab, a `localfile:` URL from the editor — so the last path segment is
 * decoded rather than read off a path. Sources that carry no name (data: URLs,
 * blob: handles) yield an empty string for the caller's own fallback.
 */
export function fileNameFromSrc(src: string): string {
	if (src.startsWith('data:') || src.startsWith('blob:')) return '';
	const tail = src.split(/[?#]/)[0].split('/').pop() ?? '';
	try {
		return decodeURIComponent(tail);
	} catch {
		return tail;
	}
}
