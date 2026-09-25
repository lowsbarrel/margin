import { s3Download } from '$lib/s3/bridge';
import { decryptBlob } from '$lib/crypto/bridge';
import { walkDirectory } from '$lib/fs/bridge';
import { hashFilesBatch, pruneTombstonesNative } from './bridge';

export function nowSeconds(): number {
	return Math.floor(Date.now() / 1000);
}

export type { ManifestEntry_Serialize as ManifestEntry } from '$lib/bindings';
export type { Manifest_Serialize as Manifest } from '$lib/bindings';
import type { ManifestEntry_Serialize as ManifestEntry } from '$lib/bindings';
import type { Manifest_Serialize as Manifest } from '$lib/bindings';

function validateManifest(obj: unknown): Manifest {
	if (typeof obj !== 'object' || obj === null) throw new Error('Manifest is not an object');
	const m = obj as Record<string, unknown>;
	if (typeof m.version !== 'number') throw new Error('Manifest missing version');
	if (!Array.isArray(m.files)) throw new Error('Manifest missing files array');
	for (const entry of m.files) {
		if (typeof entry !== 'object' || entry === null) throw new Error('Invalid manifest entry');
		const e = entry as Record<string, unknown>;
		if (typeof e.path !== 'string') throw new Error('Manifest entry missing path');
		if (typeof e.hash !== 'string') throw new Error('Manifest entry missing hash');
		if (typeof e.modified !== 'number') throw new Error('Manifest entry missing modified');
		if (e.deleted_at !== undefined && typeof e.deleted_at !== 'number')
			throw new Error('Manifest entry has invalid deleted_at');
	}
	return obj as Manifest;
}

interface LocalFile {
	path: string;
	fullPath: string;
	modified: number;
}

async function walkVault(basePath: string): Promise<LocalFile[]> {
	const all = await walkDirectory(basePath);
	return all
		.filter((e) => !e.is_dir)
		.map((e) => {
			const relativePath = e.path.slice(basePath.length + 1);
			return { path: relativePath, fullPath: e.path, modified: e.modified };
		});
}

export async function buildLocalManifest(
	vaultPath: string,
	baseMap: Map<string, ManifestEntry>
): Promise<Manifest> {
	const localFiles = await walkVault(vaultPath);
	const unchangedEntries: ManifestEntry[] = [];
	const pathsToHash: string[] = [];
	const pathsMeta: { path: string; modified: number }[] = [];

	for (const file of localFiles) {
		const baseEntry = baseMap.get(file.path);
		if (baseEntry && !baseEntry.deleted_at && baseEntry.modified === file.modified) {
			unchangedEntries.push(baseEntry);
		} else {
			pathsToHash.push(file.path);
			pathsMeta.push({ path: file.path, modified: file.modified });
		}
	}

	const hashes = pathsToHash.length > 0 ? await hashFilesBatch(vaultPath, pathsToHash) : [];
	const manifest: Manifest = { version: 3, files: [...unchangedEntries] };
	for (let i = 0; i < pathsToHash.length; i++) {
		manifest.files.push({
			path: pathsMeta[i].path,
			hash: hashes[i],
			modified: pathsMeta[i].modified
		});
	}
	return manifest;
}

export function isMissingRemoteManifestError(err: unknown): boolean {
	const message = String(err);
	return message.includes('NoSuchKey') || message.includes('Not Found') || message.includes('404');
}

export async function loadRemoteManifest(
	s3Prefix: string,
	encryptionKey: number[]
): Promise<Manifest | null> {
	const encManifest = await s3Download(`${s3Prefix}manifest.enc`);
	const decManifest = await decryptBlob(encManifest, encryptionKey);
	const parsed = validateManifest(JSON.parse(new TextDecoder().decode(decManifest)));
	// A v2 manifest keyed plaintext paths; absent forces re-upload under HMAC keys.
	return parsed.version >= 3 ? parsed : null;
}

export async function buildManifests(
	mergedFiles: Map<string, ManifestEntry>,
	tombstones: Map<string, ManifestEntry>,
	missingRemoteBlobs: Map<string, ManifestEntry>
): Promise<{ local: Manifest; uploaded: Manifest }> {
	const pruned = await pruneTombstonesNative(Array.from(tombstones.values()), nowSeconds());
	const local: Manifest = {
		version: 3,
		files: [...Array.from(mergedFiles.values()), ...pruned]
	};
	const uploaded: Manifest =
		missingRemoteBlobs.size > 0
			? { version: 3, files: [...local.files, ...Array.from(missingRemoteBlobs.values())] }
			: local;
	return { local, uploaded };
}
