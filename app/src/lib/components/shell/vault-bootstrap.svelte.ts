import { untrack } from 'svelte';
import { editor } from '$lib/stores/editor.svelte';
import { files } from '$lib/stores/files.svelte';
import { vault } from '$lib/stores/vault.svelte';
import { hasUnsyncedChanges, sweepUnusedAttachments, watchVault } from '$lib/fs/bridge';
import { loadSettings } from '$lib/settings/bridge';
import { s3Configure } from '$lib/s3/bridge';
import { setSyncCredentials, startAutoSync, type ConflictStrategy } from '$lib/sync/s3sync';
import { DEFAULT_ATTACHMENT_FOLDER, resolveAttachmentFolder } from '$lib/editor/attachments';
import { restoreWorkspace } from './workspace-persistence.svelte';

let folder = $state<string>(DEFAULT_ATTACHMENT_FOLDER);

export const vaultBootstrap = {
	get attachmentFolder(): string {
		return folder;
	}
};

export function initVaultBootstrap(): void {
	$effect(() => {
		if (!vault.isUnlocked || !vault.vaultPath) return;
		const currentVaultPath = vault.vaultPath;
		const currentKey = vault.encryptionKey;
		untrack(() => {
			files
				.refresh(currentVaultPath)
				.catch((err) => console.warn('Failed to load file tree:', err));
			restoreWorkspace();
			watchVault(currentVaultPath).catch((err) =>
				console.warn('Failed to start vault watcher:', err)
			);
			if (currentKey) {
				loadSettings(currentVaultPath, currentKey)
					.then((settings) => {
						if (vault.vaultPath !== currentVaultPath) return;
						if (settings?.s3) {
							s3Configure(settings.s3).catch((err) => console.warn('Failed to configure S3:', err));
							const conflictStrategy: ConflictStrategy =
								settings.conflict_strategy === 'keep_newer' ? 'keep_newer' : 'local_wins';
							if (vault.vaultId && vault.encryptionKey) {
								setSyncCredentials(
									currentVaultPath,
									vault.vaultId,
									vault.encryptionKey,
									settings.s3,
									{ conflictStrategy }
								);
							}
							if (currentKey) {
								hasUnsyncedChanges(currentVaultPath, currentKey)
									.then((pending) => {
										if (vault.vaultPath !== currentVaultPath) return;
										if (editor.syncStatus === 'syncing') return;
										editor.setSyncStatus(pending ? 'idle' : 'synced');
									})
									.catch(() => {
										if (editor.syncStatus === 'syncing') return;
										editor.setSyncStatus('idle');
									});
							}
							if (settings.auto_sync && vault.vaultId && vault.encryptionKey) {
								startAutoSync(
									currentVaultPath,
									vault.vaultId,
									vault.encryptionKey,
									settings.s3,
									undefined,
									{ conflictStrategy }
								);
							}
						} else {
							editor.setSyncStatus('idle');
						}
						folder = resolveAttachmentFolder(settings?.attachment_folder);
						sweepUnusedAttachments(folder).catch((err) =>
							console.warn('Attachment sweep failed:', err)
						);
					})
					.catch((err) => {
						console.warn('Failed to load settings:', err);
					});
			}
		});
	});

	$effect(() => {
		if (!vault.vaultPath) return;
		files.setHiddenPaths([`${vault.vaultPath}/${folder}`]);
	});
}
