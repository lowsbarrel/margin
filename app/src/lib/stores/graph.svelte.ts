import { vault } from "$lib/stores/vault.svelte";
import {
  readFileBytes,
  writeFileBytes,
  walkDirectory,
  readLinkBatch,
} from "$lib/fs/bridge";

export interface GraphNode {
  id: string;
  label: string;
}

export interface GraphEdge {
  source: string;
  target: string;
}

export interface GraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

interface GraphState {
  data: GraphData;
  loading: boolean;
  nodeToPath: Map<string, string>;
}

let state = $state<GraphState>({
  data: { nodes: [], edges: [] },
  loading: false,
  nodeToPath: new Map(),
});

// Per-file {mtime, links[]} cache at .margin/graph-cache.json.
// Only stale files are re-read from disk.

interface CacheEntry {
  mtime: number;
  links: string[];
}

interface LinkCache {
  version: number;
  entries: Record<string, CacheEntry>;
}

const CACHE_VERSION = 1;

async function loadLinkCache(vaultPath: string): Promise<LinkCache> {
  const path = `${vaultPath}/.margin/graph-cache.json`;
  try {
    const bytes = await readFileBytes(path);
    const parsed = JSON.parse(new TextDecoder().decode(bytes)) as LinkCache;
    if (
      parsed?.version === CACHE_VERSION &&
      typeof parsed.entries === "object"
    ) {
      return parsed;
    }
  } catch { /* ignore */ }
  return { version: CACHE_VERSION, entries: {} };
}

async function saveLinkCache(
  vaultPath: string,
  cache: LinkCache,
): Promise<void> {
  const path = `${vaultPath}/.margin/graph-cache.json`;
  try {
    const bytes = new TextEncoder().encode(JSON.stringify(cache));
    await writeFileBytes(path, bytes);
  } catch { /* non-fatal */ }
}

function fileNameToId(name: string): string {
  return name.endsWith(".md") ? name.slice(0, -3) : name;
}

export const graph = {
  get data() {
    return state.data;
  },
  get loading() {
    return state.loading;
  },
  get nodeToPath() {
    return state.nodeToPath;
  },

  async build() {
    const vaultPath = vault.vaultPath;
    if (!vaultPath) return;
    if (state.loading) return;

    state.loading = true;
    try {
      const allEntries = await walkDirectory(vaultPath);
      const mdFiles = allEntries.filter(
        (e) => !e.is_dir && e.name.endsWith(".md"),
      );

      const titleToAbs = new Map<string, string>();
      for (const f of mdFiles) {
        titleToAbs.set(fileNameToId(f.name).toLowerCase(), f.path);
      }

      const cache = await loadLinkCache(vaultPath);
      let cacheUpdated = false;

      const nodeSet = new Set<string>();
      const edges: GraphEdge[] = [];

      const stale: typeof mdFiles = [];
      for (const f of mdFiles) {
        const sourceId = fileNameToId(f.name);
        nodeSet.add(sourceId);
        const cached = cache.entries[f.path];
        if (cached && cached.mtime === f.modified) {
          for (const link of cached.links) {
            nodeSet.add(link);
            edges.push({ source: sourceId, target: link });
          }
        } else {
          stale.push(f);
        }
      }

      if (stale.length > 0) {
        const stalePaths = stale.map((f) => f.path);
        const freshEntries = await readLinkBatch(stalePaths);
        for (const { path, links } of freshEntries) {
          const f = stale.find((sf) => sf.path === path);
          const sourceId = f
            ? fileNameToId(f.name)
            : fileNameToId(path.split("/").pop() ?? path);
          for (const link of links) {
            nodeSet.add(link);
            edges.push({ source: sourceId, target: link });
          }
          cache.entries[path] = { mtime: f?.modified ?? 0, links };
          cacheUpdated = true;
        }
      }

      const existingPaths = new Set(mdFiles.map((f) => f.path));
      for (const p of Object.keys(cache.entries)) {
        if (!existingPaths.has(p)) {
          delete cache.entries[p];
          cacheUpdated = true;
        }
      }

      if (cacheUpdated) {
        saveLinkCache(vaultPath, cache); // fire-and-forget
      }

      const nodes: GraphNode[] = [...nodeSet].map((id) => ({ id, label: id }));
      state.data = { nodes, edges };
      state.nodeToPath = titleToAbs;
    } finally {
      state.loading = false;
    }
  },

  clear() {
    state.data = { nodes: [], edges: [] };
    state.nodeToPath = new Map();
  },
};
