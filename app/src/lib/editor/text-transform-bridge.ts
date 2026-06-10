import { commands } from "$lib/bindings";

export type { FuzzyEntry } from "$lib/bindings";
import type { FuzzyEntry } from "$lib/bindings";

/** Fuzzy-filter files by name in Rust (much faster for large vaults). */
export async function fuzzyFilterFiles(
  files: FuzzyEntry[],
  query: string,
  limit: number,
): Promise<FuzzyEntry[]> {
  return commands.fuzzyFilterFiles(files, query, limit);
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
  return commands.transformImagePaths(
    markdown,
    vaultPath,
    attachmentFolder,
    mode,
  );
}
