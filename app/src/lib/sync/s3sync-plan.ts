import {
	loadManifest,
	computeSyncActionsNative,
	syncLoadRemoteManifest,
	type SyncAction
} from './bridge';
import type { ManifestEntry, Manifest } from './s3sync-manifest';
import { buildLocalManifest, nowSeconds } from './s3sync-manifest';
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
	signal: AbortSignal
): Promise<SyncPlan> {
	checkAbort(signal);
	const baseManifest = await loadManifest(vaultPath);
	const baseMap = new Map(baseManifest.files.map((e) => [e.path, e]));

	checkAbort(signal);
	const localManifest = await buildLocalManifest(vaultPath, baseMap);
	checkAbort(signal);

	// Rust returns null only for a real 404; a non-empty base with a missing remote re-uploads rather than deletes.
	let remoteManifest: Manifest = { version: 3, files: [] };
	try {
		checkAbort(signal);
		const loaded = await syncLoadRemoteManifest(s3Prefix);
		if (loaded) remoteManifest = loaded;
	} catch (err) {
		if (signal.aborted) throw new Error('Sync cancelled', { cause: err });
		throw new Error(`Failed to download remote manifest: ${err}`, { cause: err });
	}

	const localMap = new Map(localManifest.files.map((e) => [e.path, e]));
	const remoteMap = new Map(remoteManifest.files.map((e) => [e.path, e]));
	const actions = await computeSyncActionsNative(
		baseManifest.files,
		localManifest.files,
		remoteManifest.files,
		nowSeconds()
	);
	checkAbort(signal);

	return { baseManifest, localManifest, remoteManifest, baseMap, localMap, remoteMap, actions };
}
