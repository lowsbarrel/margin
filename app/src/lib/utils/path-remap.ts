/**
 * Where an entry lives after the entry at `from` is renamed or moved to `to`.
 *
 * `isDir` extends the remap to everything beneath the moved folder. Pane tabs,
 * the tree's expanded folders and the tree selection are all keyed by path and
 * must follow the same move, or one of them keeps pointing at a path that no
 * longer exists.
 */
export function remapPath(path: string, from: string, to: string, isDir: boolean): string {
	if (path === from) return to;
	if (isDir && path.startsWith(`${from}/`)) return `${to}${path.slice(from.length)}`;
	return path;
}
