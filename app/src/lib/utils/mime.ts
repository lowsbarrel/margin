// Mirrored by `mime_from_ext` in src-tauri/src/lib.rs; the two lists must stay in step.
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

export function fileNameFromSrc(src: string): string {
	if (src.startsWith('data:') || src.startsWith('blob:')) return '';
	const tail = src.split(/[?#]/)[0].split('/').pop() ?? '';
	try {
		return decodeURIComponent(tail);
	} catch {
		return tail;
	}
}
