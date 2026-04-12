import { invoke } from "@tauri-apps/api/core";

export interface Theme {
  name: string;
  colors: Record<string, string>;
}

export interface ThemeData {
  themes: Theme[];
  active_theme: string | null;
}

export async function loadThemes(): Promise<ThemeData> {
  return invoke<ThemeData>("load_themes");
}

export async function saveThemes(data: ThemeData): Promise<void> {
  return invoke("save_themes", { data });
}

export async function exportTheme(theme: Theme, dest: string): Promise<void> {
  return invoke("export_theme", { theme, dest });
}

export async function importTheme(path: string): Promise<Theme> {
  return invoke<Theme>("import_theme", { path });
}
