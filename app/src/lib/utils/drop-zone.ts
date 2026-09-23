import { vault } from '$lib/stores/vault.svelte';

export type DropZone =
	| { kind: 'folder'; path: string }
	| { kind: 'parent'; path: string }
	| { kind: 'tree-root' }
	| { kind: 'editor' };

/**
 * Where a drop at viewport point `(x, y)` lands, read off the `data-drop-*`
 * attributes the tree rows and the editor carry.
 *
 * Both drag paths resolve their target with this: the in-app pointer drag at
 * mouseup, and the OS drag-drop router at every enter/over/drop. Hit-testing at
 * release — instead of tracking mouseenter/mouseleave per row — is what makes a
 * file row land in its parent folder rather than in the vault root, and what
 * lets one resolver serve a drop the pointer never announced an interest in.
 */
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

/** The vault directory a drop zone imports/moves into, or null when it is not a tree target. */
export function dropDirectory(zone: DropZone | null): string | null {
	if (!zone) return null;
	if (zone.kind === 'folder' || zone.kind === 'parent') return zone.path;
	if (zone.kind === 'tree-root') return vault.vaultPath;
	return null;
}
