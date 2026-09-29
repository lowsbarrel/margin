import { loadManifest, computeSyncActionsNative, type SyncAction } from './bridge';
import type { ManifestEntry, Manifest } from './s3sync-manifest';
import {
	buildLocalManifest,
	isMissingRemoteManifestError,
	loadRemoteManifest
} from './s3sync-manifest';
import { checkAbort } from './s3sync-abort';

export interface SyncPlan {
	baseManifest: Manifest;
	localManifest: Manifest;
	remoteManifest: Manifest;
	baseMap: Map<string, ManifestEntry>;
	localMap: Map<string, ManifestEntry>;
	remoteMap: Map<string, ManifestEntry>;
	actions: SyncAction[];
}

export async function planSync(
	vaultPath: string,
	s3Prefix: string,
	encryptionKey: number[],
	signal: AbortSignal
): Promise<SyncPlan> {
	checkAbort(signal);
	const baseManifest = await loadManifest(vaultPath, encryptionKey);
	const baseMap = new Map(baseManifest.files.map((e) => [e.path, e]));

	checkAbort(signal);
	const localManifest = await buildLocalManifest(vaultPath, baseMap);
	checkAbort(signal);

	let remoteManifest: Manifest = { version: 3, files: [] };
	try {
		checkAbort(signal);
		const loaded = await loadRemoteManifest(s3Prefix, encryptionKey);
		if (loaded) remoteManifest = loaded;
	} catch (err) {
		if (signal.aborted) throw new Error('Sync cancelled', { cause: err });
		// An unreadable remote manifest with a non-empty base would wipe every file.
		if (!isMissingRemoteManifestError(err) || baseManifest.files.length > 0) {
			throw new Error(`Failed to download remote manifest: ${err}`, { cause: err });
		}
	}

	const localMap = new Map(localManifest.files.map((e) => [e.path, e]));
	const remoteMap = new Map(remoteManifest.files.map((e) => [e.path, e]));
	const actions = await computeSyncActionsNative(
		baseManifest.files,
		localManifest.files,
		remoteManifest.files
	);
	checkAbort(signal);

	return { baseManifest, localManifest, remoteManifest, baseMap, localMap, remoteMap, actions };
}
