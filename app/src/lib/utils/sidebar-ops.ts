import { fileExists } from "$lib/fs/bridge";
import { validateName } from "$lib/utils/filename";
import { toast } from "$lib/stores/toast.svelte";

export function normalizeFileName(input: string): string | null {
  const name = input.trim();
  const error = validateName(name);
  if (error) {
    toast.error(error);
    return null;
  }
  return name.includes(".") ? name : `${name}.md`;
}

export function normalizeDirName(input: string): string | null {
  const name = input.trim();
  const error = validateName(name);
  if (error) {
    toast.error(error);
    return null;
  }
  return name;
}

export async function createUniqueFilePath(
  base: string,
  desiredName?: string,
): Promise<string | null> {
  let name = desiredName ? normalizeFileName(desiredName) : "Untitled.md";
  if (!name) return null;

  const extIndex = name.lastIndexOf(".");
  const stem = extIndex > 0 ? name.slice(0, extIndex) : name;
  const ext = extIndex > 0 ? name.slice(extIndex) : ".md";
  let candidate = `${base}/${name}`;
  let i = 1;
  while (await fileExists(candidate)) {
    candidate = `${base}/${stem} ${i}${ext}`;
    i++;
  }
  return candidate;
}

export function displayPath(fullPath: string, vaultPath: string | null): string {
  if (!vaultPath) return fullPath;
  const rel = fullPath.slice(vaultPath.length + 1);
  const parts = rel.split("/");
  if (parts.length <= 1) return "";
  return parts.slice(0, -1).join("/");
}

export function displayName(name: string): string {
  if (name.endsWith(".md")) return name.slice(0, -3);
  if (name.endsWith(".canvas")) return name.slice(0, -7);
  return name;
}

export function highlightMatch(
  context: string,
  query: string,
  isCaseSensitive: boolean,
): string {
  if (!query) return escapeHtml(context);
  const flags = isCaseSensitive ? "g" : "gi";
  return escapeHtml(context).replace(
    new RegExp(escapeRegex(escapeHtml(query)), flags),
    '<strong class="search-highlight">$&</strong>',
  );
}

export function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

export function escapeRegex(str: string): string {
  return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
