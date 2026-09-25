import { fileTitle } from '$lib/stores/panes.svelte';
import { baseName } from '$lib/utils/path';

export interface Breadcrumb {
	label: string;
	path: string;
	isDir: boolean;
}

export function vaultBreadcrumbs(path: string, vaultPath: string | null): Breadcrumb[] {
	if (!vaultPath || !path.startsWith(`${vaultPath}/`)) return [];
	const parts = path.slice(vaultPath.length + 1).split('/');
	const crumbs: Breadcrumb[] = [
		{ label: baseName(vaultPath) || vaultPath, path: vaultPath, isDir: true }
	];
	let current = vaultPath;
	for (const part of parts.slice(0, -1)) {
		current += `/${part}`;
		crumbs.push({ label: part, path: current, isDir: true });
	}
	crumbs.push({ label: fileTitle(path), path, isDir: false });
	return crumbs;
}
