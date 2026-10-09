import { walkDirectory } from '$lib/fs/bridge';
import { hashFilesBatch, pruneTombstonesNative } from './bridge';

export function nowSeconds(): number {
	return Math.floor(Date.now() / 1000);
}

export type { ManifestEntry_Serialize as ManifestEntry } from '$lib/bindings';
export type { Manifest_Serialize as Manifest } from '$lib/bindings';
import type { ManifestEntry_Serialize as ManifestEntry } from '$lib/bindings';
import type { Manifest_Serialize as Manifest } from '$lib/bindings';

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
