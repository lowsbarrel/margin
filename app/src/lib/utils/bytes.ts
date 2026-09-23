const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

/** Human-readable byte size, e.g. `2.4 MB`. */
export function formatBytes(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	let value = bytes;
	let unit = 0;
	while (value >= 1024 && unit < UNITS.length - 1) {
		value /= 1024;
		unit++;
	}
	return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${UNITS[unit]}`;
}
