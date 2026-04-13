import { IMAGE_EXTS } from "$lib/utils/mime";

// ─── Types ───────────────────────────────────────────────────────────────

export type TabType = "markdown" | "image" | "pdf" | "canvas" | "graph" | "unknown";

export interface Tab {
  id: number;
  path: string;
  content: string;
  type: TabType;
  blobUrl?: string;
  pdfData?: Uint8Array;
}

export interface Pane {
  id: number;
  tabs: Tab[];
  activeTabIndex: number;
  externalContentVersion: number;
}

// ─── Counters ────────────────────────────────────────────────────────────

let _nextTabId = 0;
let _nextPaneId = 0;

export function nextTabId(): number {
  return _nextTabId++;
}

export function nextPaneId(): number {
  return _nextPaneId++;
}

// ─── Helpers ─────────────────────────────────────────────────────────────

const PDF_EXTS = new Set([".pdf"]);

export function getTabType(path: string): TabType {
  const ext = path.slice(path.lastIndexOf(".")).toLowerCase();
  if (ext === ".md") return "markdown";
  if (ext === ".canvas") return "canvas";
  if (IMAGE_EXTS.has(ext)) return "image";
  if (PDF_EXTS.has(ext)) return "pdf";
  return "unknown";
}

export function fileTitle(path: string): string {
  if (path === "__graph__") return "Graph";
  const name = path.split("/").pop() ?? "";
  if (name.endsWith(".md")) return name.slice(0, -3);
  if (name.endsWith(".canvas")) return name.slice(0, -7);
  return name;
}

export function toBreadcrumbs(path: string, vaultPath: string | null): string[] {
  if (path === "__graph__") return ["Graph"];
  if (!vaultPath) return [];
  const rel = path.slice(vaultPath.length + 1);
  const parts = rel.split("/");
  return parts.map((p, i) => (i === parts.length - 1 ? fileTitle(path) : p));
}

export function remapPath(
  path: string,
  from: string,
  to: string,
  isDir: boolean,
): string {
  if (path === from) return to;
  if (isDir && path.startsWith(`${from}/`)) {
    return `${to}${path.slice(from.length)}`;
  }
  return path;
}

export function pathMatches(path: string, target: string, isDir: boolean): boolean {
  return path === target || (isDir && path.startsWith(`${target}/`));
}

/** Removes pane at index and gives its flex to its left neighbor (or right if first) */
export function removeFlexAt(flexes: number[], index: number): number[] {
  const removed = flexes[index];
  const next = flexes.filter((_, i) => i !== index);
  const ni = Math.min(index > 0 ? index - 1 : 0, next.length - 1);
  if (next.length > 0) next[ni] += removed;
  return next;
}

/** Revoke blob URLs for the given tabs to prevent memory leaks. */
export function revokeBlobUrls(tabs: Tab[]): void {
  for (const t of tabs) {
    if (t.blobUrl) URL.revokeObjectURL(t.blobUrl);
  }
}

/** Propagate saved content to all OTHER tabs with the same path (cross-pane live sync) */
export function broadcastContent(
  panes: Pane[],
  sourcePaneIndex: number,
  filePath: string,
  content: string,
): void {
  for (let pi = 0; pi < panes.length; pi++) {
    if (pi === sourcePaneIndex) continue;
    for (let ti = 0; ti < panes[pi].tabs.length; ti++) {
      if (panes[pi].tabs[ti].path === filePath) {
        panes[pi].tabs[ti] = { ...panes[pi].tabs[ti], content };
        panes[pi].externalContentVersion++;
      }
    }
  }
}

export function createEmptyPane(): Pane {
  return {
    id: nextPaneId(),
    tabs: [],
    activeTabIndex: -1,
    externalContentVersion: 0,
  };
}
