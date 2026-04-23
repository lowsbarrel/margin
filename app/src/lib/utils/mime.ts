export const IMAGE_EXTS_ARRAY = [
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "svg",
  "bmp",
];

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
