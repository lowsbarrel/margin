import { s3Configure, type S3Config } from '$lib/s3/bridge';
import { editor } from '$lib/stores/editor.svelte';
import { files } from '$lib/stores/files.svelte';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';
import { buildManifests } from './s3sync-manifest';
import { planSync } from './s3sync-plan';
import { executeTransfer } from './s3sync-transfer';
import { checkAbort } from './s3sync-abort';
import { saveManifest, syncUploadManifest } from './bridge';

export type ConflictStrategy = 'local_wins' | 'keep_newer';

export interface SyncOptions {
	conflictStrategy?: ConflictStrategy;
}

let activeSyncAbort: AbortController | null = null;
let syncLock: Promise<void> = Promise.resolve();

function cancelSync(): void {
	activeSyncAbort?.abort();
	activeSyncAbort = null;
}

function isSyncing(): boolean {
	return activeSyncAbort !== null;
}

function errorText(err: unknown): string {
	return err instanceof Error ? err.message : String(err);
}

export function syncToS3(
	vaultPath: string,
	vaultId: string,
	encryptionKey: number[],
	s3Config: S3Config,
	options?: SyncOptions
): Promise<void> {
	const ticket = syncLock.then(() =>
		doSyncToS3(vaultPath, vaultId, encryptionKey, s3Config, options)
	);
	// Swallow so a failed sync never rejects the chain the next one waits on.
	syncLock = ticket.catch(() => {});
	return ticket;
}

async function doSyncToS3(
	vaultPath: string,
	vaultId: string,
	encryptionKey: number[],
	s3Config: S3Config,
	options?: SyncOptions
): Promise<void> {
	cancelSync();

	const abortController = new AbortController();
	activeSyncAbort = abortController;
	const { signal } = abortController;
	const conflictStrategy: ConflictStrategy = options?.conflictStrategy ?? 'local_wins';

	editor.setSyncStatus('syncing');

	try {
		await s3Configure(s3Config);
		const s3Prefix = `${vaultId}/`;

		const plan = await planSync(vaultPath, s3Prefix, encryptionKey, signal);
		const actionsTotal = plan.actions.length;
		let actionsDone = 0;
		editor.setSyncProgress(actionsTotal > 0 ? { total: actionsTotal, done: 0 } : null);

		const result = await executeTransfer(plan, {
			vaultPath,
			s3Prefix,
			encryptionKey,
			conflictStrategy,
			signal,
			onProgress: (advance: number) => {
				actionsDone += advance;
				editor.setSyncProgress({ total: actionsTotal, done: actionsDone });
			}
		});
		checkAbort(signal);

		const { local, uploaded } = await buildManifests(
			result.mergedFiles,
			result.tombstones,
			result.missingRemoteBlobs
		);
		await syncUploadManifest(s3Prefix, encryptionKey, uploaded);
		await saveManifest(vaultPath, encryptionKey, local);

		const hadFsChanges = plan.actions.some(
			(a) =>
				a.kind === 'download' ||
				a.kind === 'delete-local' ||
				a.kind === 'conflict' ||
				a.kind === 'conflict-delete-local'
		);
		if (hadFsChanges) {
			await files.refresh(vaultPath);
		}

		editor.setSyncStatus('synced');

		if (result.conflicts.length > 0) {
			toast.info(m.toast_sync_conflicts({ count: String(result.conflicts.length) }));
		}
	} catch (err) {
		if (signal.aborted) {
			editor.setSyncStatus('idle');
			return;
		}
		console.error('[s3sync] Sync failed:', err);
		editor.setSyncStatus('error', errorText(err));
		throw err;
	} finally {
		if (activeSyncAbort === abortController) {
			activeSyncAbort = null;
		}
	}
}

let syncCredentials: {
	vaultPath: string;
	vaultId: string;
	encryptionKey: number[];
	s3Config: S3Config;
	options?: SyncOptions;
} | null = null;

export function setSyncCredentials(
	vaultPath: string,
	vaultId: string,
	encryptionKey: number[],
	s3Config: S3Config,
	options?: SyncOptions
): void {
	syncCredentials = { vaultPath, vaultId, encryptionKey, s3Config, options };
}

export function clearSyncCredentials(): void {
	syncCredentials = null;
}

let autoSyncInterval: ReturnType<typeof setInterval> | null = null;

export function startAutoSync(
	vaultPath: string,
	vaultId: string,
	encryptionKey: number[],
	s3Config: S3Config,
	intervalMs: number = 5 * 60 * 1000,
	options?: SyncOptions
): void {
	stopAutoSync();
	syncCredentials = { vaultPath, vaultId, encryptionKey, s3Config, options };

	runQuietSync();

	autoSyncInterval = setInterval(() => {
		runQuietSync();
	}, intervalMs);
}

export function stopAutoSync(): void {
	if (autoSyncInterval) {
		clearInterval(autoSyncInterval);
		autoSyncInterval = null;
	}
}

async function runQuietSync(): Promise<void> {
	const creds = syncCredentials;
	if (!creds || isSyncing()) return;
	try {
		await syncToS3(
			creds.vaultPath,
			creds.vaultId,
			creds.encryptionKey,
			creds.s3Config,
			creds.options
		);
	} catch {}
}

export async function runManualSync(): Promise<void> {
	const creds = syncCredentials;
	if (!creds) {
		toast.info(m.toast_sync_not_configured());
		return;
	}
	if (isSyncing()) {
		toast.info(m.toast_sync_in_progress());
		return;
	}
	try {
		await syncToS3(
			creds.vaultPath,
			creds.vaultId,
			creds.encryptionKey,
			creds.s3Config,
			creds.options
		);
		toast.success(m.toast_sync_complete());
	} catch (err) {
		toast.error(m.toast_sync_failed({ error: errorText(err) }));
	}
}
