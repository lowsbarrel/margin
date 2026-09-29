let pending: string | null = null;

// Deliberately not reactive: the reader runs in an effect keyed off the pane's path.
export const noteFocus = {
	request(path: string): void {
		pending = path;
	},

	take(path: string): boolean {
		if (pending !== path) return false;
		pending = null;
		return true;
	}
};
