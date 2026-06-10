import { IMAGE_EXTS_ARRAY } from "$lib/utils/mime";
import {
  LOCALFILE_URL_PREFIX,
  LOCALFILE_IMAGE_HOST_RE,
  encodeLocalfileSpaces,
  encodeLocalfileImageSpaces,
  stripLocalfilePrefix,
} from "$lib/editor/image-url";

/** Convert wiki image embeds ![[file.ext]] to standard markdown syntax. */
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
    const safePath = encodeLocalfileSpaces(relPath);
    return `![${filename}](${safePath})`;
  });
}

/** Convert relative image paths in markdown to localfile:// URLs for display */
export function resolveImagePaths(
  md: string,
  vaultPath: string | null,
): string {
  if (!vaultPath) return md;
  // Fix previously-saved malformed localfile:// URLs (Windows bug:
  // localfile://localhostC: → localfile://localhost/C:)
  let fixed = md;
  if (fixed.includes("localfile://localhost") && !fixed.includes("localfile://localhost/")) {
    fixed = fixed.replaceAll("localfile://localhost", "localfile://localhost/");
  }
  // Encode spaces inside any existing localfile image URLs (both scheme forms)
  // so tiptap-markdown and the WebView can parse them.
  fixed = encodeLocalfileImageSpaces(fixed);
  return fixed.replace(
    /!\[([^\]]*)\]\((?!https?:\/\/|data:|localfile:\/\/)([^)]+)\)/g,
    (_match, alt, relPath) => {
      const absPath = `${vaultPath}/${relPath}`;
      const prefix = absPath.startsWith("/") ? "" : "/";
      const encoded = encodeLocalfileSpaces(absPath);
      return `![${alt}](${LOCALFILE_URL_PREFIX}${prefix}${encoded})`;
    },
  );
}

/** Convert localfile image URLs back to vault-relative paths for storage */
export function unresolveImagePaths(
  md: string,
  vaultPath: string | null,
): string {
  if (!vaultPath) return md;
  return md.replace(
    LOCALFILE_IMAGE_HOST_RE,
    (_match, alt: string, fullUrl: string) => {
      const stripped = stripLocalfilePrefix(fullUrl) ?? fullUrl;
      const noLead = stripped.startsWith("/") ? stripped.slice(1) : stripped;
      // Decode %20 → spaces for vault path matching
      const decoded = noLead.replace(/%20/g, " ");
      const normalizedAbs = "/" + decoded;
      const normalizedVault = vaultPath!.startsWith("/") ? vaultPath! : "/" + vaultPath!;
      const relPath = normalizedAbs.startsWith(normalizedVault + "/")
        ? normalizedAbs.substring(normalizedVault!.length + 1)
        : decoded;
      return `![${alt}](${relPath})`;
    },
  );
}
