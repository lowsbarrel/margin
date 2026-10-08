import { replaceInFile, readFileBytes } from '$lib/fs/bridge';
import { flushEditorWrites } from '$lib/fs/write-queue';
import { panes } from '$lib/stores/panes.svelte';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';

// A replaced note that is open keeps its pre-replace text in the editor, so its next save would undo the replacement.
async function refreshOpenTabs(paths: Iterable<string>): Promise<void> {
	const open = new Set(panes.list.flatMap((pane) => pane.tabs.map((tab) => tab.path)));
	for (const path of paths) {
		if (!open.has(path)) continue;
		try {
			panes.applyRestoredContent(path, new TextDecoder().decode(await readFileBytes(path)));
		} catch (err) {
			console.warn(`Could not refresh ${path} after replace:`, err);
		}
	}
}

export async function replaceInOneFile(
	path: string,
	query: string,
	replacement: string
): Promise<boolean> {
	try {
		// Land the debounced edit first: it would otherwise overwrite the replacement.
		await flushEditorWrites();
		const count = await replaceInFile(path, query, replacement, false);
		if (count === 0) return false;
		await refreshOpenTabs([path]);
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
	const replaced: string[] = [];
	await flushEditorWrites();
	for (const path of paths) {
		try {
			const count = await replaceInFile(path, query, replacement, false);
			total += count;
			if (count > 0) replaced.push(path);
		} catch (err) {
			failed += 1;
			console.warn(`Replace in ${path} failed:`, err);
		}
	}
	await refreshOpenTabs(replaced);
	if (total > 0) {
		toast.success(m.toast_replaced_in_files({ count: String(total), files: String(paths.length) }));
	}
	if (failed > 0) toast.error(m.toast_replace_failed_count({ count: failed }));
	return total > 0;
}
