import type { MentionItem } from "./mention-command";
import MentionMenu from "$lib/components/MentionMenu.svelte";
import { vault } from "$lib/stores/vault.svelte";
import { walkDirectory } from "$lib/fs/bridge";
import { fuzzyFilterFiles, type FuzzyEntry } from "./text-transform-bridge";
import { createSuggestionRenderer } from "./suggestion-renderer.svelte";

/** Cached markdown file list (5s TTL). */
let cachedFiles: FuzzyEntry[] = [];
let cacheTimestamp = 0;
const CACHE_TTL = 5000; // 5 seconds

async function loadAllFiles(): Promise<void> {
  if (!vault.vaultPath) return;
  const now = Date.now();
  if (cachedFiles.length > 0 && now - cacheTimestamp < CACHE_TTL) return;
  const entries = await walkDirectory(vault.vaultPath);
  cachedFiles = entries
    .filter((e) => !e.is_dir && e.name.endsWith(".md"))
    .map((e) => ({ name: e.name, path: e.path }));
  cacheTimestamp = now;
}

async function filterFiles(query: string): Promise<MentionItem[]> {
  const results = await fuzzyFilterFiles(cachedFiles, query, 20);
  return results.map((r) => ({ title: r.name, path: r.path }));
}

const renderMentionMenu = createSuggestionRenderer<MentionItem>({
  component: MentionMenu,
  loadItems: loadAllFiles,
  getItems: (query) => filterFiles(query),
});

export default renderMentionMenu;
