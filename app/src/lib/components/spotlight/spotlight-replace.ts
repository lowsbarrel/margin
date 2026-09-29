import { replaceInFile } from '$lib/fs/bridge';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';

export async function replaceInOneFile(
	path: string,
	query: string,
	replacement: string
): Promise<boolean> {
	try {
		const count = await replaceInFile(path, query, replacement, false);
		if (count === 0) return false;
		toast.success(m.toast_replaced({ count: String(count) }));
		return true;
	} catch (err) {
		toast.error(m.toast_replace_failed({ error: String(err) }));
		return false;
	}
}

export async function replaceInEveryFile(
	paths: string[],
	query: string,
	replacement: string
): Promise<boolean> {
	let total = 0;
	let failed = 0;
	for (const path of paths) {
		try {
			total += await replaceInFile(path, query, replacement, false);
		} catch (err) {
			failed += 1;
			console.warn(`Replace in ${path} failed:`, err);
		}
	}
	if (total > 0) {
		toast.success(m.toast_replaced_in_files({ count: String(total), files: String(paths.length) }));
	}
	if (failed > 0) toast.error(m.toast_replace_failed_count({ count: failed }));
	return total > 0;
}
