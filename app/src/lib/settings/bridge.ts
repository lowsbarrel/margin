import { commands, type AppSettings } from '$lib/bindings';

export type { AppSettings };

export async function saveSettings(vaultPath: string, settings: AppSettings): Promise<void> {
	const r = await commands.saveSettings(vaultPath, settings);
	if (r.status === 'error') throw r.error;
}

export async function loadSettings(vaultPath: string): Promise<AppSettings | null> {
	const r = await commands.loadSettings(vaultPath);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function exportSettingsString(settings: AppSettings): Promise<string> {
	const r = await commands.exportSettingsString(settings);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function importSettingsString(encoded: string): Promise<AppSettings> {
	const r = await commands.importSettingsString(encoded);
	if (r.status === 'error') throw r.error;
	return r.data;
}
