import { commands } from '$lib/bindings';
import type { ManifestEntry_Deserialize, Manifest_Deserialize } from '$lib/bindings';
import type { ManifestEntry, Manifest } from './s3sync-manifest';

type SyncActionKind =
	| 'upload'
	| 'download'
	| 'delete-remote'
	| 'delete-local'
	| 'conflict'
	| 'conflict-delete-local'
	| 'conflict-delete-remote';

export interface SyncAction {
	kind: SyncActionKind;
	path: string;
}

const asDeserEntries = (entries: ManifestEntry[]): ManifestEntry_Deserialize[] =>
	entries as ManifestEntry_Deserialize[];
const asDeserManifest = (manifest: Manifest): Manifest_Deserialize =>
	manifest as Manifest_Deserialize;

export async function hashFilesBatch(vaultPath: string, paths: string[]): Promise<string[]> {
	const r = await commands.hashFilesBatch(vaultPath, paths);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function loadManifest(vaultPath: string): Promise<Manifest> {
	const r = await commands.loadManifest(vaultPath);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function saveManifest(vaultPath: string, manifest: Manifest): Promise<void> {
	const r = await commands.saveManifest(vaultPath, asDeserManifest(manifest));
	if (r.status === 'error') throw r.error;
}

export async function computeSyncActionsNative(
	baseFiles: ManifestEntry[],
	localFiles: ManifestEntry[],
	remoteFiles: ManifestEntry[],
	nowSeconds: number
): Promise<SyncAction[]> {
	return commands.computeSyncActions(
		asDeserEntries(baseFiles),
		asDeserEntries(localFiles),
		asDeserEntries(remoteFiles),
		nowSeconds
	) as Promise<SyncAction[]>;
}

export async function collectTombstonesNative(files: ManifestEntry[]): Promise<ManifestEntry[]> {
	return commands.collectTombstones(asDeserEntries(files));
}

export async function mergeTombstonesNative(
	a: ManifestEntry[],
	b: ManifestEntry[]
): Promise<ManifestEntry[]> {
	return commands.mergeTombstones(asDeserEntries(a), asDeserEntries(b));
}

export async function pruneTombstonesNative(
	tombstones: ManifestEntry[],
	nowSeconds: number
): Promise<ManifestEntry[]> {
	return commands.pruneTombstones(asDeserEntries(tombstones), nowSeconds);
}

export async function syncUploadFiles(
	vaultPath: string,
	s3Prefix: string,
	paths: string[]
): Promise<void> {
	const r = await commands.syncUploadFiles(vaultPath, s3Prefix, paths);
	if (r.status === 'error') throw r.error;
}

export async function syncDownloadFiles(
	vaultPath: string,
	s3Prefix: string,
	paths: string[],
	mtimes: number[]
): Promise<string[]> {
	const r = await commands.syncDownloadFiles(vaultPath, s3Prefix, paths, mtimes);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function syncUploadManifest(s3Prefix: string, manifest: Manifest): Promise<void> {
	const r = await commands.syncUploadManifest(s3Prefix, asDeserManifest(manifest));
	if (r.status === 'error') throw r.error;
}

export async function syncDeleteFiles(s3Prefix: string, paths: string[]): Promise<void> {
	const r = await commands.syncDeleteFiles(s3Prefix, paths);
	if (r.status === 'error') throw r.error;
}

export async function syncLoadRemoteManifest(s3Prefix: string): Promise<Manifest | null> {
	const r = await commands.syncLoadRemoteManifest(s3Prefix);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function syncWriteRemoteCopy(
	vaultPath: string,
	s3Prefix: string,
	relPath: string,
	destPath: string,
	modified: number
): Promise<void> {
	const r = await commands.syncWriteRemoteCopy(vaultPath, s3Prefix, relPath, destPath, modified);
	if (r.status === 'error') throw r.error;
}
