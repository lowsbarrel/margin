import { decryptBlob } from '$lib/crypto/bridge';
import { s3Download } from '$lib/s3/bridge';
import {
	createDirectory,
	deleteEntry,
	readFileBytes,
	setMtime,
	writeFileBytes
} from '$lib/fs/bridge';
import { nowSeconds, type ManifestEntry } from './s3sync-manifest';
import { checkAbort } from './s3sync-abort';
import type { SyncPlan } from './s3sync-plan';
import type { ConflictStrategy } from './s3sync';
import {
	collectTombstonesNative,
	mergeTombstonesNative,
	pathToS3Key,
	syncDeleteFiles,
	syncDownloadFiles,
	syncUploadFiles,
	type SyncAction
} from './bridge';

export interface TransferContext {
	vaultPath: string;
	s3Prefix: string;
	encryptionKey: number[];
	conflictStrategy: ConflictStrategy;
	signal: AbortSignal;
	onProgress: (advance: number) => void;
}

export interface TransferResult {
	mergedFiles: Map<string, ManifestEntry>;
	tombstones: Map<string, ManifestEntry>;
	missingRemoteBlobs: Map<string, ManifestEntry>;
	conflicts: string[];
}

let conflictCounter = 0;

function conflictCopyName(path: string): string {
	const now = new Date();
	const ts = [
		now.getFullYear(),
		String(now.getMonth() + 1).padStart(2, '0'),
		String(now.getDate()).padStart(2, '0'),
		'-',
		String(now.getHours()).padStart(2, '0'),
		String(now.getMinutes()).padStart(2, '0'),
		String(now.getSeconds()).padStart(2, '0')
	].join('');
	const seq = ++conflictCounter;
	const suffix = `sync-conflict-${ts}-${seq}`;
	const dot = path.lastIndexOf('.');
	if (dot > 0) return `${path.slice(0, dot)}.${suffix}${path.slice(dot)}`;
	return `${path}.${suffix}`;
}

async function ensureParentDir(vaultPath: string, relativePath: string): Promise<void> {
	const parts = relativePath.split('/');
	if (parts.length > 1) {
		await createDirectory(`${vaultPath}/${parts.slice(0, -1).join('/')}`);
	}
}

async function writeConflictCopy(
	vaultPath: string,
	path: string,
	content: Uint8Array,
	hash: string,
	mergedFiles: Map<string, ManifestEntry>,
	s3Prefix: string,
	encryptionKey: number[]
): Promise<string> {
	const conflictPath = conflictCopyName(path);
	const modified = nowSeconds();

	await ensureParentDir(vaultPath, conflictPath);
	await writeFileBytes(`${vaultPath}/${conflictPath}`, content);
	await setMtime(`${vaultPath}/${conflictPath}`, modified);
	await syncUploadFiles(vaultPath, s3Prefix, [conflictPath], encryptionKey);

	mergedFiles.set(conflictPath, {
		path: conflictPath,
		hash,
		modified
	});

	return conflictPath;
}

function markTombstone(
	path: string,
	tombstones: Map<string, ManifestEntry>,
	mergedFiles: Map<string, ManifestEntry>,
	fallbackMaps: Map<string, ManifestEntry>[]
): void {
	const existing =
		mergedFiles.get(path) ??
		fallbackMaps.reduce<ManifestEntry | undefined>((acc, m) => acc ?? m.get(path), undefined);
	if (existing) {
		tombstones.set(path, { ...existing, deleted_at: nowSeconds() });
	}
	mergedFiles.delete(path);
}

export async function executeTransfer(
	plan: SyncPlan,
	ctx: TransferContext
): Promise<TransferResult> {
	const { vaultPath, s3Prefix, encryptionKey, conflictStrategy, signal, onProgress } = ctx;
	const { baseManifest, localManifest, remoteManifest, baseMap, localMap, remoteMap, actions } =
		plan;

	const mergedFiles = new Map<string, ManifestEntry>();
	for (const entry of localManifest.files) mergedFiles.set(entry.path, entry);

	const tombstones = new Map(
		(
			await mergeTombstonesNative(
				await collectTombstonesNative(baseManifest.files),
				await collectTombstonesNative(remoteManifest.files)
			)
		).map((e) => [e.path, e] as const)
	);

	const conflicts: string[] = [];
	const missingRemoteBlobs = new Map<string, ManifestEntry>();

	const uploadPaths: string[] = [];
	const downloadPaths: string[] = [];
	const downloadMtimes: number[] = [];
	const deleteRemotePaths: string[] = [];
	const deleteLocalPaths: string[] = [];
	const conflictActions: SyncAction[] = [];

	for (const action of actions) {
		switch (action.kind) {
			case 'upload':
			case 'conflict-delete-remote':
				uploadPaths.push(action.path);
				break;
			case 'download':
			case 'conflict-delete-local': {
				const remoteEntry = remoteMap.get(action.path)!;
				downloadPaths.push(action.path);
				downloadMtimes.push(remoteEntry.modified);
				break;
			}
			case 'delete-remote':
				deleteRemotePaths.push(action.path);
				break;
			case 'delete-local':
				deleteLocalPaths.push(action.path);
				break;
			case 'conflict':
				conflictActions.push(action);
				break;
			default: {
				const _exhaustive: never = action.kind;
				console.error('[s3sync] Unhandled sync action kind:', _exhaustive);
			}
		}
	}

	if (uploadPaths.length > 0) {
		checkAbort(signal);
		await syncUploadFiles(vaultPath, s3Prefix, uploadPaths, encryptionKey);
		for (const path of uploadPaths) tombstones.delete(path);
		onProgress(uploadPaths.length);
	}

	if (downloadPaths.length > 0) {
		checkAbort(signal);
		const skipped = await syncDownloadFiles(
			vaultPath,
			s3Prefix,
			downloadPaths,
			downloadMtimes,
			encryptionKey
		);
		const skippedSet = new Set(skipped);
		for (let i = 0; i < downloadPaths.length; i++) {
			const path = downloadPaths[i];
			const remoteEntry = remoteMap.get(path)!;
			if (skippedSet.has(path)) {
				missingRemoteBlobs.set(path, remoteEntry);
				continue;
			}
			mergedFiles.set(path, {
				path: remoteEntry.path,
				hash: remoteEntry.hash,
				modified: remoteEntry.modified
			});
			tombstones.delete(path);
		}
		if (skipped.length > 0) {
			console.warn('[s3sync] Skipped files whose blob is missing on S3:', skipped);
		}
		onProgress(downloadPaths.length);
	}

	if (deleteRemotePaths.length > 0) {
		checkAbort(signal);
		try {
			await syncDeleteFiles(s3Prefix, deleteRemotePaths, encryptionKey);
		} catch {}
		for (const path of deleteRemotePaths) {
			markTombstone(path, tombstones, mergedFiles, [baseMap]);
		}
		onProgress(deleteRemotePaths.length);
	}

	for (const path of deleteLocalPaths) {
		checkAbort(signal);
		try {
			await deleteEntry(`${vaultPath}/${path}`);
		} catch {}
		markTombstone(path, tombstones, mergedFiles, [remoteMap]);
		onProgress(1);
	}

	for (const action of conflictActions) {
		checkAbort(signal);
		const localEntry = localMap.get(action.path)!;
		const remoteEntry = remoteMap.get(action.path)!;

		const remoteWins =
			conflictStrategy === 'keep_newer' && remoteEntry.modified > localEntry.modified;

		if (remoteWins) {
			const localData = await readFileBytes(`${vaultPath}/${action.path}`);
			await writeConflictCopy(
				vaultPath,
				action.path,
				localData,
				localEntry.hash,
				mergedFiles,
				s3Prefix,
				encryptionKey
			);

			const skipped = await syncDownloadFiles(
				vaultPath,
				s3Prefix,
				[action.path],
				[remoteEntry.modified],
				encryptionKey
			);

			if (skipped.includes(action.path)) {
				missingRemoteBlobs.set(action.path, remoteEntry);
				console.warn('[s3sync] Conflict: remote blob missing, kept local:', action.path);
			} else {
				mergedFiles.set(action.path, {
					path: remoteEntry.path,
					hash: remoteEntry.hash,
					modified: remoteEntry.modified
				});
			}
		} else {
			const s3Key = await pathToS3Key(action.path, encryptionKey);
			const encrypted = await s3Download(`${s3Prefix}files/${s3Key}.enc`);
			const decrypted = await decryptBlob(encrypted, encryptionKey);
			await writeConflictCopy(
				vaultPath,
				action.path,
				decrypted,
				remoteEntry.hash,
				mergedFiles,
				s3Prefix,
				encryptionKey
			);

			await syncUploadFiles(vaultPath, s3Prefix, [action.path], encryptionKey);
		}

		tombstones.delete(action.path);
		conflicts.push(action.path);
		onProgress(1);
	}

	for (const action of actions) {
		if (action.kind === 'conflict-delete-local' || action.kind === 'conflict-delete-remote') {
			conflicts.push(action.path);
		}
	}

	return { mergedFiles, tombstones, missingRemoteBlobs, conflicts };
}
