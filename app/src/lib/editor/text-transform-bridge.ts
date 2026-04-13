import { invoke } from "@tauri-apps/api/core";

export interface FuzzyEntry {
  name: string;
  path: string;
}

/** Fuzzy-filter files by name in Rust (much faster for large vaults). */
export async function fuzzyFilterFiles(
  files: FuzzyEntry[],
  query: string,
  limit: number,
): Promise<FuzzyEntry[]> {
  return invoke<FuzzyEntry[]>("fuzzy_filter_files", { files, query, limit });
}

/**
 * Transform image paths in markdown content (resolve or unresolve).
 * Mode "resolve": wiki embeds → standard md images, relative → localfile:// URLs
 * Mode "unresolve": localfile:// URLs → relative paths
 */
export async function transformImagePaths(
  markdown: string,
  vaultPath: string | null,
  attachmentFolder: string | null,
  mode: "resolve" | "unresolve",
): Promise<string> {
  return invoke<string>("transform_image_paths", {
    markdown,
    vaultPath,
    attachmentFolder,
    mode,
  });
}
