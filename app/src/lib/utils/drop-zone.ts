import { vault } from '$lib/stores/vault.svelte';

export type DropZone =
	| { kind: 'folder'; path: string }
	| { kind: 'parent'; path: string }
	| { kind: 'tree-root' }
	| { kind: 'editor' };

export function hitTestDropZone(x: number, y: number): DropZone | null {
	const el = document.elementFromPoint(x, y);
	const carrier = el?.closest<HTMLElement>('[data-drop-kind]');
	if (!carrier) return null;
	switch (carrier.dataset.dropKind) {
		case 'folder':
			return carrier.dataset.dropPath ? { kind: 'folder', path: carrier.dataset.dropPath } : null;
		case 'file':
			return carrier.dataset.dropParent
				? { kind: 'parent', path: carrier.dataset.dropParent }
				: null;
		case 'tree-root':
			return { kind: 'tree-root' };
		case 'editor':
			return { kind: 'editor' };
		default:
			return null;
	}
}

export function dropDirectory(zone: DropZone | null): string | null {
	if (!zone) return null;
	if (zone.kind === 'folder' || zone.kind === 'parent') return zone.path;
	if (zone.kind === 'tree-root') return vault.vaultPath;
	return null;
}
