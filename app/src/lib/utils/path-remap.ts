export function remapPath(path: string, from: string, to: string, isDir: boolean): string {
	if (path === from) return to;
	if (isDir && path.startsWith(`${from}/`)) return `${to}${path.slice(from.length)}`;
	return path;
}
