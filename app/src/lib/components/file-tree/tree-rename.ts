import { createDirectory, fileExists, type TreeEntry } from '$lib/fs/bridge';
import * as m from '$lib/paraglide/messages.js';
import { editor } from '$lib/stores/editor.svelte';
import { files } from '$lib/stores/files.svelte';
import { toast } from '$lib/stores/toast.svelte';
import { vault } from '$lib/stores/vault.svelte';
import { validateName } from '$lib/utils/filename';
import { useInlineEdit } from '$lib/utils/inline-edit.svelte';

export function selectStem(input: HTMLInputElement) {
	const dot = input.value.lastIndexOf('.');
	input.setSelectionRange(0, dot > 0 ? dot : input.value.length);
}

export function createRenameFlow(options: {
	onrename: (entry: TreeEntry, newName: string) => Promise<boolean>;
}) {
	const { onrename } = options;
	let submitting = false;

	// Enter and blur both land here, and the first commit unmounts the input: the blur it fires must not rename a second time.
	async function submitRename(value: string) {
		const path = files.renamingPath;
		if (!path || submitting) return;
		const entry = files.flatTree.find((row) => row.path === path);
		if (!entry || value === entry.name) {
			files.cancelRename();
			return;
		}
		submitting = true;
		try {
			if (await onrename(entry, value)) files.cancelRename();
		} finally {
			submitting = false;
		}
	}

	async function confirmNewFolder(name: string) {
		const parent = files.pendingNewFolder;
		if (!name.trim() || !parent || !vault.vaultPath) {
			files.cancelNewFolder();
			return;
		}
		const trimmed = name.trim();
		const error = validateName(trimmed);
		if (error) {
			toast.error(error);
			files.cancelNewFolder();
			return;
		}
		const folderPath = `${parent}/${trimmed}`;
		try {
			// `create_dir_all` succeeds silently on an existing folder, so this pre-flight check is the only thing that reports the collision.
			if (await fileExists(folderPath)) {
				toast.error(m.toast_path_exists({ name: trimmed }));
				return;
			}
			await createDirectory(folderPath);
		} catch (err) {
			toast.error(m.toast_create_folder_failed({ error: String(err) }));
			return;
		} finally {
			files.cancelNewFolder();
		}
		await files.expandFolder(folderPath);
		files.setSelectedFolder(folderPath);
		files.requestTreeReveal(folderPath);
		editor.markLocalChange();
	}

	const newFolderEdit = useInlineEdit({
		onSubmit: (value) => void confirmNewFolder(value),
		onCancel: () => files.cancelNewFolder()
	});
	const renameEdit = useInlineEdit({
		onSubmit: (value) => void submitRename(value),
		onCancel: () => files.cancelRename()
	});

	return { newFolderEdit, renameEdit };
}
