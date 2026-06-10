/** Single source of truth for recognized image extensions (no leading dot). */
export const IMAGE_EXTS_ARRAY = [
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "svg",
  "bmp",
  "ico",
];

/** Dotted form of {@link IMAGE_EXTS_ARRAY}; kept in sync from the same source. */
export const IMAGE_EXTS = new Set(IMAGE_EXTS_ARRAY.map((ext) => `.${ext}`));

export function isImagePath(nameOrPath: string): boolean {
  const ext = nameOrPath.split(".").pop()?.toLowerCase() ?? "";
  return IMAGE_EXTS_ARRAY.includes(ext);
}

export function mimeForPath(path: string): string {
  const ext = path.slice(path.lastIndexOf(".")).toLowerCase();
  const map: Record<string, string> = {
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".gif": "image/gif",
    ".webp": "image/webp",
    ".svg": "image/svg+xml",
    ".bmp": "image/bmp",
    ".ico": "image/x-icon",
    ".pdf": "application/pdf",
  };
  return map[ext] ?? "application/octet-stream";
}

export function getExt(nameOrPath: string): string {
  return nameOrPath.split(".").pop()?.toLowerCase() ?? "";
}

export function isImageFile(name: string, mimeType?: string): boolean {
  if (mimeType) return mimeType.startsWith("image/");
  return IMAGE_EXTS_ARRAY.includes(getExt(name));
}
