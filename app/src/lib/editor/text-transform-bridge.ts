import { commands } from '$lib/bindings';

export type { FuzzyEntry } from '$lib/bindings';
import type { FuzzyEntry } from '$lib/bindings';

export async function fuzzyFilterFiles(
	files: FuzzyEntry[],
	query: string,
	limit: number
): Promise<FuzzyEntry[]> {
	return commands.fuzzyFilterFiles(files, query, limit);
}
