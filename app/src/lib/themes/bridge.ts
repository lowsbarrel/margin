import { commands } from "$lib/bindings";

export type { Theme, ThemeData } from "$lib/bindings";
import type { Theme, ThemeData } from "$lib/bindings";

export async function loadThemes(): Promise<ThemeData> {
  const r = await commands.loadThemes();
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function saveThemes(data: ThemeData): Promise<void> {
  const r = await commands.saveThemes(data);
  if (r.status === "error") throw r.error;
}

export async function exportTheme(theme: Theme, dest: string): Promise<void> {
  const r = await commands.exportTheme(theme, dest);
  if (r.status === "error") throw r.error;
}

export async function importTheme(path: string): Promise<Theme> {
  const r = await commands.importTheme(path);
  if (r.status === "error") throw r.error;
  return r.data;
}
