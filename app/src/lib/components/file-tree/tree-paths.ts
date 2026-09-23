export function parentDir(path: string): string {
	return path.slice(0, path.lastIndexOf('/'));
}
