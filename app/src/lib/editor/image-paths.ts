import { IMAGE_EXTS_ARRAY } from "$lib/utils/mime";

/**
 * Convert Obsidian-style wiki image embeds ![[filename.ext]] to standard
 * markdown image syntax so that resolveImagePaths can handle them.
 * Only handles image file extensions; non-image embeds are left for the
 * FileEmbed markdown-it plugin.
 */
export function resolveWikiEmbeds(
  md: string,
  attachmentFolder: string | null,
): string {
  const folder = attachmentFolder || "attachments";
  return md.replace(/!\[\[([^\]]+)\]\]/g, (match, filename: string) => {
    if (!filename.includes(".")) return match; // note link, not file embed
    const ext = filename.split(".").pop()?.toLowerCase() ?? "";
    if (!IMAGE_EXTS_ARRAY.includes(ext)) return match; // handled by FileEmbed plugin
    const relPath = filename.includes("/") ? filename : `${folder}/${filename}`;
    const safePath = relPath.replace(/ /g, "%20");
    return `![${filename}](${safePath})`;
  });
}

/** Convert relative image paths in markdown to localfile:// URLs for display */
export function resolveImagePaths(
  md: string,
  vaultPath: string | null,
): string {
  if (!vaultPath) return md;
  return md.replace(
    /!\[([^\]]*)\]\((?!https?:\/\/|data:|localfile:\/\/)([^)]+)\)/g,
    (_match, alt, relPath) => {
      const absPath = `${vaultPath}/${relPath}`;
      const prefix = absPath.startsWith("/") ? "" : "/";
      return `![${alt}](localfile://localhost${prefix}${absPath})`;
    },
  );
}

/** Convert localfile:// URLs back to vault-relative paths for storage */
export function unresolveImagePaths(
  md: string,
  vaultPath: string | null,
): string {
  if (!vaultPath) return md;
  return md.replace(
    /!\[([^\]]*)\]\(localfile:\/\/localhost\/?([^)]+)\)/g,
    (_match, alt, absPath) => {
      const normalizedAbs = absPath.startsWith("/") ? absPath : "/" + absPath;
      const normalizedVault = vaultPath.startsWith("/") ? vaultPath : "/" + vaultPath;
      const relPath = normalizedAbs.startsWith(normalizedVault + "/")
        ? normalizedAbs.substring(normalizedVault.length + 1)
        : absPath;
      return `![${alt}](${relPath})`;
    },
  );
}
