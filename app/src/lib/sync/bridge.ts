import { commands } from '$lib/bindings';
import type { ManifestEntry_Deserialize, Manifest_Deserialize } from '$lib/bindings';
import type { ManifestEntry, Manifest } from './s3sync-manifest';

export type SyncActionKind =
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

export async function loadManifest(vaultPath: string, encryptionKey: number[]): Promise<Manifest> {
	const r = await commands.loadManifest(vaultPath, encryptionKey);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function saveManifest(
	vaultPath: string,
	encryptionKey: number[],
	manifest: Manifest
): Promise<void> {
	const r = await commands.saveManifest(vaultPath, encryptionKey, asDeserManifest(manifest));
	if (r.status === 'error') throw r.error;
}

export async function computeSyncActionsNative(
	baseFiles: ManifestEntry[],
	localFiles: ManifestEntry[],
	remoteFiles: ManifestEntry[]
): Promise<SyncAction[]> {
	return commands.computeSyncActions(
		asDeserEntries(baseFiles),
		asDeserEntries(localFiles),
		asDeserEntries(remoteFiles)
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
	paths: string[],
	encryptionKey: number[]
): Promise<void> {
	const r = await commands.syncUploadFiles(vaultPath, s3Prefix, paths, encryptionKey);
	if (r.status === 'error') throw r.error;
}

export async function syncDownloadFiles(
	vaultPath: string,
	s3Prefix: string,
	paths: string[],
	mtimes: number[],
	encryptionKey: number[]
): Promise<string[]> {
	const r = await commands.syncDownloadFiles(vaultPath, s3Prefix, paths, mtimes, encryptionKey);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function syncUploadManifest(
	s3Prefix: string,
	encryptionKey: number[],
	manifest: Manifest
): Promise<void> {
	const r = await commands.syncUploadManifest(s3Prefix, encryptionKey, asDeserManifest(manifest));
	if (r.status === 'error') throw r.error;
}

export async function syncDeleteFiles(
	s3Prefix: string,
	paths: string[],
	encryptionKey: number[]
): Promise<void> {
	const r = await commands.syncDeleteFiles(s3Prefix, paths, encryptionKey);
	if (r.status === 'error') throw r.error;
}

export async function pathToS3Key(relPath: string, encryptionKey: number[]): Promise<string> {
	return commands.pathToS3Key(relPath, encryptionKey);
}
