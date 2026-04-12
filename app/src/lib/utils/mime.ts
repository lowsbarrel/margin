/** Image file extensions (without leading dot). */
export const IMAGE_EXTS_ARRAY = [
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "svg",
  "bmp",
];

/** Image file extensions as a Set (with leading dot) for O(1) lookup. */
export const IMAGE_EXTS = new Set([
  ".png",
  ".jpg",
  ".jpeg",
  ".gif",
  ".webp",
  ".svg",
  ".bmp",
  ".ico",
]);

/** Return true if the filename or path refers to a supported image type. */
export function isImagePath(nameOrPath: string): boolean {
  const ext = nameOrPath.split(".").pop()?.toLowerCase() ?? "";
  return IMAGE_EXTS_ARRAY.includes(ext);
}

/** Map a file extension to its MIME type. */
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
